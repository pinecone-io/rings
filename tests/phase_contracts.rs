// Run with: cargo test --features testing
use rings::contracts::{
    check_consumes_at_startup, check_consumes_pre_run, check_produces_after_run, non_glob_prefix,
    ContractWarning,
};
use rings::engine::{run_workflow, EngineConfig};
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::workflow::{CompiledErrorProfile, CompletionSignalMode, PhaseConfig, Workflow};
use tempfile::tempdir;

fn default_error_profile() -> CompiledErrorProfile {
    CompiledErrorProfile {
        quota_regexes: vec![],
        auth_regexes: vec![],
    }
}

fn make_workflow_with_contracts(
    context_dir: &str,
    produces: Vec<String>,
    produces_required: bool,
    manifest_enabled: bool,
) -> Workflow {
    Workflow {
        completion_signal: "DONE".to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: context_dir.to_string(),
        max_cycles: 3,
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
        manifest_enabled,
        manifest_ignore: vec![],
        manifest_mtime_optimization: false,
        snapshot_cycles: false,
        phases: vec![PhaseConfig {
            name: "builder".to_string(),
            prompt: None,
            prompt_text: Some("build the code".to_string()),
            runs_per_cycle: 1,
            budget_cap_usd: None,
            timeout_per_run_secs: None,
            consumes: vec![],
            produces,
            produces_required,
        }],
    }
}

// --- Unit tests for non_glob_prefix ---

#[test]
fn non_glob_prefix_path_before_glob() {
    assert_eq!(non_glob_prefix("src/**/*.rs"), "src/");
}

#[test]
fn non_glob_prefix_no_metachar_returns_full() {
    assert_eq!(non_glob_prefix("review-notes.md"), "review-notes.md");
}

#[test]
fn non_glob_prefix_leading_star_returns_empty() {
    assert_eq!(non_glob_prefix("*.rs"), "");
}

// --- Unit tests for check_consumes_at_startup ---

#[test]
fn check_consumes_at_startup_file_exists_no_warning() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("review-notes.md"), "content").unwrap();
    let warnings = check_consumes_at_startup(
        "reviewer",
        &["review-notes.md".to_string()],
        dir.path(),
        "some prompt text",
    )
    .unwrap();
    assert!(warnings.is_empty(), "Expected no warnings when file exists");
}

#[test]
fn check_consumes_at_startup_no_files_prompt_contains_prefix_no_warning() {
    let dir = tempdir().unwrap();
    // No files matching src/**/*.rs, but prompt contains "src/"
    let warnings = check_consumes_at_startup(
        "builder",
        &["src/**/*.rs".to_string()],
        dir.path(),
        "Please read all files in src/ and improve them.",
    )
    .unwrap();
    assert!(
        warnings.is_empty(),
        "Expected no warning when prefix found in prompt"
    );
}

#[test]
fn check_consumes_at_startup_no_files_no_prefix_in_prompt_warning_fires() {
    let dir = tempdir().unwrap();
    let warnings = check_consumes_at_startup(
        "reviewer",
        &["review-notes.md".to_string()],
        dir.path(),
        "Do some review work.",
    )
    .unwrap();
    assert_eq!(warnings.len(), 1, "Expected one warning");
    let msg = match &warnings[0] {
        ContractWarning::ConsumesNoMatchStartup { phase, pattern, .. } => {
            assert_eq!(phase, "reviewer");
            assert_eq!(pattern, "review-notes.md");
            warnings[0].format_message()
        }
        _ => panic!("Expected ConsumesNoMatchStartup"),
    };
    assert!(msg.contains("Phase \"reviewer\""));
    assert!(msg.contains("review-notes.md"));
    assert!(msg.contains("--no-contract-check"));
}

// --- Unit tests for check_consumes_pre_run ---

#[test]
fn check_consumes_pre_run_no_files_emits_warning() {
    let dir = tempdir().unwrap();
    let warnings = check_consumes_pre_run(
        "reviewer",
        &["review-notes.md".to_string()],
        dir.path(),
        2,
        5,
    )
    .unwrap();
    assert_eq!(warnings.len(), 1, "Expected one pre-run warning");
    let msg = warnings[0].format_message();
    assert!(msg.contains("Phase \"reviewer\""));
    assert!(msg.contains("run 5, cycle 2"));
    assert!(msg.contains("review-notes.md"));
}

// --- Unit tests for check_produces_after_run ---

#[test]
fn check_produces_after_run_matched_added_no_violation() {
    let violations = check_produces_after_run(
        &["src/**/*.rs".to_string()],
        &["src/main.rs".to_string()],
        &[],
    );
    assert!(violations.is_empty());
}

#[test]
fn check_produces_after_run_empty_diff_returns_violation() {
    let violations = check_produces_after_run(&["src/**/*.rs".to_string()], &[], &[]);
    assert_eq!(violations, vec!["src/**/*.rs"]);
}

#[test]
fn check_produces_after_run_deleted_only_returns_violation() {
    let violations = check_produces_after_run(
        &["src/**/*.rs".to_string()],
        &[],
        &[], // No added or modified — deleted doesn't count
    );
    assert_eq!(violations, vec!["src/**/*.rs"]);
}

#[test]
fn check_produces_after_run_empty_produces_always_empty() {
    let violations = check_produces_after_run(&[], &["src/main.rs".to_string()], &[]);
    assert!(violations.is_empty());
}

#[test]
fn check_produces_after_run_partial_match_returns_unmatched_only() {
    let violations = check_produces_after_run(
        &["src/**/*.rs".to_string(), "tests/**/*.rs".to_string()],
        &["src/main.rs".to_string()],
        &[],
    );
    // src/**/*.rs matched, tests/**/*.rs did not
    assert_eq!(violations, vec!["tests/**/*.rs"]);
}

// --- Engine integration tests ---

#[test]
fn engine_produces_violations_populated_when_no_match() {
    let context_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    // Write a file to scan as "before" state
    std::fs::write(context_dir.path().join("existing.txt"), "existing").unwrap();

    let mut workflow = make_workflow_with_contracts(
        context_dir.path().to_str().unwrap(),
        vec!["src/**/*.rs".to_string()],
        false,
        true,
    );
    // Must emit DONE to actually exit (otherwise max_cycles=3 needed)
    workflow.phases[0].prompt_text = Some("build the code".to_string());
    workflow.max_cycles = 1;

    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);

    let config = EngineConfig {
        output_dir: output_dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-contracts-violations".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        no_contract_check: false,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    // Should exit with 0 (completion signal)
    assert_eq!(result.exit_code, 0);

    // Check costs.jsonl has produces_violations populated
    let costs_path = output_dir.path().join("costs.jsonl");
    let content = std::fs::read_to_string(&costs_path).unwrap();
    let entry: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    let violations = entry["produces_violations"].as_array().unwrap();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].as_str().unwrap(), "src/**/*.rs");
}

#[test]
fn engine_produces_violations_empty_when_matched() {
    let context_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    // Start with an existing file so manifest baseline is established
    std::fs::write(context_dir.path().join("existing.txt"), "old").unwrap();

    let mut workflow = make_workflow_with_contracts(
        context_dir.path().to_str().unwrap(),
        vec!["*.txt".to_string()],
        false,
        true,
    );
    workflow.max_cycles = 1;

    // Executor will write a file in context_dir and emit DONE
    let ctx_dir_path = context_dir.path().to_path_buf();
    let executor = MockExecutor::with_side_effect(
        vec![ExecutorOutput {
            combined: "DONE".to_string(),
            exit_code: 0,
        }],
        move |_| {
            std::fs::write(ctx_dir_path.join("output.txt"), "new file").unwrap();
        },
    );

    let config = EngineConfig {
        output_dir: output_dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-contracts-matched".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        no_contract_check: false,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);

    let costs_path = output_dir.path().join("costs.jsonl");
    let content = std::fs::read_to_string(&costs_path).unwrap();
    let entry: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    let violations = entry["produces_violations"].as_array().unwrap();
    assert!(
        violations.is_empty(),
        "Expected no violations when file was produced"
    );
}

#[test]
fn engine_produces_required_exits_two_when_no_match() {
    let context_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    std::fs::write(context_dir.path().join("existing.txt"), "existing").unwrap();

    let mut workflow = make_workflow_with_contracts(
        context_dir.path().to_str().unwrap(),
        vec!["src/**/*.rs".to_string()],
        true, // produces_required
        true,
    );
    // Never emits DONE — would exhaust cycles if not for hard exit
    workflow.max_cycles = 5;

    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "working".to_string(),
            exit_code: 0,
        };
        5
    ]);

    let config = EngineConfig {
        output_dir: output_dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-contracts-required".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        no_contract_check: false,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(
        result.exit_code, 2,
        "Expected exit code 2 for produces_required failure"
    );

    // State should be saved
    let state_path = output_dir.path().join("state.json");
    assert!(
        state_path.exists(),
        "State file should be saved on produces_required exit"
    );
}

#[test]
fn engine_produces_required_false_advisory_continues() {
    let context_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    std::fs::write(context_dir.path().join("existing.txt"), "existing").unwrap();

    let mut workflow = make_workflow_with_contracts(
        context_dir.path().to_str().unwrap(),
        vec!["src/**/*.rs".to_string()],
        false, // advisory only
        true,
    );
    workflow.max_cycles = 1;

    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);

    let config = EngineConfig {
        output_dir: output_dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-contracts-advisory".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        no_contract_check: false,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    // Should still exit 0 (completion signal fires), advisory warning doesn't block
    assert_eq!(result.exit_code, 0);
}

#[test]
fn engine_manifest_disabled_skips_produces_check() {
    let context_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    let mut workflow = make_workflow_with_contracts(
        context_dir.path().to_str().unwrap(),
        vec!["src/**/*.rs".to_string()],
        false,
        false, // manifest_enabled = false
    );
    workflow.max_cycles = 1;

    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);

    let config = EngineConfig {
        output_dir: output_dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-contracts-no-manifest".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        no_contract_check: false,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);

    // produces_violations should be [] even though no files were produced
    let costs_path = output_dir.path().join("costs.jsonl");
    let content = std::fs::read_to_string(&costs_path).unwrap();
    let entry: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    let violations = entry["produces_violations"].as_array().unwrap();
    assert!(
        violations.is_empty(),
        "Expected [] violations when manifest disabled"
    );
}

#[test]
fn engine_no_contract_check_suppresses_produces() {
    let context_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    std::fs::write(context_dir.path().join("existing.txt"), "existing").unwrap();

    let mut workflow = make_workflow_with_contracts(
        context_dir.path().to_str().unwrap(),
        vec!["src/**/*.rs".to_string()],
        false,
        true,
    );
    workflow.max_cycles = 1;

    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);

    let config = EngineConfig {
        output_dir: output_dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-no-contract-check".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        no_contract_check: true, // suppress all contract checks
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);

    // With --no-contract-check, produces_violations should be [] (check was skipped)
    let costs_path = output_dir.path().join("costs.jsonl");
    let content = std::fs::read_to_string(&costs_path).unwrap();
    let entry: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    let violations = entry["produces_violations"].as_array().unwrap();
    assert!(
        violations.is_empty(),
        "Expected [] with --no-contract-check"
    );
}

// OD-3: --no-completion-check and --no-contract-check are fully independent flags.
// --no-completion-check suppresses completion signal checking only; it does NOT suppress
// contract checks. The engine's EngineConfig has no no_completion_check field at all —
// contract check suppression is controlled solely by no_contract_check.
// This test verifies that with no_contract_check=false, contract violations are recorded
// even when no completion-check suppression is in effect (simulating --no-completion-check
// without --no-contract-check).
#[test]
fn engine_no_completion_check_does_not_suppress_contract_warnings() {
    let context_dir = tempdir().unwrap();
    let output_dir = tempdir().unwrap();

    std::fs::write(context_dir.path().join("existing.txt"), "existing").unwrap();

    let mut workflow = make_workflow_with_contracts(
        context_dir.path().to_str().unwrap(),
        vec!["src/**/*.rs".to_string()],
        false,
        true,
    );
    workflow.max_cycles = 1;

    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);

    // no_contract_check=false simulates: --no-completion-check passed but NOT --no-contract-check.
    // Per OD-3, these flags are fully independent. Contract checks must still fire.
    let config = EngineConfig {
        output_dir: output_dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-no-completion-check-independence".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        no_contract_check: false, // --no-completion-check does NOT set this
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);

    // Contract check is active (no_contract_check=false), so violations must be recorded.
    let costs_path = output_dir.path().join("costs.jsonl");
    let content = std::fs::read_to_string(&costs_path).unwrap();
    let entry: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    let violations = entry["produces_violations"].as_array().unwrap();
    assert_eq!(
        violations.len(),
        1,
        "--no-completion-check must not suppress contract warnings (OD-3: flags are independent)"
    );
    assert_eq!(violations[0].as_str().unwrap(), "src/**/*.rs");
}
