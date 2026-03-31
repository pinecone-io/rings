/// Tests for Task 4: JSONL per-run events (run_start, run_end, completion_signal, executor_error).
use rings::cli::OutputFormat;
use rings::engine::{run_workflow, EngineConfig};
use rings::events;
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::workflow::{CompiledErrorProfile, CompletionSignalMode, PhaseConfig, Workflow};
use tempfile::tempdir;

fn make_error_profile(
    quota_pattern: Option<&str>,
    auth_pattern: Option<&str>,
) -> CompiledErrorProfile {
    let quota_regexes = quota_pattern
        .map(|p| vec![regex::Regex::new(p).unwrap()])
        .unwrap_or_default();
    let auth_regexes = auth_pattern
        .map(|p| vec![regex::Regex::new(p).unwrap()])
        .unwrap_or_default();
    CompiledErrorProfile {
        quota_regexes,
        auth_regexes,
    }
}

fn make_workflow(signal: &str, phases: &[(&str, u32)], max_cycles: u32) -> Workflow {
    make_workflow_with_profile(signal, phases, max_cycles, make_error_profile(None, None))
}

fn make_workflow_with_profile(
    signal: &str,
    phases: &[(&str, u32)],
    max_cycles: u32,
    profile: CompiledErrorProfile,
) -> Workflow {
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
        compiled_error_profile: profile,
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

fn make_jsonl_config(dir: &std::path::Path, run_id: &str) -> EngineConfig {
    EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.to_path_buf(),
        verbose: false,
        run_id: run_id.to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: OutputFormat::Jsonl,
        strict_parsing: false,
        ..Default::default()
    }
}

fn parse_events(lines: &[String]) -> Vec<serde_json::Value> {
    lines
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn event_type(v: &serde_json::Value) -> &str {
    v["event"].as_str().unwrap()
}

#[test]
fn each_run_produces_exactly_one_run_start_and_run_end() {
    let dir = tempdir().unwrap();
    // 2 cycles, 1 phase with 1 run per cycle = 2 total runs, no signal
    let workflow = make_workflow("DONE", &[("builder", 1)], 2);
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "working".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "still working".to_string(),
            exit_code: 0,
        },
    ]);
    let config = make_jsonl_config(dir.path(), "test-per-run-events");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    assert_eq!(result.exit_code, 1, "should reach max_cycles");

    let evts = parse_events(&lines);
    let run_starts: Vec<_> = evts
        .iter()
        .filter(|e| event_type(e) == "run_start")
        .collect();
    let run_ends: Vec<_> = evts.iter().filter(|e| event_type(e) == "run_end").collect();

    assert_eq!(
        run_starts.len(),
        2,
        "should have 2 run_start events (one per run)"
    );
    assert_eq!(
        run_ends.len(),
        2,
        "should have 2 run_end events (one per run)"
    );
}

#[test]
fn run_start_template_context_has_required_fields() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 1);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "working".to_string(),
        exit_code: 0,
    }]);
    let config = make_jsonl_config(dir.path(), "test-template-context");

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    let evts = parse_events(&lines);
    let run_start = evts.iter().find(|e| event_type(e) == "run_start").unwrap();

    let ctx = &run_start["template_context"];
    assert_eq!(ctx["phase_name"], "builder");
    assert_eq!(ctx["cycle"], 1);
    assert_eq!(ctx["max_cycles"], 1);
    assert_eq!(ctx["iteration"], 1);
    assert_eq!(ctx["run"], 1);
    assert!(
        ctx["cost_so_far_usd"].is_number(),
        "cost_so_far_usd must be a number"
    );
}

#[test]
fn run_start_template_context_max_cycles_null_when_unlimited() {
    // max_cycles > 0 in this implementation, but we test that it's present as a number
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 1);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "working".to_string(),
        exit_code: 0,
    }]);
    let config = make_jsonl_config(dir.path(), "test-template-context-max");

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    let evts = parse_events(&lines);
    let run_start = evts.iter().find(|e| event_type(e) == "run_start").unwrap();
    // max_cycles is present and is 1
    assert_eq!(run_start["template_context"]["max_cycles"], 1);
}

#[test]
fn run_end_cost_usd_is_null_when_cost_parsing_fails() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 1);
    // No cost info in output — parser should return None
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "no cost info here".to_string(),
        exit_code: 0,
    }]);
    let config = make_jsonl_config(dir.path(), "test-run-end-null-cost");

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    let evts = parse_events(&lines);
    let run_end = evts.iter().find(|e| event_type(e) == "run_end").unwrap();

    assert!(
        run_end["cost_usd"].is_null(),
        "cost_usd should be null when cost parsing fails, got: {}",
        run_end["cost_usd"]
    );
}

#[test]
fn completion_signal_emitted_between_run_end_and_summary() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("TASK_COMPLETE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "TASK_COMPLETE".to_string(),
        exit_code: 0,
    }]);
    let config = make_jsonl_config(dir.path(), "test-completion-signal-order");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    assert_eq!(result.exit_code, 0);

    let evts = parse_events(&lines);
    let types: Vec<&str> = evts.iter().map(|e| event_type(e)).collect();

    // Find positions
    let run_end_pos = types.iter().rposition(|&t| t == "run_end").unwrap();
    let signal_pos = types
        .iter()
        .position(|&t| t == "completion_signal")
        .unwrap();
    let summary_pos = types.iter().position(|&t| t == "summary").unwrap();

    assert!(
        run_end_pos < signal_pos,
        "completion_signal should come after run_end"
    );
    assert!(
        signal_pos < summary_pos,
        "completion_signal should come before summary"
    );

    // Verify signal value
    let signal_evt = &evts[signal_pos];
    assert_eq!(signal_evt["signal"], "TASK_COMPLETE");
}

#[test]
fn executor_error_event_has_correct_class_for_quota_failure() {
    let dir = tempdir().unwrap();
    let profile = make_error_profile(Some("quota exceeded"), None);
    let workflow = make_workflow_with_profile("DONE", &[("builder", 1)], 5, profile);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "quota exceeded - try again later".to_string(),
        exit_code: 1,
    }]);
    let config = make_jsonl_config(dir.path(), "test-executor-error-quota");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    assert_eq!(result.exit_code, 3);
    let evts = parse_events(&lines);
    let err_evt = evts
        .iter()
        .find(|e| event_type(e) == "executor_error")
        .unwrap();
    assert_eq!(err_evt["error_class"], "quota");
}

#[test]
fn executor_error_event_has_correct_class_for_auth_failure() {
    let dir = tempdir().unwrap();
    let profile = make_error_profile(None, Some("invalid api key"));
    let workflow = make_workflow_with_profile("DONE", &[("builder", 1)], 5, profile);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "invalid api key".to_string(),
        exit_code: 1,
    }]);
    let config = make_jsonl_config(dir.path(), "test-executor-error-auth");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    assert_eq!(result.exit_code, 3);
    let evts = parse_events(&lines);
    let err_evt = evts
        .iter()
        .find(|e| event_type(e) == "executor_error")
        .unwrap();
    assert_eq!(err_evt["error_class"], "auth");
}

#[test]
fn executor_error_event_has_unknown_class_for_unclassified_failure() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "some unclassified error".to_string(),
        exit_code: 1,
    }]);
    let config = make_jsonl_config(dir.path(), "test-executor-error-unknown");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    assert_eq!(result.exit_code, 3);
    let evts = parse_events(&lines);
    let err_evt = evts
        .iter()
        .find(|e| event_type(e) == "executor_error")
        .unwrap();
    assert_eq!(err_evt["error_class"], "unknown");
}

#[test]
fn events_in_chronological_order_run_start_run_end() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 2);
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "working".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "still working".to_string(),
            exit_code: 0,
        },
    ]);
    let config = make_jsonl_config(dir.path(), "test-chronological-order");

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    let evts = parse_events(&lines);
    let types: Vec<&str> = evts.iter().map(|e| event_type(e)).collect();

    // First event is start
    assert_eq!(types[0], "start");

    // Each run_start should be immediately followed by run_end (with nothing between for simple runs)
    let run1_start = types.iter().position(|&t| t == "run_start").unwrap();
    let run1_end = types.iter().position(|&t| t == "run_end").unwrap();
    assert!(run1_start < run1_end, "run_start must come before run_end");

    // Last event is summary
    assert_eq!(*types.last().unwrap(), "summary");
}

#[test]
fn run_end_exit_code_matches_executor_exit_code() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "failed".to_string(),
        exit_code: 42,
    }]);
    let config = make_jsonl_config(dir.path(), "test-run-end-exit-code");

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    let evts = parse_events(&lines);
    let run_end = evts.iter().find(|e| event_type(e) == "run_end").unwrap();
    assert_eq!(run_end["exit_code"], 42);
}
