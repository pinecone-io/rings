// Tests for F-123: Snapshot Storage Warning
// Run with: cargo test --features testing
//
// The warning is printed to stderr when `snapshot_cycles = true` and estimated storage > 100 MB.
// On non-TTY stdin (always the case in tests), the engine proceeds without prompting.
use rings::cancel::CancelState;
use rings::engine::{run_workflow, EngineConfig};
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::workflow::{CompiledErrorProfile, CompletionSignalMode, PhaseConfig, Workflow};
use std::sync::Arc;
use tempfile::tempdir;

fn done_executor() -> MockExecutor {
    MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }])
}

fn make_snapshot_workflow(context_dir: &str, max_cycles: u32, snapshot_cycles: bool) -> Workflow {
    Workflow {
        completion_signal: "DONE".to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: context_dir.to_string(),
        max_cycles,
        output_dir: None,
        delay_between_runs: 0,
        delay_between_cycles: 0,
        executor: None,
        budget_cap_usd: None,
        timeout_per_run_secs: None,
        compiled_error_profile: CompiledErrorProfile {
            quota_regexes: vec![],
            auth_regexes: vec![],
        },
        quota_backoff: false,
        quota_backoff_delay: 0,
        quota_backoff_max_retries: 0,
        manifest_enabled: false,
        manifest_ignore: vec![],
        manifest_mtime_optimization: false,
        snapshot_cycles,
        compiled_cost_parser: rings::cost::CompiledCostParser::ClaudeCode,
        lock_name: None,
        phases: vec![PhaseConfig {
            name: "builder".to_string(),
            prompt: None,
            prompt_text: Some("do work".to_string()),
            runs_per_cycle: 1,
            budget_cap_usd: None,
            timeout_per_run_secs: None,
            consumes: vec![],
            produces: vec![],
            produces_required: false,
            executor: None,
        }],
    }
}

/// When `snapshot_cycles = false`, the storage check is skipped entirely.
/// Engine should run normally regardless of directory size.
#[test]
fn snapshot_false_skips_storage_check() {
    let dir = tempdir().unwrap();
    // Create a small context dir
    let context = dir.path().join("ctx");
    std::fs::create_dir_all(&context).unwrap();
    std::fs::write(context.join("file.txt"), "content").unwrap();

    let workflow = make_snapshot_workflow(
        context.to_str().unwrap(),
        50,
        false, // snapshot_cycles = false
    );
    let executor = done_executor();
    let cancel = Arc::new(CancelState::new());
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-snapshot-false".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, Some(cancel)).unwrap();
    // Engine ran and completed (exit 0 from signal)
    assert_eq!(result.exit_code, 0);
}

/// When `snapshot_cycles = true` and context is small, no warning is emitted
/// and the engine runs normally.
#[test]
fn snapshot_true_small_context_no_warning() {
    let dir = tempdir().unwrap();
    let context = dir.path().join("ctx");
    std::fs::create_dir_all(&context).unwrap();
    // Write a very small file — estimated storage is tiny
    std::fs::write(context.join("tiny.txt"), "hi").unwrap();

    let workflow = make_snapshot_workflow(
        context.to_str().unwrap(),
        3, // small max_cycles → tiny total
        true,
    );
    let executor = done_executor();
    let cancel = Arc::new(CancelState::new());
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-snapshot-small".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, Some(cancel)).unwrap();
    // Should complete normally without warning or abort
    assert_eq!(result.exit_code, 0);
}

/// When `snapshot_cycles = true` and estimated total exceeds 100 MB, the warning is
/// emitted to stderr. On non-TTY stdin (test environment), the engine proceeds without
/// prompting — it should NOT abort.
///
/// We achieve >100 MB estimated total without creating large files by using a very high
/// max_cycles: per_snapshot_bytes × max_cycles > 100_000_000.
#[test]
fn snapshot_true_large_estimated_storage_proceeds_on_non_tty() {
    let dir = tempdir().unwrap();
    let context = dir.path().join("ctx");
    std::fs::create_dir_all(&context).unwrap();
    // Write a 2 KB file. With max_cycles = 100_000, estimated total = 2048 * 100_000 = 204_800_000 > 100 MB.
    let data = vec![b'x'; 2048];
    std::fs::write(context.join("data.bin"), &data).unwrap();

    let workflow = make_snapshot_workflow(
        context.to_str().unwrap(),
        100_000, // large max_cycles so estimate exceeds threshold
        true,
    );
    let executor = done_executor();
    let cancel = Arc::new(CancelState::new());
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-snapshot-large".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    // In a non-TTY test environment, the prompt is skipped and the engine proceeds.
    let result = run_workflow(&workflow, &executor, &config, None, Some(cancel)).unwrap();
    // Engine should run and complete (exit 0 from signal), not abort with exit 1.
    assert_eq!(result.exit_code, 0);
}
