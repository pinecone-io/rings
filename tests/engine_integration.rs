// Run with: cargo test --features testing
use rings::audit::CostEntry;
use rings::cancel::CancelState;
use rings::engine::{run_workflow, EngineConfig, ResumePoint};
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
        compiled_cost_parser: rings::cost::CompiledCostParser::ClaudeCode,
        lock_name: None,
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
                executor: None,
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
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
        compiled_cost_parser: rings::cost::CompiledCostParser::ClaudeCode,
        lock_name: None,
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
                executor: None,
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
                executor: None,
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
                executor: None,
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
        strict_parsing: false,
        ..Default::default()
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
        compiled_cost_parser: rings::cost::CompiledCostParser::ClaudeCode,
        lock_name: None,
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
                executor: None,
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
                executor: None,
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
        strict_parsing: false,
        ..Default::default()
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
        compiled_cost_parser: rings::cost::CompiledCostParser::ClaudeCode,
        lock_name: None,
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
            executor: None,
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
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
        strict_parsing: false,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(
        result.exit_code, 1,
        "JSONL mode should exit 1 on max_cycles"
    );
}

// ─────────────────────────────────────────────────────────────────
// --strict-parsing tests
// ─────────────────────────────────────────────────────────────────

/// Full confidence → strict parsing doesn't halt, run completes normally.
#[test]
fn strict_parsing_full_confidence_continues() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    // Full-confidence output: "Cost: $X.XX (N input tokens, M output tokens)"
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "Cost: $0.05 (100 input tokens, 20 output tokens) DONE".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-strict-full".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: true,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0, "full confidence should not halt");
    assert_eq!(result.total_runs, 1);
}

/// Partial confidence → strict parsing doesn't halt, run completes normally.
#[test]
fn strict_parsing_partial_confidence_continues() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    // Partial-confidence output: "Cost: $X.XX" (no token counts)
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "Cost: $0.05 DONE".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-strict-partial".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: true,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0, "partial confidence should not halt");
    assert_eq!(result.total_runs, 1);
}

/// Low confidence → strict parsing halts, state saved, exit code 2.
#[test]
fn strict_parsing_low_confidence_halts() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    // Low-confidence output: generic "$X.XX" pattern only
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "spent $0.05 today DONE".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-strict-low".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: true,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(
        result.exit_code, 2,
        "low confidence should halt with exit 2"
    );

    // State should be saved
    let state_path = dir.path().join("state.json");
    assert!(
        state_path.exists(),
        "state.json should be saved on strict parsing halt"
    );
}

/// None confidence → strict parsing halts, state saved, exit code 2.
#[test]
fn strict_parsing_none_confidence_halts() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    // No cost info in output → None confidence
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "no cost info here DONE".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-strict-none".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: true,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(
        result.exit_code, 2,
        "none confidence should halt with exit 2"
    );

    // State should be saved
    let state_path = dir.path().join("state.json");
    assert!(
        state_path.exists(),
        "state.json should be saved on strict parsing halt"
    );
}

/// Without --strict-parsing, low confidence produces a warning but run continues.
#[test]
fn without_strict_parsing_low_confidence_continues() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    // Low-confidence output
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "spent $0.05 today DONE".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-no-strict-low".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(
        result.exit_code, 0,
        "without strict parsing, low confidence should not halt"
    );
    assert_eq!(result.total_runs, 1);
    // Should have a parse warning recorded
    assert_eq!(
        result.parse_warnings.len(),
        1,
        "should record parse warning"
    );
}

#[test]
fn cost_entry_written_after_each_successful_run() {
    // Verify that costs.jsonl is populated on success — this confirms the cost
    // append happens (timing is validated structurally in engine.rs).
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "Cost: $0.05\nno signal".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "Cost: $0.07\nDONE".to_string(),
            exit_code: 0,
        },
    ]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-cost-timing".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);

    let costs_path = dir.path().join("costs.jsonl");
    assert!(costs_path.exists(), "costs.jsonl must exist");
    let content = std::fs::read_to_string(&costs_path).unwrap();
    let entries: Vec<CostEntry> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    // Two runs, so two cost entries.
    assert_eq!(entries.len(), 2, "expected two cost entries");
    assert_eq!(entries[0].run, 1);
    assert_eq!(entries[1].run, 2);
}

#[test]
fn resume_deduplicates_duplicate_cost_entries() {
    // Simulate the crash scenario: cost was appended for run 1, but the process
    // was killed before the next state write. On resume, run 1 is re-executed
    // and a second cost entry for run 1 is appended. The reconstruction must
    // count run 1 only once.
    let dir = tempdir().unwrap();

    // Pre-populate costs.jsonl with run 1 appearing twice (duplicate).
    let costs_path = dir.path().join("costs.jsonl");
    let run1_cost = 0.05_f64;
    let dup_entry = CostEntry {
        run: 1,
        cycle: 1,
        phase: "builder".to_string(),
        iteration: 1,
        cost_usd: Some(run1_cost),
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
    let line = serde_json::to_string(&dup_entry).unwrap();
    // Write the same entry twice to simulate a duplicate.
    std::fs::write(&costs_path, format!("{}\n{}\n", line, line)).unwrap();

    // The workflow has 2 runs_per_cycle; we resume after run 1.
    let workflow = make_workflow("DONE", &[("builder", 2)], 1);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: format!("Cost: ${:.2}\nDONE", run1_cost),
        exit_code: 0,
    }]);
    let resume_point = ResumePoint {
        last_completed_run: 1,
        last_completed_cycle: 1,
        last_completed_phase_index: 0,
        last_completed_iteration: 1,
    };
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-dedup".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, Some(resume_point), None).unwrap();
    // total_cost_usd should be ~run1_cost (dedup) + run2_cost (~run1_cost from mock).
    // It must NOT be ~3 * run1_cost (which would happen if the duplicate was counted).
    assert!(
        result.total_cost_usd < run1_cost * 2.5,
        "duplicate cost entry should not be double-counted; got total_cost_usd = {}",
        result.total_cost_usd
    );
}

#[test]
fn resume_with_clean_costs_jsonl_accumulates_correctly() {
    // Normal resume (no duplicates): cumulative cost should be prior cost + new run cost.
    let dir = tempdir().unwrap();

    let costs_path = dir.path().join("costs.jsonl");
    let prior_cost = 0.05_f64;
    let prior_entry = CostEntry {
        run: 1,
        cycle: 1,
        phase: "builder".to_string(),
        iteration: 1,
        cost_usd: Some(prior_cost),
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
    let line = serde_json::to_string(&prior_entry).unwrap();
    std::fs::write(&costs_path, format!("{}\n", line)).unwrap();

    let run2_cost = 0.07_f64;
    let workflow = make_workflow("DONE", &[("builder", 2)], 1);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: format!("Cost: ${:.2}\nDONE", run2_cost),
        exit_code: 0,
    }]);
    let resume_point = ResumePoint {
        last_completed_run: 1,
        last_completed_cycle: 1,
        last_completed_phase_index: 0,
        last_completed_iteration: 1,
    };
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-clean-resume".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, Some(resume_point), None).unwrap();
    // Total cost = prior_cost (0.05) + run2_cost (0.07) ≈ 0.12, within float tolerance.
    let expected = prior_cost + run2_cost;
    assert!(
        (result.total_cost_usd - expected).abs() < 0.01,
        "expected total ~{}, got {}",
        expected,
        result.total_cost_usd
    );
}

// ── Step-through tests ────────────────────────────────────────────────────────

fn make_step_reader(input: &str) -> std::sync::Mutex<Box<dyn std::io::BufRead + Send>> {
    let cursor = std::io::Cursor::new(input.as_bytes().to_vec());
    std::sync::Mutex::new(Box::new(cursor))
}

/// --step with mock stdin "c\nc\nq\n": runs 3 runs then quits (exit 130).
/// (c after run 1, c after run 2, q after run 3 → cancellation)
#[test]
fn step_continue_continue_quit_exits_130() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 10);
    // Provide enough outputs; the 4th would never be reached.
    let executor = MockExecutor::new(
        (0..5)
            .map(|_| ExecutorOutput {
                combined: "no signal".to_string(),
                exit_code: 0,
            })
            .collect(),
    );
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-step-ccq".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        step: true,
        step_reader: Some(make_step_reader("c\nc\nq\n")),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 130, "expected cancellation exit code");
    assert_eq!(result.total_runs, 3, "expected 3 runs before quit");
}

/// --step with mock stdin "s\n": after first run in cycle, skips remaining
/// iterations of that cycle and advances to next cycle.
#[test]
fn step_skip_cycle_skips_remaining_iterations() {
    let dir = tempdir().unwrap();
    // Phase has 3 iterations per cycle, 2 cycles max.
    let workflow = make_workflow("DONE", &[("builder", 3)], 2);
    // After 's' skips cycle 1 after run 1, cycle 2 runs all 3.
    // step_reader has "s\n" for run 1, then EOF (defaults to continue) for remaining.
    let executor = MockExecutor::new(
        (0..4)
            .map(|_| ExecutorOutput {
                combined: "no signal".to_string(),
                exit_code: 0,
            })
            .collect(),
    );
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-step-skip".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        step: true,
        step_reader: Some(make_step_reader("s\n")),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    // Cycle 1: only 1 run (then 's' skips remainder). Cycle 2: 3 runs.
    assert_eq!(
        result.total_runs, 4,
        "expected 1 (cycle1) + 3 (cycle2) runs"
    );
    assert_eq!(result.exit_code, 1, "expected max_cycles exit");
}

/// --step-cycles only pauses at cycle boundaries, not between runs within a cycle.
/// With 2 phases per cycle and step_reader "q\n", the prompt fires after the
/// last run of cycle 1 (cycle boundary) and the user quits → exit 130 after 2 runs.
#[test]
fn step_cycles_only_pauses_at_cycle_boundary() {
    let dir = tempdir().unwrap();
    // 2 phases, 1 run each → cycle boundary after run 2.
    let workflow = make_workflow("DONE", &[("phase-a", 1), ("phase-b", 1)], 5);
    let executor = MockExecutor::new(
        (0..10)
            .map(|_| ExecutorOutput {
                combined: "no signal".to_string(),
                exit_code: 0,
            })
            .collect(),
    );
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-step-cycles".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        step_cycles: true,
        step_reader: Some(make_step_reader("q\n")),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    // phase-a (run 1) → no pause (not cycle end). phase-b (run 2) → cycle end
    // → prompt → 'q' → quit.
    assert_eq!(
        result.exit_code, 130,
        "expected cancellation at cycle boundary"
    );
    assert_eq!(
        result.total_runs, 2,
        "expected 2 runs (both phases of cycle 1)"
    );
}

/// Non-TTY mode: --step is silently ignored when step_reader is None and stderr
/// is not a TTY (test environments have no TTY). The workflow runs to completion.
#[test]
fn step_non_tty_silently_ignored() {
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
        output_dir: dir.path().to_path_buf(),
        run_id: "test-step-non-tty".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        // step=true but step_reader=None → TTY check will fail → no pause.
        step: true,
        step_reader: None,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    // Runs to max_cycles without any pausing.
    assert_eq!(
        result.exit_code, 1,
        "expected max_cycles exit (step silently ignored)"
    );
    assert_eq!(result.total_runs, 2);
}

/// The step summary captures cost and completion-signal status.
/// Verify the step prompt is reached and cost data is available (exit_code=130 proves
/// the prompt was reached and processed the 'q' input).
#[test]
fn step_summary_shows_cost_and_signal_status() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        // Include a cost line to verify cost parsing path works with step.
        combined: "Cost: $0.01\nno signal".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-step-summary".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        step: true,
        step_reader: Some(make_step_reader("q\n")),
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    // The step prompt was reached (proved by 'q' causing exit 130).
    assert_eq!(result.exit_code, 130);
    // Cost was tracked correctly.
    assert!(result.total_cost_usd > 0.0, "expected non-zero cost");
}

#[test]
fn manifest_diff_data_appears_in_costs_jsonl() {
    let context_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    // Create an existing file so the before-manifest has something.
    std::fs::write(context_dir.path().join("existing.txt"), "old content").unwrap();

    let ctx_path = context_dir.path().to_path_buf();
    let executor = MockExecutor::with_side_effect(
        vec![ExecutorOutput {
            combined: "RINGS_DONE".to_string(),
            exit_code: 0,
        }],
        move |_| {
            // Modify existing file and add a new one — simulates claude code changes.
            std::fs::write(ctx_path.join("existing.txt"), "modified content").unwrap();
            std::fs::write(ctx_path.join("new_file.rs"), "fn main() {}").unwrap();
        },
    );

    let workflow = Workflow {
        completion_signal: "RINGS_DONE".to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: context_dir.path().to_str().unwrap().to_string(),
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
        manifest_enabled: true,
        manifest_ignore: vec![],
        manifest_mtime_optimization: false,
        snapshot_cycles: false,
        compiled_cost_parser: rings::cost::CompiledCostParser::ClaudeCode,
        lock_name: None,
        phases: vec![PhaseConfig {
            name: "builder".to_string(),
            prompt: None,
            prompt_text: Some("build stuff".to_string()),
            runs_per_cycle: 1,
            budget_cap_usd: None,
            timeout_per_run_secs: None,
            consumes: vec![],
            produces: vec![],
            produces_required: false,
            executor: None,
        }],
    };

    let config = EngineConfig {
        output_dir: output_dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-diff".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
        ..Default::default()
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);

    let costs_path = output_dir.path().join("costs.jsonl");
    let content = std::fs::read_to_string(&costs_path).unwrap();
    let entry: CostEntry = serde_json::from_str(content.trim()).unwrap();

    // 1 modified (existing.txt) + 1 added (new_file.rs) = 2 changed
    assert_eq!(entry.files_modified, 1, "existing.txt should be modified");
    assert_eq!(entry.files_added, 1, "new_file.rs should be added");
    assert_eq!(entry.files_deleted, 0, "no files deleted");
    assert_eq!(entry.files_changed, 2, "total files changed should be 2");
}
