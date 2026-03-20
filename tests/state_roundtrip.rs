// tests/state_roundtrip.rs
use rings::state::{RunMeta, RunStatus, StateFile};
use tempfile::tempdir;

#[test]
fn state_file_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    let state = StateFile {
        schema_version: 1,
        run_id: "run_test_123".to_string(),
        workflow_file: "/path/to/workflow.toml".to_string(),
        last_completed_run: 7,
        last_completed_cycle: 2,
        last_completed_phase_index: 0,
        last_completed_iteration: 3,
        total_runs_completed: 7,
        cumulative_cost_usd: 1.42,
        claude_resume_commands: vec!["claude resume abc".to_string()],
        canceled_at: None,
        failure_reason: None,
        ancestry: None,
    };

    state.write_atomic(&path).unwrap();
    let loaded = StateFile::read(&path).unwrap();
    assert_eq!(loaded.run_id, state.run_id);
    assert_eq!(loaded.last_completed_run, 7);
    assert_eq!(loaded.cumulative_cost_usd, 1.42);
    assert_eq!(loaded.claude_resume_commands, vec!["claude resume abc"]);
}

#[test]
fn run_meta_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("run.toml");

    let meta = RunMeta {
        run_id: "run_abc".to_string(),
        workflow_file: "/abs/path.toml".to_string(),
        started_at: "2026-03-15T14:30:00Z".to_string(),
        rings_version: "0.1.0".to_string(),
        status: RunStatus::Running,
        phase_fingerprint: None,
        parent_run_id: None,
        continuation_of: None,
        ancestry_depth: 0,
        context_dir: None,
    };

    meta.write(&path).unwrap();
    let loaded = RunMeta::read(&path).unwrap();
    assert_eq!(loaded.run_id, meta.run_id);
    assert_eq!(loaded.status, RunStatus::Running);
}

#[test]
fn state_write_is_atomic() {
    // Write must go via temp file + rename to avoid corruption.
    // We verify by checking that a valid file always exists after write.
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    for i in 0..10u32 {
        let state = StateFile {
            schema_version: 1,
            run_id: "run_test".to_string(),
            workflow_file: "/w.toml".to_string(),
            last_completed_run: i,
            last_completed_cycle: 1,
            last_completed_phase_index: 0,
            last_completed_iteration: 1,
            total_runs_completed: i,
            cumulative_cost_usd: 0.0,
            claude_resume_commands: vec![],
            canceled_at: None,
            failure_reason: None,
            ancestry: None,
        };
        state.write_atomic(&path).unwrap();
        // File must be readable after every write
        let loaded = StateFile::read(&path).unwrap();
        assert_eq!(loaded.last_completed_run, i);
    }
}
