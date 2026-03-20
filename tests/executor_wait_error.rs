/// Tests that the engine emits a SummaryEvent when try_wait() returns an error,
/// maintaining the JSONL start/summary event pairing contract.
use rings::cli::OutputFormat;
use rings::engine::{run_workflow, EngineConfig};
use rings::events;
use rings::executor::ErrorMockExecutor;
use rings::workflow::{CompiledErrorProfile, CompletionSignalMode, PhaseConfig, Workflow};
use tempfile::tempdir;

fn default_compiled_error_profile() -> CompiledErrorProfile {
    CompiledErrorProfile {
        quota_regexes: vec![],
        auth_regexes: vec![],
    }
}

fn make_workflow() -> Workflow {
    Workflow {
        completion_signal: "DONE".to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: ".".to_string(),
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
        phases: vec![PhaseConfig {
            name: "builder".to_string(),
            prompt: None,
            prompt_text: Some("do work, signal=DONE".to_string()),
            runs_per_cycle: 1,
            budget_cap_usd: None,
            timeout_per_run_secs: None,
            consumes: vec![],
            produces: vec![],
            produces_required: false,
            executor: None,
        }],
    }
}

fn make_jsonl_config(dir: &std::path::Path) -> EngineConfig {
    EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.to_path_buf(),
        verbose: false,
        run_id: "test-wait-error".to_string(),
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

#[test]
fn executor_wait_error_emits_start_and_summary() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow();
    let executor = ErrorMockExecutor;
    let config = make_jsonl_config(dir.path());

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None);
    let lines = events::stop_capture();

    assert!(
        result.is_err(),
        "run_workflow should return Err when try_wait fails"
    );

    let evts = parse_events(&lines);
    assert!(
        evts.len() >= 2,
        "should have at least start + summary events"
    );

    let start = &evts[0];
    assert_eq!(start["event"], "start", "first event should be start");
    assert_eq!(start["run_id"], "test-wait-error");

    let summary = evts.last().unwrap();
    assert_eq!(summary["event"], "summary", "last event should be summary");
    assert_eq!(summary["status"], "executor_error");
    assert_eq!(summary["run_id"], "test-wait-error");
}

#[test]
fn executor_wait_error_summary_precedes_error_propagation() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow();
    let executor = ErrorMockExecutor;
    let config = make_jsonl_config(dir.path());

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None);
    let lines = events::stop_capture();

    // The error must have propagated (not swallowed)
    assert!(result.is_err());
    let err_msg = format!("{:#}", result.err().unwrap());
    assert!(
        err_msg.contains("simulated OS process poll failure"),
        "original error must be propagated: {err_msg}"
    );

    // Summary must still have been emitted
    let evts = parse_events(&lines);
    assert!(
        evts.iter().any(|e| e["event"] == "summary"),
        "summary event must be emitted even when try_wait errors"
    );
}
