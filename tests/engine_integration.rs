// Run with: cargo test --features testing
use rings::cancel::CancelState;
use rings::engine::{run_workflow, EngineConfig};
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::state;
use rings::workflow::{CompiledErrorProfile, CompletionSignalMode, PhaseConfig, Workflow};
use std::sync::Arc;
use tempfile::tempdir;

fn default_compiled_error_profile() -> CompiledErrorProfile {
    CompiledErrorProfile {
        quota_regexes: vec![],
        auth_regexes: vec![],
    }
}

fn make_workflow(signal: &str, phases: &[(&str, u32)], max_cycles: u32) -> Workflow {
    Workflow {
        completion_signal: signal.to_string(),
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
        phases: phases
            .iter()
            .map(|(name, runs)| PhaseConfig {
                name: name.to_string(),
                prompt: None,
                prompt_text: Some(format!("do work, signal={signal}")),
                runs_per_cycle: *runs,
                budget_cap_usd: None,
                timeout_per_run_secs: None,
                consumes: vec![],
                produces: vec![],
                produces_required: false,
            })
            .collect(),
    }
}

#[test]
fn engine_exits_zero_on_completion_signal() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("RINGS_DONE", &[("builder", 1)], 10);
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "working...".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "RINGS_DONE".to_string(),
            exit_code: 0,
        },
    ]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-run-id".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.completed_cycles, 2);
}

#[test]
fn engine_exits_one_when_max_cycles_reached() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("RINGS_DONE", &[("builder", 1)], 3);
    // Never emit signal
    let outputs: Vec<_> = (0..3)
        .map(|_| ExecutorOutput {
            combined: "no signal".to_string(),
            exit_code: 0,
        })
        .collect();
    let executor = MockExecutor::new(outputs);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-run-id".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 1);
}

#[test]
fn engine_writes_run_logs() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-run-id".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    run_workflow(&workflow, &executor, &config, None, None).unwrap();

    // Log file for run 1 must exist
    let log_path = dir.path().join("runs").join("001.log");
    assert!(
        log_path.exists(),
        "run log not written: {}",
        log_path.display()
    );
}

#[test]
fn engine_writes_costs_jsonl() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "Cost: $0.05\nDONE".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-run-id".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    run_workflow(&workflow, &executor, &config, None, None).unwrap();

    let costs_path = dir.path().join("costs.jsonl");
    assert!(costs_path.exists());
    let content = std::fs::read_to_string(&costs_path).unwrap();
    assert!(content.contains("\"cost_usd\""));
}

#[test]
fn engine_classifies_nonzero_exit_as_error() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "quota exceeded".to_string(),
        exit_code: 1,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-run-id".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 3);
}

#[test]
fn engine_saves_state_and_exits_130_on_cancel() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 10);
    // First run succeeds, second run triggers cancellation
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "run 1 output".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "run 2 output".to_string(),
            exit_code: 0,
        },
    ]);
    let cancel = Arc::new(CancelState::new());
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-run-id".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    // Signal cancellation immediately (test simplicity)
    cancel.signal_received();

    let result = run_workflow(&workflow, &executor, &config, None, Some(cancel)).unwrap();
    assert_eq!(
        result.exit_code, 130,
        "exit code should be 130 on cancellation"
    );

    // state.json must exist
    let state_path = dir.path().join("state.json");
    assert!(state_path.exists(), "state.json must be saved on cancel");

    // run_id must be populated correctly
    let state = state::StateFile::read(&state_path).unwrap();
    assert_eq!(
        state.run_id, "test-run-id",
        "run_id should match EngineConfig"
    );
}

#[test]
fn continue_signal_skips_remaining_phases_in_cycle() {
    let dir = tempdir().unwrap();
    // 3-phase workflow; cycle 1: phase_a emits continue_signal → phases b and c skipped.
    // Cycle 2: all 3 phases run; phase_c emits the completion signal.
    let workflow = Workflow {
        completion_signal: "DONE".to_string(),
        continue_signal: Some("SKIP_REST".to_string()),
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: ".".to_string(),
        max_cycles: 3,
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
        phases: vec![
            PhaseConfig {
                name: "phase_a".to_string(),
                prompt: None,
                prompt_text: Some("p".to_string()),
                runs_per_cycle: 1,
                budget_cap_usd: None,
                timeout_per_run_secs: None,
                consumes: vec![],
                produces: vec![],
                produces_required: false,
            },
            PhaseConfig {
                name: "phase_b".to_string(),
                prompt: None,
                prompt_text: Some("p".to_string()),
                runs_per_cycle: 1,
                budget_cap_usd: None,
                timeout_per_run_secs: None,
                consumes: vec![],
                produces: vec![],
                produces_required: false,
            },
            PhaseConfig {
                name: "phase_c".to_string(),
                prompt: None,
                prompt_text: Some("p".to_string()),
                runs_per_cycle: 1,
                budget_cap_usd: None,
                timeout_per_run_secs: None,
                consumes: vec![],
                produces: vec![],
                produces_required: false,
            },
        ],
    };
    let executor = MockExecutor::new(vec![
        // cycle 1: only phase_a runs, emits continue_signal
        ExecutorOutput {
            combined: "SKIP_REST".to_string(),
            exit_code: 0,
        },
        // cycle 2: all three phases run
        ExecutorOutput {
            combined: "no signal".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "no signal".to_string(),
            exit_code: 0,
        },
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
        run_id: "test-continue".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0, "should complete via DONE signal");
    // 4 total runs: 1 (cycle 1, phase_a) + 3 (cycle 2, all phases)
    assert_eq!(
        result.total_runs, 4,
        "phases b and c must be skipped in cycle 1"
    );
}

#[test]
fn completion_signal_phases_restricts_completion_to_named_phases() {
    let dir = tempdir().unwrap();
    // completion only fires from "synthesize", not "review"
    let workflow = Workflow {
        completion_signal: "DONE".to_string(),
        continue_signal: None,
        completion_signal_phases: vec!["synthesize".to_string()],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: ".".to_string(),
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
        phases: vec![
            PhaseConfig {
                name: "review".to_string(),
                prompt: None,
                prompt_text: Some("p".to_string()),
                runs_per_cycle: 1,
                budget_cap_usd: None,
                timeout_per_run_secs: None,
                consumes: vec![],
                produces: vec![],
                produces_required: false,
            },
            PhaseConfig {
                name: "synthesize".to_string(),
                prompt: None,
                prompt_text: Some("p".to_string()),
                runs_per_cycle: 1,
                budget_cap_usd: None,
                timeout_per_run_secs: None,
                consumes: vec![],
                produces: vec![],
                produces_required: false,
            },
        ],
    };
    // review emits DONE (should be ignored), then synthesize emits DONE (should fire)
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "DONE".to_string(), // review phase — must NOT trigger completion
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "DONE".to_string(), // synthesize phase — MUST trigger completion
            exit_code: 0,
        },
    ]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-phases".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);
    // Both review and synthesize ran: total_runs == 2
    assert_eq!(result.total_runs, 2);
}

#[test]
fn line_mode_completion_requires_signal_on_own_line() {
    let dir = tempdir().unwrap();
    let workflow = Workflow {
        completion_signal: "DONE".to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Line,
        context_dir: ".".to_string(),
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
        phases: vec![PhaseConfig {
            name: "builder".to_string(),
            prompt: None,
            prompt_text: Some("p".to_string()),
            runs_per_cycle: 1,
            budget_cap_usd: None,
            timeout_per_run_secs: None,
            consumes: vec![],
            produces: vec![],
            produces_required: false,
        }],
    };
    // First output: "DONE" embedded mid-line (should NOT fire in line mode)
    // Second output: "DONE" alone on its own line (SHOULD fire)
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "work: DONE, continuing".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "work done\nDONE\nbye".to_string(),
            exit_code: 0,
        },
    ]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-line-mode".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0, "should complete on second run");
    assert_eq!(result.total_runs, 2);
}

#[test]
fn engine_classifies_quota_error() {
    use regex::Regex;
    let dir = tempdir().unwrap();
    let mut workflow = make_workflow("RINGS_DONE", &[("builder", 1)], 10);

    // Set up error profile with quota patterns
    workflow.compiled_error_profile = CompiledErrorProfile {
        quota_regexes: vec![
            Regex::new("(?i)usage limit reached").unwrap(),
            Regex::new("(?i)quota exceeded").unwrap(),
        ],
        auth_regexes: vec![],
    };

    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "Error: quota exceeded on your account".to_string(),
        exit_code: 1,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-quota-error".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 3);
    assert_eq!(
        result.failure_reason,
        Some(state::FailureReason::Quota),
        "should classify as quota error"
    );
}

#[test]
fn engine_classifies_auth_error() {
    use regex::Regex;
    let dir = tempdir().unwrap();
    let mut workflow = make_workflow("RINGS_DONE", &[("builder", 1)], 10);

    // Set up error profile with auth patterns
    workflow.compiled_error_profile = CompiledErrorProfile {
        quota_regexes: vec![],
        auth_regexes: vec![
            Regex::new("(?i)unauthorized").unwrap(),
            Regex::new("(?i)invalid api key").unwrap(),
        ],
    };

    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "Error: unauthorized - invalid API key".to_string(),
        exit_code: 1,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-auth-error".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 3);
    assert_eq!(
        result.failure_reason,
        Some(state::FailureReason::Auth),
        "should classify as auth error"
    );
}

#[test]
fn engine_classifies_unknown_error() {
    use regex::Regex;
    let dir = tempdir().unwrap();
    let mut workflow = make_workflow("RINGS_DONE", &[("builder", 1)], 10);

    // Set up error profile with specific patterns
    workflow.compiled_error_profile = CompiledErrorProfile {
        quota_regexes: vec![Regex::new("(?i)quota").unwrap()],
        auth_regexes: vec![Regex::new("(?i)auth").unwrap()],
    };

    // Output that doesn't match any patterns
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "Error: something went wrong".to_string(),
        exit_code: 1,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-unknown-error".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 3);
    assert_eq!(
        result.failure_reason,
        Some(state::FailureReason::Unknown),
        "should classify as unknown error"
    );
}

#[test]
fn engine_saves_failure_reason_in_state() {
    use regex::Regex;
    let dir = tempdir().unwrap();
    let mut workflow = make_workflow("RINGS_DONE", &[("builder", 1)], 10);

    // Set up error profile with quota patterns
    workflow.compiled_error_profile = CompiledErrorProfile {
        quota_regexes: vec![Regex::new("(?i)quota exceeded").unwrap()],
        auth_regexes: vec![],
    };

    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "quota exceeded".to_string(),
        exit_code: 1,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-save-reason".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 3);

    // Verify failure_reason is saved in state.json
    let state_path = dir.path().join("state.json");
    let state = state::StateFile::read(&state_path).unwrap();
    assert_eq!(
        state.failure_reason,
        Some(state::FailureReason::Quota),
        "state.json should contain failure_reason"
    );
}

/// Engine with output_format: Jsonl runs to completion without panicking.
/// The actual stderr suppression is structural (guarded by output_format checks),
/// but we verify the engine produces correct results in JSONL mode.
#[test]
fn engine_jsonl_mode_runs_correctly() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-jsonl".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Jsonl,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(
        result.exit_code, 0,
        "JSONL mode should still exit 0 on completion"
    );
    assert_eq!(result.total_runs, 1);
}

/// Engine with output_format: Human runs to completion (regression check).
#[test]
fn engine_human_mode_runs_correctly() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-human".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(
        result.exit_code, 0,
        "Human mode should still exit 0 on completion"
    );
    assert_eq!(result.total_runs, 1);
}

/// Engine with JSONL mode and max_cycles reached returns exit code 1 correctly.
#[test]
fn engine_jsonl_mode_max_cycles() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 2);
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "no signal".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "no signal".to_string(),
            exit_code: 0,
        },
    ]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-jsonl-max".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Jsonl,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(
        result.exit_code, 1,
        "JSONL mode should exit 1 on max_cycles"
    );
}
