// Tests for cumulative token tracking in BudgetTracker and EngineResult (F-190).
// Run with: cargo test --features testing
use rings::audit::CostEntry;
use rings::engine::{run_workflow, BudgetTracker, EngineConfig};
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::workflow::{CompiledErrorProfile, CompletionSignalMode, PhaseConfig, Workflow};
use tempfile::tempdir;

fn default_compiled_error_profile() -> CompiledErrorProfile {
    CompiledErrorProfile {
        quota_regexes: vec![],
        auth_regexes: vec![],
    }
}

fn make_workflow(signal: &str) -> Workflow {
    Workflow {
        completion_signal: signal.to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: ".".to_string(),
        max_cycles: 10,
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
            prompt_text: Some("do work".to_string()),
            runs_per_cycle: 1,
            budget_cap_usd: None,
            timeout_per_run_secs: None,
            consumes: vec![],
            produces: vec![],
            produces_required: false,
        }],
    }
}

fn make_config(dir: &tempfile::TempDir) -> EngineConfig {
    EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-run-id".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
    }
}

/// Output with token data in the full pattern format.
fn run_output_with_tokens(input: u64, output: u64, signal: &str) -> String {
    format!(
        "doing work...\nCost: $0.01 ({} input tokens, {} output tokens)\n{}",
        input, output, signal
    )
}

/// Output with no token data (only cost).
fn run_output_no_tokens(signal: &str) -> String {
    format!("doing work...\nCost: $0.01\n{}", signal)
}

#[test]
fn cumulative_tokens_increment_across_runs() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("RINGS_DONE");
    // Two runs, each with 1000 input and 500 output tokens, then done.
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: run_output_with_tokens(1000, 500, "working"),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: run_output_with_tokens(2000, 800, "RINGS_DONE"),
            exit_code: 0,
        },
    ]);
    let config = make_config(&dir);
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.total_input_tokens, 3000); // 1000 + 2000
    assert_eq!(result.total_output_tokens, 1300); // 500 + 800
}

#[test]
fn runs_with_none_tokens_do_not_affect_totals() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("RINGS_DONE");
    // First run has no token data, second run has tokens, third signals done.
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: run_output_no_tokens("working"),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: run_output_with_tokens(500, 200, "working"),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: run_output_with_tokens(300, 100, "RINGS_DONE"),
            exit_code: 0,
        },
    ]);
    let config = make_config(&dir);
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();

    assert_eq!(result.exit_code, 0);
    // Only the last two runs have tokens
    assert_eq!(result.total_input_tokens, 800); // 0 + 500 + 300
    assert_eq!(result.total_output_tokens, 300); // 0 + 200 + 100
}

#[test]
fn reconstruct_from_costs_accumulates_token_totals() {
    let dir = tempdir().unwrap();
    let costs_path = dir.path().join("costs.jsonl");

    // Write three cost entries: two with tokens, one without.
    let entries: Vec<CostEntry> = vec![
        CostEntry {
            run: 1,
            cycle: 1,
            phase: "builder".to_string(),
            iteration: 1,
            cost_usd: Some(0.01),
            input_tokens: Some(1000),
            output_tokens: Some(400),
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
            iteration: 1,
            cost_usd: Some(0.02),
            input_tokens: None, // no tokens on this run
            output_tokens: None,
            cost_confidence: "partial".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        },
        CostEntry {
            run: 3,
            cycle: 2,
            phase: "builder".to_string(),
            iteration: 1,
            cost_usd: Some(0.03),
            input_tokens: Some(2500),
            output_tokens: Some(900),
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        },
    ];

    let mut jsonl = String::new();
    for e in &entries {
        jsonl.push_str(&serde_json::to_string(e).unwrap());
        jsonl.push('\n');
    }
    std::fs::write(&costs_path, &jsonl).unwrap();

    let tracker = BudgetTracker::reconstruct_from_costs(&costs_path).unwrap();

    assert_eq!(tracker.cumulative_input_tokens, 3500); // 1000 + 0 + 2500
    assert_eq!(tracker.cumulative_output_tokens, 1300); // 400 + 0 + 900
    assert!((tracker.cumulative_cost - 0.06).abs() < 1e-9);
}
