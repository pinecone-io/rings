// Cycle gate integration tests.
// Run with: cargo test --features testing
use rings::engine::{run_workflow, EngineConfig};
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::workflow::{
    CompiledErrorProfile, CompletionSignalMode, GateAction, GateConfig, PhaseConfig, Workflow,
};
use tempfile::tempdir;

fn default_compiled_error_profile() -> CompiledErrorProfile {
    CompiledErrorProfile {
        quota_regexes: vec![],
        auth_regexes: vec![],
    }
}

fn make_workflow_with_cycle_gate(
    cycle_gate: Option<GateConfig>,
    max_cycles: u32,
    delay_between_cycles: u64,
) -> Workflow {
    Workflow {
        completion_signal: "RINGS_DONE".to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: ".".to_string(),
        max_cycles,
        output_dir: None,
        delay_between_runs: 0,
        delay_between_cycles,
        executor: None,
        budget_cap_usd: None,
        timeout_per_run_secs: None,
        compiled_error_profile: default_compiled_error_profile(),
        quota_backoff: false,
        quota_backoff_delay: 0,
        quota_backoff_max_retries: 0,
        manifest_enabled: false,
        manifest_ignore: vec![],
        manifest_mtime_optimization: false,
        snapshot_cycles: false,
        compiled_cost_parser: rings::cost::CompiledCostParser::ClaudeCode,
        lock_name: None,
        cycle_gate,
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
            gate: None,
            gate_each_run: false,
        }],
    }
}

fn no_signal_output() -> ExecutorOutput {
    ExecutorOutput {
        combined: "no signal".to_string(),
        exit_code: 0,
    }
}

/// A gate that always passes — cycles run normally.
#[test]
fn cycle_gate_true_runs_normally() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "true".to_string(),
        on_fail: Some(GateAction::Stop),
        timeout: None,
    };
    let workflow = make_workflow_with_cycle_gate(Some(gate), 2, 0);
    let executor = MockExecutor::new(vec![no_signal_output(), no_signal_output()]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    // Both cycles ran (exit_code 1 = max_cycles)
    assert_eq!(result.exit_code, 1);
    assert_eq!(result.total_runs, 2);
}

/// Gate with `on_fail = "stop"` exits gracefully (exit 0) when the gate fails on the first cycle.
#[test]
fn cycle_gate_stop_exits_zero_on_first_cycle() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "false".to_string(),
        on_fail: Some(GateAction::Stop),
        timeout: None,
    };
    let workflow = make_workflow_with_cycle_gate(Some(gate), 5, 0);
    // No executor outputs needed: phases never run
    let executor = MockExecutor::new(vec![]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.total_runs, 0, "no phases should have run");
    assert_eq!(result.completed_cycles, 0);
}

/// Default on_fail for cycle_gate is Stop.
#[test]
fn cycle_gate_default_on_fail_is_stop() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "false".to_string(),
        on_fail: None, // should default to Stop
        timeout: None,
    };
    let workflow = make_workflow_with_cycle_gate(Some(gate), 5, 0);
    let executor = MockExecutor::new(vec![]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(
        result.exit_code, 0,
        "default on_fail should be stop (exit 0)"
    );
    assert_eq!(result.total_runs, 0);
}

/// Gate with `on_fail = "error"` exits with code 2.
#[test]
fn cycle_gate_error_exits_with_code_2() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "false".to_string(),
        on_fail: Some(GateAction::Error),
        timeout: None,
    };
    let workflow = make_workflow_with_cycle_gate(Some(gate), 5, 0);
    let executor = MockExecutor::new(vec![]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 2);
    assert_eq!(result.total_runs, 0, "no phases should have run");
}

/// Gate with `on_fail = "skip"` skips all phases for that cycle.
/// With max_cycles = 3 and always-failing gate, all cycles are skipped and we exit with 1 (max_cycles).
#[test]
fn cycle_gate_skip_skips_phases_and_continues_to_next_cycle() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "false".to_string(),
        on_fail: Some(GateAction::Skip),
        timeout: None,
    };
    let workflow = make_workflow_with_cycle_gate(Some(gate), 3, 0);
    // No executor outputs: phases never run
    let executor = MockExecutor::new(vec![]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    // All 3 cycles were skipped, max_cycles reached
    assert_eq!(result.exit_code, 1, "should exit with max_cycles code");
    assert_eq!(result.total_runs, 0, "no phases should have run");
}

/// Cycle gate passes on first cycle but fails on second.
/// First cycle's phases execute; second cycle does not start.
#[test]
fn cycle_gate_passes_first_cycle_fails_second() {
    let dir = tempdir().unwrap();
    // We need a gate that passes once then fails. We'll use a file existence check.
    // Create a temp file, gate checks for it. Phase deletes the file.
    // On first cycle: file exists → gate passes → phase runs → phase deletes file
    // On second cycle: file doesn't exist → gate fails → stop
    let flag_file = dir.path().join("gate_flag");
    std::fs::write(&flag_file, "").unwrap();

    let gate = GateConfig {
        command: format!("test -f {}", flag_file.display()),
        on_fail: Some(GateAction::Stop),
        timeout: None,
    };

    let workflow = Workflow {
        completion_signal: "RINGS_DONE".to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: dir.path().to_string_lossy().to_string(),
        max_cycles: 5,
        output_dir: None,
        delay_between_runs: 0,
        delay_between_cycles: 0,
        executor: None,
        budget_cap_usd: None,
        timeout_per_run_secs: None,
        compiled_error_profile: default_compiled_error_profile(),
        quota_backoff: false,
        quota_backoff_delay: 0,
        quota_backoff_max_retries: 0,
        manifest_enabled: false,
        manifest_ignore: vec![],
        manifest_mtime_optimization: false,
        snapshot_cycles: false,
        compiled_cost_parser: rings::cost::CompiledCostParser::ClaudeCode,
        lock_name: None,
        cycle_gate: Some(gate),
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
            gate: None,
            gate_each_run: false,
        }],
    };

    // The executor run deletes the flag file
    let flag_file_clone = flag_file.clone();
    let executor = MockExecutor::with_side_effect(
        vec![ExecutorOutput {
            combined: "no signal".to_string(),
            exit_code: 0,
        }],
        move |_inv| {
            let _ = std::fs::remove_file(&flag_file_clone);
        },
    );
    let config = EngineConfig {
        output_dir: dir.path().join("output"),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0, "gate stop should exit with code 0");
    assert_eq!(
        result.total_runs, 1,
        "only one run should have executed (cycle 1)"
    );
    assert_eq!(result.completed_cycles, 1, "one cycle completed");
}
