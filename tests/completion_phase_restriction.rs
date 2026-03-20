// Run with: cargo test --features testing
use rings::engine::{run_workflow, EngineConfig};
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::workflow::{CompiledErrorProfile, CompletionSignalMode, PhaseConfig, Workflow};
use std::str::FromStr;
use tempfile::tempdir;

fn default_error_profile() -> CompiledErrorProfile {
    CompiledErrorProfile {
        quota_regexes: vec![],
        auth_regexes: vec![],
    }
}

fn two_phase_workflow(completion_signal_phases: Vec<String>, max_cycles: u32) -> Workflow {
    Workflow {
        completion_signal: "DONE".to_string(),
        continue_signal: None,
        completion_signal_phases,
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: ".".to_string(),
        max_cycles,
        output_dir: None,
        delay_between_runs: 0,
        delay_between_cycles: 0,
        executor: None,
        budget_cap_usd: None,
        timeout_per_run_secs: None,
        compiled_error_profile: default_error_profile(),
        quota_backoff: false,
        quota_backoff_delay: 0,
        quota_backoff_max_retries: 0,
        manifest_enabled: false,
        manifest_ignore: vec![],
        manifest_mtime_optimization: false,
        snapshot_cycles: false,
        phases: vec![
            PhaseConfig {
                name: "builder".to_string(),
                prompt: None,
                prompt_text: Some("build the thing".to_string()),
                runs_per_cycle: 1,
                budget_cap_usd: None,
                timeout_per_run_secs: None,
                consumes: vec![],
                produces: vec![],
                produces_required: false,
            },
            PhaseConfig {
                name: "reviewer".to_string(),
                prompt: None,
                prompt_text: Some("review the thing".to_string()),
                runs_per_cycle: 1,
                budget_cap_usd: None,
                timeout_per_run_secs: None,
                consumes: vec![],
                produces: vec![],
                produces_required: false,
            },
        ],
    }
}

// Validate that an unknown phase name in completion_signal_phases fails at parse time.
#[test]
fn unknown_phase_in_completion_signal_phases_fails_at_parse() {
    let toml = r#"
[workflow]
completion_signal = "DONE"
context_dir = "."
max_cycles = 5
completion_signal_phases = ["nonexistent"]

[[phases]]
name = "builder"
prompt_text = "build"
"#;
    let result = Workflow::from_str(toml);
    assert!(result.is_err(), "expected Err for unknown phase, got Ok");
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("nonexistent"),
        "error should mention the unknown phase name: {msg}"
    );
}

// Two-phase workflow with completion_signal_phases = ["reviewer"].
// Cycle 1: builder emits DONE → run does NOT exit (builder is ineligible).
//           reviewer emits nothing → cycle ends.
// Cycle 2: builder emits nothing, reviewer emits DONE → exits 0.
#[test]
fn phase_restriction_builder_signal_ignored_reviewer_signal_exits_zero() {
    let dir = tempdir().unwrap();
    let workflow = two_phase_workflow(vec!["reviewer".to_string()], 5);

    let executor = MockExecutor::new(vec![
        // Cycle 1: builder emits DONE (should be ignored — builder is ineligible)
        ExecutorOutput {
            combined: "DONE".to_string(),
            exit_code: 0,
        },
        // Cycle 1: reviewer emits no signal
        ExecutorOutput {
            combined: "still reviewing".to_string(),
            exit_code: 0,
        },
        // Cycle 2: builder normal output
        ExecutorOutput {
            combined: "building".to_string(),
            exit_code: 0,
        },
        // Cycle 2: reviewer emits DONE → exits 0
        ExecutorOutput {
            combined: "DONE".to_string(),
            exit_code: 0,
        },
    ]);

    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-phase-restriction".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);
    // Completed on cycle 2, not cycle 1
    assert_eq!(result.completed_cycles, 2);
}

// When completion_eligible is false for the emitting phase, the run continues to
// max_cycles without completing. The signal output is still present in run logs
// (captured by the executor output) but does not trigger exit 0.
#[test]
fn phase_restriction_ineligible_signal_does_not_complete() {
    let dir = tempdir().unwrap();
    // Only reviewer is eligible; builder emits DONE every cycle; reviewer never emits it.
    let workflow = two_phase_workflow(vec!["reviewer".to_string()], 2);

    let executor = MockExecutor::new(vec![
        // Cycle 1: builder emits DONE (ineligible)
        ExecutorOutput {
            combined: "DONE".to_string(),
            exit_code: 0,
        },
        // Cycle 1: reviewer does not emit signal
        ExecutorOutput {
            combined: "no signal here".to_string(),
            exit_code: 0,
        },
        // Cycle 2: builder emits DONE again (ineligible)
        ExecutorOutput {
            combined: "DONE".to_string(),
            exit_code: 0,
        },
        // Cycle 2: reviewer still no signal
        ExecutorOutput {
            combined: "still reviewing".to_string(),
            exit_code: 0,
        },
    ]);

    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-ineligible-phase".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    // Exhausted max_cycles without completion → exit 1
    assert_eq!(result.exit_code, 1);
}

// Empty completion_signal_phases → any phase can trigger completion.
#[test]
fn empty_completion_signal_phases_any_phase_can_complete() {
    let dir = tempdir().unwrap();
    // No restriction — builder can complete
    let workflow = two_phase_workflow(vec![], 5);

    let executor = MockExecutor::new(vec![
        // Cycle 1: builder emits DONE immediately
        ExecutorOutput {
            combined: "DONE".to_string(),
            exit_code: 0,
        },
    ]);

    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-no-restriction".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.completed_cycles, 1);
}
