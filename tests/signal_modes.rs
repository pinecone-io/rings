// Run with: cargo test --features testing
use regex::Regex;
use rings::completion::{output_line_contains_signal, output_regex_matches_signal};
use rings::dry_run::DryRunPlan;
use rings::engine::{run_workflow, EngineConfig};
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::workflow::{CompiledErrorProfile, CompletionSignalMode, PhaseConfig, Workflow};
use std::str::FromStr;
use tempfile::tempdir;

// --- Unit tests for output_regex_matches_signal ---

#[test]
fn regex_matches_returns_true_on_match() {
    let re = Regex::new("DONE").unwrap();
    assert!(output_regex_matches_signal("task DONE here", &re));
}

#[test]
fn regex_matches_returns_false_on_no_match() {
    let re = Regex::new("DONE").unwrap();
    assert!(!output_regex_matches_signal("still working", &re));
}

#[test]
fn regex_anchored_pattern_matches_full_line() {
    let re = Regex::new("(?m)^DONE$").unwrap();
    assert!(output_regex_matches_signal(
        "first line\nDONE\nlast line",
        &re
    ));
    assert!(!output_regex_matches_signal("DONE_EXTRA", &re));
}

#[test]
fn regex_capture_group_returns_bool_no_panic() {
    let re = Regex::new("(DONE)").unwrap();
    // Just verify it returns a bool and doesn't panic
    let result = output_regex_matches_signal("task DONE", &re);
    assert!(result);
    let result2 = output_regex_matches_signal("no match", &re);
    assert!(!result2);
}

// --- Unit tests for line mode ---

#[test]
fn line_mode_matches_with_whitespace() {
    assert!(output_line_contains_signal("  DONE  ", "DONE"));
}

#[test]
fn line_mode_no_match_superstring() {
    assert!(!output_line_contains_signal("DONE_EXTRA", "DONE"));
}

#[test]
fn line_mode_crlf_output_matches() {
    assert!(output_line_contains_signal("DONE\r\n", "DONE"));
}

// --- Engine integration: regex mode exits 0 on match ---

fn default_compiled_error_profile() -> CompiledErrorProfile {
    CompiledErrorProfile {
        quota_regexes: vec![],
        auth_regexes: vec![],
    }
}

fn make_workflow_regex(signal: &str) -> (Workflow, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let re = Regex::new(signal).unwrap();
    let workflow = Workflow {
        completion_signal: signal.to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Regex(re),
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
            prompt_text: Some(format!("do work, signal={signal}")),
            runs_per_cycle: 1,
            budget_cap_usd: None,
            timeout_per_run_secs: None,
            consumes: vec![],
            produces: vec![],
            produces_required: false,
        }],
    };
    (workflow, dir)
}

#[test]
fn engine_regex_mode_exits_zero_on_match() {
    let (workflow, dir) = make_workflow_regex("DONE|COMPLETE");
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "working...".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "task COMPLETE".to_string(),
            exit_code: 0,
        },
    ]);
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test-regex-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
        ..Default::default()
    };
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);
}

// --- continue_signal uses substring even when completion_signal_mode = regex ---

#[test]
fn continue_signal_uses_substring_not_regex() {
    // completion_signal_mode = regex with pattern "DONE"
    // continue_signal = "SKIP"
    // First run emits "SKIP" → should skip to next cycle (substring match)
    // Second run emits "DONE" → regex matches → exit 0
    let dir = tempdir().unwrap();
    let re = Regex::new("DONE").unwrap();
    let workflow = Workflow {
        completion_signal: "DONE".to_string(),
        continue_signal: Some("SKIP".to_string()),
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Regex(re),
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
                name: "builder".to_string(),
                prompt: None,
                prompt_text: Some("do work".to_string()),
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
                prompt_text: Some("review work".to_string()),
                runs_per_cycle: 1,
                budget_cap_usd: None,
                timeout_per_run_secs: None,
                consumes: vec![],
                produces: vec![],
                produces_required: false,
            },
        ],
    };

    // Cycle 1: builder emits SKIP → skips reviewer
    // Cycle 2: builder emits nothing, reviewer emits DONE → exits 0
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "SKIP".to_string(),
            exit_code: 0,
        },
        // Cycle 2: builder
        ExecutorOutput {
            combined: "still working".to_string(),
            exit_code: 0,
        },
        // Cycle 2: reviewer
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
    assert_eq!(result.exit_code, 0);
}

// --- dry_run: regex mode, signal found in prompt → SignalCheck { found: true } ---

#[test]
fn dry_run_regex_mode_signal_in_prompt() {
    use std::fs;
    let temp = tempdir().unwrap();
    let context_dir = temp.path().join("context");
    fs::create_dir_all(&context_dir).unwrap();

    let toml = format!(
        r#"
[workflow]
completion_signal = "DONE"
context_dir = "{}"
max_cycles = 3
completion_signal_mode = "regex"

[[phases]]
name = "builder"
prompt_text = "Do the work. When done print DONE."
"#,
        context_dir.display()
    );

    let workflow = Workflow::from_str(&toml).unwrap();
    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();
    let phase = &plan.phases[0];
    assert!(phase.signal_check.found, "DONE is in the prompt text");
}
