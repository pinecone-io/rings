use chrono::{Duration, Utc};
use rings::duration::SinceSpec;
use rings::list::{list_runs, ListFilters};
use rings::state::{RunMeta, RunStatus, StateFile};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use tempfile::tempdir;

/// Create a test run directory with run.toml and optionally state.json
fn create_test_run(
    base_dir: &std::path::Path,
    run_id: &str,
    started_at: &str,
    status: RunStatus,
    cycles: u32,
    cost: f64,
) -> PathBuf {
    let run_dir = base_dir.join(run_id);
    fs::create_dir_all(&run_dir).unwrap();

    let meta = RunMeta {
        run_id: run_id.to_string(),
        workflow_file: format!("/path/to/workflow-{}.toml", run_id),
        started_at: started_at.to_string(),
        rings_version: "0.1.0".to_string(),
        status,
        phase_fingerprint: None,
        parent_run_id: None,
        continuation_of: None,
        ancestry_depth: 0,
    };
    meta.write(&run_dir.join("run.toml")).unwrap();

    // Write state.json if cycles > 0
    if cycles > 0 {
        let state = StateFile {
            schema_version: 1,
            run_id: run_id.to_string(),
            workflow_file: meta.workflow_file.clone(),
            last_completed_run: cycles,
            last_completed_cycle: cycles,
            last_completed_phase_index: 0,
            last_completed_iteration: 1,
            total_runs_completed: cycles,
            cumulative_cost_usd: cost,
            claude_resume_commands: vec![],
            canceled_at: None,
            failure_reason: None,
            ancestry: None,
        };
        state.write_atomic(&run_dir.join("state.json")).unwrap();
    }

    run_dir
}

#[test]
fn test_runstatus_roundtrip() {
    // Test round-trip via RunMeta which contains a RunStatus field
    let variants = vec![
        RunStatus::Running,
        RunStatus::Completed,
        RunStatus::Canceled,
        RunStatus::Failed,
        RunStatus::Incomplete,
        RunStatus::Stopped,
    ];

    for variant in variants {
        let meta = RunMeta {
            run_id: "test".to_string(),
            workflow_file: "/path/to/workflow.toml".to_string(),
            started_at: "2024-01-01T00:00:00Z".to_string(),
            rings_version: "0.1.0".to_string(),
            status: variant,
            phase_fingerprint: None,
            parent_run_id: None,
            continuation_of: None,
            ancestry_depth: 0,
        };

        let toml_str = toml::to_string_pretty(&meta).unwrap();
        let parsed: RunMeta = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.status, variant);
    }
}

#[test]
fn test_runstatus_fromstr() {
    assert_eq!("running".parse::<RunStatus>().unwrap(), RunStatus::Running);
    assert_eq!(
        "completed".parse::<RunStatus>().unwrap(),
        RunStatus::Completed
    );
    assert_eq!(
        "canceled".parse::<RunStatus>().unwrap(),
        RunStatus::Canceled
    );
    assert_eq!("failed".parse::<RunStatus>().unwrap(), RunStatus::Failed);
    assert_eq!(
        "incomplete".parse::<RunStatus>().unwrap(),
        RunStatus::Incomplete
    );
    assert_eq!("stopped".parse::<RunStatus>().unwrap(), RunStatus::Stopped);
}

#[test]
fn test_runstatus_old_format_deserializes() {
    // Old run.toml with bare `status = "running"` should parse correctly
    let toml_str = r#"
run_id = "test_run"
workflow_file = "/path/to/workflow.toml"
started_at = "2024-01-01T00:00:00Z"
rings_version = "0.1.0"
status = "running"
"#;
    let meta: RunMeta = toml::from_str(toml_str).unwrap();
    assert_eq!(meta.status, RunStatus::Running);
}

#[test]
fn test_runstatus_display() {
    assert_eq!(RunStatus::Running.to_string(), "running");
    assert_eq!(RunStatus::Completed.to_string(), "completed");
}

#[test]
fn test_since_spec_relative() {
    let spec = SinceSpec::from_str("7d").unwrap();
    match spec {
        SinceSpec::Relative(_) => {
            // OK
        }
        _ => panic!("expected Relative"),
    }
}

#[test]
fn test_since_spec_absolute() {
    let spec = SinceSpec::from_str("2024-03-15").unwrap();
    match spec {
        SinceSpec::AbsoluteDate(date) => {
            assert_eq!(date.to_string(), "2024-03-15");
        }
        _ => panic!("expected AbsoluteDate"),
    }
}

#[test]
fn test_since_spec_invalid_date() {
    assert!(SinceSpec::from_str("2024-13-01").is_err());
}

#[test]
fn test_list_runs_empty_directory() {
    let dir = tempdir().unwrap();
    let filters = ListFilters {
        since: None,
        status: None,
        workflow: None,
        limit: 20,
    };
    let runs = list_runs(&filters, dir.path()).unwrap();
    assert!(runs.is_empty());
}

#[test]
fn test_list_runs_nonexistent_directory() {
    let runs = list_runs(
        &ListFilters {
            since: None,
            status: None,
            workflow: None,
            limit: 20,
        },
        std::path::Path::new("/nonexistent/path"),
    )
    .unwrap();
    assert!(runs.is_empty());
}

#[test]
fn test_list_runs_basic_listing() {
    let dir = tempdir().unwrap();
    create_test_run(
        dir.path(),
        "run_20240315_143022_abc",
        "2024-03-15T14:30:22Z",
        RunStatus::Completed,
        5,
        1.23,
    );
    create_test_run(
        dir.path(),
        "run_20240314_100000_def",
        "2024-03-14T10:00:00Z",
        RunStatus::Running,
        3,
        0.75,
    );

    let filters = ListFilters {
        since: None,
        status: None,
        workflow: None,
        limit: 20,
    };
    let runs = list_runs(&filters, dir.path()).unwrap();
    assert_eq!(runs.len(), 2);

    // Check that runs are sorted by date descending (most recent first)
    assert_eq!(runs[0].run_id, "run_20240315_143022_abc");
    assert_eq!(runs[1].run_id, "run_20240314_100000_def");
}

#[test]
fn test_list_runs_status_filter() {
    let dir = tempdir().unwrap();
    create_test_run(
        dir.path(),
        "run_20240315_completed",
        "2024-03-15T14:30:22Z",
        RunStatus::Completed,
        5,
        1.23,
    );
    create_test_run(
        dir.path(),
        "run_20240314_running",
        "2024-03-14T10:00:00Z",
        RunStatus::Running,
        3,
        0.75,
    );
    create_test_run(
        dir.path(),
        "run_20240313_canceled",
        "2024-03-13T09:00:00Z",
        RunStatus::Canceled,
        2,
        0.50,
    );

    let filters = ListFilters {
        since: None,
        status: Some(RunStatus::Completed),
        workflow: None,
        limit: 20,
    };
    let runs = list_runs(&filters, dir.path()).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, RunStatus::Completed);
}

#[test]
fn test_list_runs_workflow_filter() {
    let dir = tempdir().unwrap();
    create_test_run(
        dir.path(),
        "run_001",
        "2024-03-15T14:30:22Z",
        RunStatus::Completed,
        5,
        1.23,
    );
    create_test_run(
        dir.path(),
        "run_002",
        "2024-03-14T10:00:00Z",
        RunStatus::Running,
        3,
        0.75,
    );

    let filters = ListFilters {
        since: None,
        status: None,
        workflow: Some("001".to_string()),
        limit: 20,
    };
    let runs = list_runs(&filters, dir.path()).unwrap();
    assert_eq!(runs.len(), 1);
}

#[test]
fn test_list_runs_limit() {
    let dir = tempdir().unwrap();
    for i in 1..=10 {
        create_test_run(
            dir.path(),
            &format!("run_2024031{}_{:02}", 5 - (i / 5), i),
            "2024-03-15T14:30:22Z",
            RunStatus::Completed,
            1,
            0.1,
        );
    }

    let filters = ListFilters {
        since: None,
        status: None,
        workflow: None,
        limit: 5,
    };
    let runs = list_runs(&filters, dir.path()).unwrap();
    assert_eq!(runs.len(), 5);
}

#[test]
fn test_list_runs_since_filter_relative() {
    let dir = tempdir().unwrap();
    let now = Utc::now();
    let two_days_ago = (now - Duration::days(2))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let eight_days_ago = (now - Duration::days(8))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    create_test_run(
        dir.path(),
        "run_recent",
        &two_days_ago,
        RunStatus::Completed,
        1,
        0.1,
    );
    create_test_run(
        dir.path(),
        "run_old",
        &eight_days_ago,
        RunStatus::Completed,
        1,
        0.1,
    );

    let filters = ListFilters {
        since: Some(SinceSpec::from_str("7d").unwrap()),
        status: None,
        workflow: None,
        limit: 20,
    };
    let runs = list_runs(&filters, dir.path()).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].run_id, "run_recent");
}

#[test]
fn test_list_runs_since_filter_absolute() {
    let dir = tempdir().unwrap();
    create_test_run(
        dir.path(),
        "run_after",
        "2024-03-16T10:00:00Z",
        RunStatus::Completed,
        1,
        0.1,
    );
    create_test_run(
        dir.path(),
        "run_before",
        "2024-03-14T10:00:00Z",
        RunStatus::Completed,
        1,
        0.1,
    );

    let filters = ListFilters {
        since: Some(SinceSpec::from_str("2024-03-15").unwrap()),
        status: None,
        workflow: None,
        limit: 20,
    };
    let runs = list_runs(&filters, dir.path()).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].run_id, "run_after");
}

#[test]
fn test_list_runs_skips_corrupt_run_toml() {
    let dir = tempdir().unwrap();
    let corrupt_dir = dir.path().join("run_corrupt");
    fs::create_dir_all(&corrupt_dir).unwrap();
    fs::write(corrupt_dir.join("run.toml"), "invalid toml [[[").unwrap();

    create_test_run(
        dir.path(),
        "run_valid",
        "2024-03-15T14:30:22Z",
        RunStatus::Completed,
        1,
        0.1,
    );

    let filters = ListFilters {
        since: None,
        status: None,
        workflow: None,
        limit: 20,
    };
    let runs = list_runs(&filters, dir.path()).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].run_id, "run_valid");
}

#[test]
fn test_list_runs_missing_state_json() {
    let dir = tempdir().unwrap();
    let meta = RunMeta {
        run_id: "run_no_state".to_string(),
        workflow_file: "/path/to/workflow.toml".to_string(),
        started_at: "2024-03-15T14:30:22Z".to_string(),
        rings_version: "0.1.0".to_string(),
        status: RunStatus::Running,
        phase_fingerprint: None,
        parent_run_id: None,
        continuation_of: None,
        ancestry_depth: 0,
    };
    let run_dir = dir.path().join("run_no_state");
    fs::create_dir_all(&run_dir).unwrap();
    meta.write(&run_dir.join("run.toml")).unwrap();

    let filters = ListFilters {
        since: None,
        status: None,
        workflow: None,
        limit: 20,
    };
    let runs = list_runs(&filters, dir.path()).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].cycles_completed, 0);
    assert_eq!(runs[0].total_cost_usd, None);
}

#[test]
fn test_list_runs_status_incomplete() {
    let dir = tempdir().unwrap();
    create_test_run(
        dir.path(),
        "run_incomplete",
        "2024-03-15T14:30:22Z",
        RunStatus::Incomplete,
        5,
        1.0,
    );

    let filters = ListFilters {
        since: None,
        status: Some(RunStatus::Incomplete),
        workflow: None,
        limit: 20,
    };
    let runs = list_runs(&filters, dir.path()).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, RunStatus::Incomplete);
}

#[test]
fn test_list_runs_status_stopped() {
    let dir = tempdir().unwrap();
    create_test_run(
        dir.path(),
        "run_stopped",
        "2024-03-15T14:30:22Z",
        RunStatus::Stopped,
        3,
        0.5,
    );

    let filters = ListFilters {
        since: None,
        status: Some(RunStatus::Stopped),
        workflow: None,
        limit: 20,
    };
    let runs = list_runs(&filters, dir.path()).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, RunStatus::Stopped);
}
