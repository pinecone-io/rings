// Phase gate integration tests.
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

fn no_signal_output() -> ExecutorOutput {
    ExecutorOutput {
        combined: "no signal".to_string(),
        exit_code: 0,
    }
}

fn make_phase(
    name: &str,
    gate: Option<GateConfig>,
    gate_each_run: bool,
    runs_per_cycle: u32,
) -> PhaseConfig {
    PhaseConfig {
        name: name.to_string(),
        prompt: None,
        prompt_text: Some("do work".to_string()),
        runs_per_cycle,
        budget_cap_usd: None,
        timeout_per_run_secs: None,
        consumes: vec![],
        produces: vec![],
        produces_required: false,
        executor: None,
        gate,
        gate_each_run,
    }
}

fn make_workflow(phases: Vec<PhaseConfig>, max_cycles: u32) -> Workflow {
    Workflow {
        completion_signal: "RINGS_DONE".to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: ".".to_string(),
        max_cycles,
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
        cycle_gate: None,
        phases,
    }
}

/// Phase with `gate = { command = "true" }` — phase runs normally.
#[test]
fn phase_gate_true_runs_normally() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "true".to_string(),
        on_fail: Some(GateAction::Skip),
        timeout: None,
    };
    let workflow = make_workflow(vec![make_phase("builder", Some(gate), false, 1)], 2);
    let executor = MockExecutor::new(vec![no_signal_output(), no_signal_output()]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 1, "should reach max_cycles");
    assert_eq!(result.total_runs, 2, "both cycles ran");
}

/// Phase with `gate = { command = "false" }` — phase skipped (default on_fail = skip).
/// Next phase still runs.
#[test]
fn phase_gate_false_skips_phase_default_on_fail() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "false".to_string(),
        on_fail: None, // default = skip
        timeout: None,
    };
    let workflow = make_workflow(
        vec![
            make_phase("skipped-phase", Some(gate), false, 1),
            make_phase("runs-phase", None, false, 1),
        ],
        1,
    );
    // Only "runs-phase" should execute (1 run)
    let executor = MockExecutor::new(vec![no_signal_output()]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 1, "max_cycles reached");
    assert_eq!(result.total_runs, 1, "only second phase ran");
}

/// Phase with `gate = { command = "false", on_fail = "stop" }` — workflow stops gracefully.
#[test]
fn phase_gate_stop_exits_zero() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "false".to_string(),
        on_fail: Some(GateAction::Stop),
        timeout: None,
    };
    let workflow = make_workflow(vec![make_phase("builder", Some(gate), false, 1)], 5);
    let executor = MockExecutor::new(vec![]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0, "stop exits with code 0");
    assert_eq!(result.total_runs, 0, "no runs should have executed");
}

/// Phase with `gate = { command = "false", on_fail = "error" }` — workflow exits with code 2.
#[test]
fn phase_gate_error_exits_code_2() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "false".to_string(),
        on_fail: Some(GateAction::Error),
        timeout: None,
    };
    let workflow = make_workflow(vec![make_phase("builder", Some(gate), false, 1)], 5);
    let executor = MockExecutor::new(vec![]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 2);
    assert_eq!(result.total_runs, 0, "no runs should have executed");
}

/// Phase with `runs_per_cycle = 3` and gate — gate checked once before first run (default).
#[test]
fn phase_gate_checked_once_for_runs_per_cycle() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "true".to_string(),
        on_fail: Some(GateAction::Skip),
        timeout: None,
    };
    let workflow = make_workflow(vec![make_phase("builder", Some(gate), false, 3)], 1);
    // All 3 runs in the cycle should execute
    let executor = MockExecutor::new(vec![
        no_signal_output(),
        no_signal_output(),
        no_signal_output(),
    ]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 1, "max_cycles reached");
    assert_eq!(result.total_runs, 3, "all 3 runs executed");
}

/// Phase with `runs_per_cycle = 3`, `gate_each_run = true`, gate always passes.
/// All 3 runs execute.
#[test]
fn phase_gate_each_run_all_pass() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "true".to_string(),
        on_fail: Some(GateAction::Skip),
        timeout: None,
    };
    let workflow = make_workflow(vec![make_phase("builder", Some(gate), true, 3)], 1);
    let executor = MockExecutor::new(vec![
        no_signal_output(),
        no_signal_output(),
        no_signal_output(),
    ]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.total_runs, 3, "all 3 runs executed");
}

/// Phase with `runs_per_cycle = 3`, `gate_each_run = true`, and a gate that fails on second check.
/// First run executes, second is skipped (remaining iterations of this phase are skipped).
#[test]
fn phase_gate_each_run_fails_on_second_check() {
    // Use a flag file: gate passes while file exists; phase deletes it on first run.
    let tmpdir = tempdir().unwrap();
    let flag_file = tmpdir.path().join("flag");
    std::fs::write(&flag_file, "").unwrap();

    let gate = GateConfig {
        command: format!("test -f {}", flag_file.display()),
        on_fail: Some(GateAction::Skip),
        timeout: None,
    };
    let workflow = Workflow {
        completion_signal: "RINGS_DONE".to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: tmpdir.path().to_string_lossy().to_string(),
        max_cycles: 1,
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
        cycle_gate: None,
        phases: vec![PhaseConfig {
            name: "builder".to_string(),
            prompt: None,
            prompt_text: Some("do work".to_string()),
            runs_per_cycle: 3,
            budget_cap_usd: None,
            timeout_per_run_secs: None,
            consumes: vec![],
            produces: vec![],
            produces_required: false,
            executor: None,
            gate: Some(gate),
            gate_each_run: true,
        }],
    };

    // Executor deletes the flag on the first run
    let flag_clone = flag_file.clone();
    let executor = MockExecutor::with_side_effect(
        vec![ExecutorOutput {
            combined: "no signal".to_string(),
            exit_code: 0,
        }],
        move |_inv| {
            let _ = std::fs::remove_file(&flag_clone);
        },
    );
    let config = EngineConfig {
        output_dir: tmpdir.path().join("output"),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    // Gate passes on run 1, fails on run 2 → runs 2 and 3 skipped → 1 total run
    assert_eq!(result.total_runs, 1, "only the first run executed");
    assert_eq!(result.exit_code, 1, "max_cycles reached");
}

/// Two phases: first has failing gate (skip), second has no gate — second phase still runs.
#[test]
fn two_phases_first_skipped_second_runs() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "false".to_string(),
        on_fail: Some(GateAction::Skip),
        timeout: None,
    };
    let workflow = make_workflow(
        vec![
            make_phase("phase-a", Some(gate), false, 1),
            make_phase("phase-b", None, false, 1),
        ],
        1,
    );
    // Only phase-b runs (1 run)
    let executor = MockExecutor::new(vec![no_signal_output()]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 1, "max_cycles reached");
    assert_eq!(result.total_runs, 1, "only phase-b ran");
}
