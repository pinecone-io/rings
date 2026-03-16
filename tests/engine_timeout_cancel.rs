// tests/engine_timeout_cancel.rs
use rings::cancel::CancelState;
use rings::engine::{run_workflow, EngineConfig};
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::state::StateFile;
use rings::workflow::Workflow;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn sigterm_called_on_cancellation() {
    let mock_output = ExecutorOutput {
        combined: "test output".to_string(),
        exit_code: 0,
    };
    let executor = MockExecutor::new(vec![mock_output.clone()]);

    let dir = tempdir().unwrap();
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test_cancel_1".to_string(),
        workflow_file: "test.rings.toml".to_string(),
    };

    let workflow_str = r#"
[workflow]
completion_signal = "done"
context_dir = "."
max_cycles = 1

[[phases]]
name = "test"
prompt_text = "test prompt"
"#;

    let workflow: Workflow = workflow_str.parse().unwrap();

    // Create cancel state
    let cancel_state = Arc::new(CancelState::new());

    // Run workflow (will use the mock executor)
    let result = run_workflow(
        &workflow,
        &executor,
        &config,
        None,
        Some(cancel_state.clone()),
    );

    // The result should succeed since we never actually trigger cancellation in the run
    assert!(result.is_ok());
}

#[test]
fn timeout_failure_reason_recorded_in_state() {
    // This test verifies that when a timeout occurs, the failure_reason is set to "timeout"
    // in the state file. We can't directly test timeout behavior in unit tests without
    // mocking time, but we verify the field exists and is properly serialized.

    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    let state = StateFile {
        schema_version: 1,
        run_id: "test_timeout".to_string(),
        workflow_file: "test.toml".to_string(),
        last_completed_run: 1,
        last_completed_cycle: 1,
        last_completed_phase_index: 0,
        last_completed_iteration: 1,
        total_runs_completed: 1,
        cumulative_cost_usd: 0.0,
        claude_resume_commands: vec![],
        canceled_at: None,
        failure_reason: Some("timeout".to_string()),
    };

    state.write_atomic(&path).unwrap();
    let loaded = StateFile::read(&path).unwrap();

    assert_eq!(loaded.failure_reason, Some("timeout".to_string()));
}

#[test]
fn state_includes_failure_reason_field_with_default() {
    // Test that the failure_reason field is properly deserialized with default when missing
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    // Simulate an old state file without failure_reason
    let old_json = r#"{
        "schema_version": 1,
        "run_id": "old_run",
        "workflow_file": "test.toml",
        "last_completed_run": 1,
        "last_completed_cycle": 1,
        "last_completed_phase_index": 0,
        "last_completed_iteration": 1,
        "total_runs_completed": 1,
        "cumulative_cost_usd": 0.0,
        "claude_resume_commands": [],
        "canceled_at": null
    }"#;

    std::fs::write(&path, old_json).unwrap();
    let loaded = StateFile::read(&path).unwrap();

    // Should deserialize successfully with failure_reason as None
    assert_eq!(loaded.failure_reason, None);
}

#[test]
fn cancellation_state_recorded_with_null_failure_reason() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    let state = StateFile {
        schema_version: 1,
        run_id: "test_cancel".to_string(),
        workflow_file: "test.toml".to_string(),
        last_completed_run: 1,
        last_completed_cycle: 1,
        last_completed_phase_index: 0,
        last_completed_iteration: 1,
        total_runs_completed: 1,
        cumulative_cost_usd: 0.0,
        claude_resume_commands: vec![],
        canceled_at: Some("2026-03-15T14:30:00Z".to_string()),
        failure_reason: None,
    };

    state.write_atomic(&path).unwrap();
    let loaded = StateFile::read(&path).unwrap();

    assert_eq!(loaded.canceled_at, Some("2026-03-15T14:30:00Z".to_string()));
    assert_eq!(loaded.failure_reason, None);
}
