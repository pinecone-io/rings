// tests/state_recovery.rs
use rings::audit::CostEntry;
use rings::state::{StateFile, StateLoadResult};
use tempfile::tempdir;

#[test]
fn valid_state_file_loads_ok() {
    let dir = tempdir().unwrap();
    let state_path = dir.path().join("state.json");
    let costs_path = dir.path().join("costs.jsonl");

    let state = StateFile {
        schema_version: 1,
        run_id: "run_test_001".to_string(),
        workflow_file: "/path/to/workflow.toml".to_string(),
        last_completed_run: 5,
        last_completed_cycle: 1,
        last_completed_phase_index: 0,
        last_completed_iteration: 2,
        total_runs_completed: 5,
        cumulative_cost_usd: 0.10,
        claude_resume_commands: vec![],
        canceled_at: None,
        failure_reason: None,
        ancestry: None,
    };

    state.write_atomic(&state_path).unwrap();

    let result = StateFile::load_or_recover(&state_path, &costs_path);
    assert!(matches!(result, StateLoadResult::Ok(_)));
    if let StateLoadResult::Ok(loaded) = result {
        assert_eq!(loaded.last_completed_run, 5);
        assert_eq!(loaded.run_id, "run_test_001");
    }
}

#[test]
fn corrupt_state_with_valid_costs_recovers() {
    let dir = tempdir().unwrap();
    let state_path = dir.path().join("state.json");
    let costs_path = dir.path().join("costs.jsonl");

    // Write corrupt state file
    std::fs::write(&state_path, "{ invalid json").unwrap();

    // Write valid costs.jsonl with several entries
    let entries = vec![
        CostEntry {
            run: 1,
            cycle: 1,
            phase: "builder".to_string(),
            iteration: 1,
            cost_usd: Some(0.05),
            input_tokens: Some(1000),
            output_tokens: Some(200),
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        },
        CostEntry {
            run: 2,
            cycle: 1,
            phase: "builder".to_string(),
            iteration: 2,
            cost_usd: Some(0.06),
            input_tokens: Some(1200),
            output_tokens: Some(250),
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        },
        CostEntry {
            run: 3,
            cycle: 1,
            phase: "reviewer".to_string(),
            iteration: 1,
            cost_usd: Some(0.04),
            input_tokens: Some(800),
            output_tokens: Some(150),
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        },
    ];

    for entry in &entries {
        let line = serde_json::to_string(entry).unwrap();
        std::fs::write(&costs_path, format!("{}\n", line)).unwrap();
    }

    let result = StateFile::load_or_recover(&state_path, &costs_path);
    assert!(matches!(result, StateLoadResult::Recovered { .. }));
    if let StateLoadResult::Recovered { state, warning } = result {
        assert_eq!(state.last_completed_run, 3);
        assert!(warning.contains("3"));
        assert!(warning.contains("corrupt"));
    }
}

#[test]
fn corrupt_state_with_empty_costs_unrecoverable() {
    let dir = tempdir().unwrap();
    let state_path = dir.path().join("state.json");
    let costs_path = dir.path().join("costs.jsonl");

    // Write corrupt state file
    std::fs::write(&state_path, "{ invalid json").unwrap();

    // Create empty costs file
    std::fs::write(&costs_path, "").unwrap();

    let result = StateFile::load_or_recover(&state_path, &costs_path);
    assert!(matches!(result, StateLoadResult::Unrecoverable { .. }));
}

#[test]
fn corrupt_state_absent_costs_unrecoverable() {
    let dir = tempdir().unwrap();
    let state_path = dir.path().join("state.json");
    let costs_path = dir.path().join("costs.jsonl");

    // Write corrupt state file
    std::fs::write(&state_path, "{ invalid json").unwrap();

    // costs_path doesn't exist

    let result = StateFile::load_or_recover(&state_path, &costs_path);
    assert!(matches!(result, StateLoadResult::Unrecoverable { .. }));
}

#[test]
fn malformed_jsonl_lines_skipped() {
    let dir = tempdir().unwrap();
    let state_path = dir.path().join("state.json");
    let costs_path = dir.path().join("costs.jsonl");

    // Write corrupt state file
    std::fs::write(&state_path, "{ invalid json").unwrap();

    // Write costs.jsonl with mixed valid and malformed lines
    let mut content = String::new();
    content.push_str("{ invalid json\n");
    content.push_str(
        &serde_json::to_string(&CostEntry {
            run: 1,
            cycle: 1,
            phase: "builder".to_string(),
            iteration: 1,
            cost_usd: Some(0.05),
            input_tokens: Some(1000),
            output_tokens: Some(200),
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        })
        .unwrap(),
    );
    content.push_str("\n");
    content.push_str("not json at all\n");
    content.push_str(
        &serde_json::to_string(&CostEntry {
            run: 5,
            cycle: 2,
            phase: "reviewer".to_string(),
            iteration: 1,
            cost_usd: Some(0.03),
            input_tokens: Some(500),
            output_tokens: Some(100),
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        })
        .unwrap(),
    );
    content.push_str("\n");

    std::fs::write(&costs_path, content).unwrap();

    let result = StateFile::load_or_recover(&state_path, &costs_path);
    if let StateLoadResult::Recovered { state, .. } = result {
        assert_eq!(state.last_completed_run, 5);
    } else {
        panic!("Expected Recovered but got {:?}", result);
    }
}

#[test]
fn none_cost_entries_included_in_recovery() {
    let dir = tempdir().unwrap();
    let state_path = dir.path().join("state.json");
    let costs_path = dir.path().join("costs.jsonl");

    // Write corrupt state file
    std::fs::write(&state_path, "{ invalid json").unwrap();

    // Write costs.jsonl with one None cost entry followed by valid entries
    let mut content = String::new();
    content.push_str(
        &serde_json::to_string(&CostEntry {
            run: 1,
            cycle: 1,
            phase: "builder".to_string(),
            iteration: 1,
            cost_usd: None,
            input_tokens: None,
            output_tokens: None,
            cost_confidence: "none".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        })
        .unwrap(),
    );
    content.push_str("\n");
    content.push_str(
        &serde_json::to_string(&CostEntry {
            run: 2,
            cycle: 1,
            phase: "builder".to_string(),
            iteration: 2,
            cost_usd: Some(0.06),
            input_tokens: Some(1200),
            output_tokens: Some(250),
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        })
        .unwrap(),
    );
    content.push_str("\n");

    std::fs::write(&costs_path, content).unwrap();

    let result = StateFile::load_or_recover(&state_path, &costs_path);
    if let StateLoadResult::Recovered { state, .. } = result {
        assert_eq!(state.last_completed_run, 2);
    } else {
        panic!("Expected Recovered but got {:?}", result);
    }
}

#[test]
fn crash_scenario_state_lags_costs() {
    let dir = tempdir().unwrap();
    let state_path = dir.path().join("state.json");
    let costs_path = dir.path().join("costs.jsonl");

    // Simulate crash: state.json saved run 2, but costs.jsonl has run 3
    std::fs::write(&state_path, "{ invalid json").unwrap();

    let mut content = String::new();
    for run in 1..=3 {
        let entry = CostEntry {
            run,
            cycle: 1,
            phase: "builder".to_string(),
            iteration: run,
            cost_usd: Some(0.05),
            input_tokens: Some(1000),
            output_tokens: Some(200),
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        };
        content.push_str(&serde_json::to_string(&entry).unwrap());
        content.push_str("\n");
    }

    std::fs::write(&costs_path, content).unwrap();

    let result = StateFile::load_or_recover(&state_path, &costs_path);
    if let StateLoadResult::Recovered { state, .. } = result {
        // Recovery should pick the latest run from costs, which is 3
        assert_eq!(state.last_completed_run, 3);
    } else {
        panic!("Expected Recovered but got {:?}", result);
    }
}

#[test]
fn unrecoverable_includes_file_paths() {
    let dir = tempdir().unwrap();
    let state_path = dir.path().join("state.json");
    let costs_path = dir.path().join("costs.jsonl");

    // Write corrupt state, missing costs
    std::fs::write(&state_path, "{ invalid json").unwrap();

    let result = StateFile::load_or_recover(&state_path, &costs_path);
    if let StateLoadResult::Unrecoverable {
        state_path: sp,
        costs_path: cp,
    } = result
    {
        assert!(sp.to_string_lossy().contains("state.json"));
        assert!(cp.to_string_lossy().contains("costs.jsonl"));
    } else {
        panic!("Expected Unrecoverable but got {:?}", result);
    }
}
