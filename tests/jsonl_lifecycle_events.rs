/// Tests for Task 3: JSONL lifecycle events (start, summary, fatal_error).
use rings::cancel::CancelState;
use rings::cli::OutputFormat;
use rings::engine::{run_workflow, EngineConfig};
use rings::events;
use rings::executor::{ExecutorOutput, MockExecutor};
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
fn jsonl_run_emits_start_first_and_summary_last() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 2);
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "working".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "DONE".to_string(),
            exit_code: 0,
        },
    ]);
    let config = make_jsonl_config(dir.path(), "test-run-start-summary");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    assert_eq!(result.exit_code, 0, "should complete with signal");
    assert!(lines.len() >= 2, "should have at least start + summary");
    assert_eq!(event_type(&parse_events(&lines)[0]), "start");
    assert_eq!(event_type(parse_events(&lines).last().unwrap()), "summary");
}

#[test]
fn start_event_has_correct_rings_version_and_schema_version() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 1);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);
    let config = make_jsonl_config(dir.path(), "test-start-fields");

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    let evts = parse_events(&lines);
    let start = &evts[0];
    assert_eq!(start["event"], "start");
    assert_eq!(start["schema_version"], 1);
    let version = start["rings_version"].as_str().unwrap();
    assert!(!version.is_empty(), "rings_version must not be empty");
    assert_eq!(start["workflow"], "test.rings.toml");
}

#[test]
fn summary_status_completed_on_completion_signal() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);
    let config = make_jsonl_config(dir.path(), "test-status-completed");

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    let evts = parse_events(&lines);
    let summary = evts.last().unwrap();
    assert_eq!(summary["event"], "summary");
    assert_eq!(summary["status"], "completed");
}

#[test]
fn summary_status_max_cycles_when_no_signal() {
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
    let config = make_jsonl_config(dir.path(), "test-status-max-cycles");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    assert_eq!(result.exit_code, 1);
    let evts = parse_events(&lines);
    let summary = evts.last().unwrap();
    assert_eq!(summary["event"], "summary");
    assert_eq!(summary["status"], "max_cycles");
}

#[test]
fn summary_status_executor_error_on_nonzero_exit() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "something went wrong".to_string(),
        exit_code: 1,
    }]);
    let config = make_jsonl_config(dir.path(), "test-status-executor-error");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    assert_eq!(result.exit_code, 3);
    let evts = parse_events(&lines);
    let summary = evts.last().unwrap();
    assert_eq!(summary["event"], "summary");
    assert_eq!(summary["status"], "executor_error");
}

#[test]
fn summary_status_budget_cap_when_cap_exceeded() {
    let dir = tempdir().unwrap();
    // Build a workflow with a tiny budget cap. The mock output must contain a cost
    // that exceeds the cap. Use the JSON format that the cost parser understands.
    let mut workflow = make_workflow("DONE", &[("builder", 1)], 5);
    workflow.budget_cap_usd = Some(0.001); // very small cap

    // Emit a cost that exceeds the cap. The cost parser looks for "total_cost_usd".
    let mock_output =
        r#"{"type":"result","subtype":"success","total_cost_usd":0.05,"result":"working"}"#;
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: mock_output.to_string(),
        exit_code: 0,
    }]);
    let config = make_jsonl_config(dir.path(), "test-status-budget-cap");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    assert_eq!(result.exit_code, 4, "should exit with budget_cap code");
    let evts = parse_events(&lines);
    let summary = evts.last().unwrap();
    assert_eq!(summary["event"], "summary");
    assert_eq!(summary["status"], "budget_cap");
}

#[test]
fn summary_phases_has_correct_per_phase_run_counts() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1), ("reviewer", 1)], 1);
    // Two phases, 1 run each = 2 runs, no signal → max_cycles at cycle 1
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "working".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "working".to_string(),
            exit_code: 0,
        },
    ]);
    let config = make_jsonl_config(dir.path(), "test-summary-phases");

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    let evts = parse_events(&lines);
    let summary = evts.last().unwrap();
    assert_eq!(summary["event"], "summary");

    let phases = summary["phases"].as_array().unwrap();
    assert_eq!(phases.len(), 2, "should have 2 phase entries");

    let builder = phases.iter().find(|p| p["name"] == "builder").unwrap();
    assert_eq!(builder["runs"], 1);

    let reviewer = phases.iter().find(|p| p["name"] == "reviewer").unwrap();
    assert_eq!(reviewer["runs"], 1);
}

#[test]
fn summary_status_canceled_on_cancellation() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 10);
    let cancel = Arc::new(CancelState::new());

    // Signal cancellation before spawning — the engine sees it in the inner wait loop.
    cancel.signal_received();

    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "partial".to_string(),
        exit_code: 0,
    }]);
    let config = make_jsonl_config(dir.path(), "test-status-canceled");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, Some(cancel)).unwrap();
    let lines = events::stop_capture();

    assert_eq!(result.exit_code, 130, "should be canceled");
    let evts = parse_events(&lines);
    let summary = evts.last().unwrap();
    assert_eq!(summary["event"], "summary");
    assert_eq!(summary["status"], "canceled");
}

#[test]
fn fatal_error_event_has_null_run_id() {
    // Test the FatalErrorEvent struct directly (matching the spec requirement)
    let ev = events::FatalErrorEvent::new(None, "Invalid workflow file");
    let json = serde_json::to_string(&ev).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["event"], "fatal_error");
    assert!(v["run_id"].is_null(), "run_id should be null when None");
    assert!(!v["message"].as_str().unwrap().is_empty());
    assert!(!v["timestamp"].as_str().unwrap().is_empty());
}
