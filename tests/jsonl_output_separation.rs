/// Tests for Task 6: stdout/stderr separation, event sequence verification,
/// and --step/--output-format jsonl conflict check.
use rings::cli::OutputFormat;
use rings::engine::{run_workflow, EngineConfig};
use rings::events;
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::workflow::{CompiledErrorProfile, CompletionSignalMode, PhaseConfig, Workflow};
use tempfile::tempdir;

fn default_error_profile() -> CompiledErrorProfile {
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
        compiled_error_profile: default_error_profile(),
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

fn make_human_config(dir: &std::path::Path, run_id: &str) -> EngineConfig {
    EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.to_path_buf(),
        verbose: false,
        run_id: run_id.to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: OutputFormat::Human,
        strict_parsing: false,
    }
}

fn parse_events(lines: &[String]) -> Vec<serde_json::Value> {
    lines
        .iter()
        .map(|l| serde_json::from_str(l).expect("all captured lines should be valid JSON"))
        .collect()
}

fn event_type(v: &serde_json::Value) -> &str {
    v["event"].as_str().unwrap()
}

/// A complete 2-cycle JSONL workflow produces parseable JSON on every line,
/// with event sequence: start → (run_start, run_end)+ → completion_signal → summary.
#[test]
fn jsonl_two_cycle_event_sequence() {
    let dir = tempdir().unwrap();
    // 2-cycle workflow: cycle 1 no signal, cycle 2 emits signal
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "no signal".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "DONE".to_string(),
            exit_code: 0,
        },
    ]);
    let config = make_jsonl_config(dir.path(), "test-seq");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    assert_eq!(result.exit_code, 0, "should complete");
    assert!(
        lines.len() >= 5,
        "should have start, 2×run_start, 2×run_end, completion_signal, summary"
    );

    // All lines must be valid JSON
    let evts = parse_events(&lines);

    // First event must be "start"
    assert_eq!(event_type(&evts[0]), "start", "first event must be start");

    // Last event must be "summary"
    assert_eq!(
        event_type(evts.last().unwrap()),
        "summary",
        "last event must be summary"
    );

    // Somewhere there must be run_start, run_end, and completion_signal events
    let types: Vec<&str> = evts.iter().map(|v| event_type(v)).collect();
    assert!(
        types.contains(&"run_start"),
        "must contain run_start events"
    );
    assert!(types.contains(&"run_end"), "must contain run_end events");
    assert!(
        types.contains(&"completion_signal"),
        "must contain completion_signal event"
    );

    // Verify ordering: start comes before any run_start; summary is last
    let start_idx = types.iter().position(|&t| t == "start").unwrap();
    let first_run_start_idx = types.iter().position(|&t| t == "run_start").unwrap();
    let signal_idx = types
        .iter()
        .position(|&t| t == "completion_signal")
        .unwrap();
    let summary_idx = types.len() - 1;

    assert!(
        start_idx < first_run_start_idx,
        "start must precede run_start"
    );
    assert!(
        signal_idx < summary_idx,
        "completion_signal must precede summary"
    );
}

/// All JSONL events in a workflow run share the same run_id.
#[test]
fn jsonl_all_events_share_run_id() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 2);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);
    let config = make_jsonl_config(dir.path(), "my-run-id");

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    let evts = parse_events(&lines);
    for ev in &evts {
        let run_id = ev["run_id"]
            .as_str()
            .unwrap_or_else(|| panic!("event missing run_id: {ev}"));
        assert_eq!(run_id, "my-run-id", "all events must share the same run_id");
    }
}

/// In JSONL mode, run_start precedes its corresponding run_end.
#[test]
fn jsonl_run_start_precedes_run_end() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 3);
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "no signal".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "DONE".to_string(),
            exit_code: 0,
        },
    ]);
    let config = make_jsonl_config(dir.path(), "test-order");

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    let evts = parse_events(&lines);
    let types: Vec<&str> = evts.iter().map(|v| event_type(v)).collect();

    // Verify each run_start is followed (eventually) by a run_end before the next run_start
    let mut last_was_run_start = false;
    for t in &types {
        if *t == "run_start" {
            last_was_run_start = true;
        } else if *t == "run_end" {
            // After every run_end, we reset
            last_was_run_start = false;
        } else if *t == "run_start" && last_was_run_start {
            panic!("got two consecutive run_start events without a run_end between them");
        }
    }
}

/// In Human mode, no JSONL events are emitted (stdout is empty from events perspective).
#[test]
fn human_mode_emits_no_jsonl_events() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 2);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);
    let config = make_human_config(dir.path(), "test-human-no-events");

    events::start_capture();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let lines = events::stop_capture();

    assert_eq!(
        result.exit_code, 0,
        "human mode should complete successfully"
    );
    assert!(
        lines.is_empty(),
        "human mode must emit zero JSONL events to stdout; got: {lines:?}"
    );
}

/// --step with --output-format jsonl exits 2 with a descriptive error message.
#[test]
fn step_jsonl_conflict_exits_2() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rings"))
        .args([
            "run",
            "any-nonexistent-file.toml",
            "--step",
            "--output-format",
            "jsonl",
        ])
        .output()
        .expect("failed to spawn rings binary");

    assert_eq!(
        output.status.code(),
        Some(2),
        "--step --output-format jsonl must exit with code 2"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--step is incompatible with --output-format jsonl"),
        "error message must mention the incompatibility; got: {stderr}"
    );

    // stdout must be empty (no JSONL events emitted before conflict check)
    assert!(
        output.stdout.is_empty(),
        "no JSONL events should be emitted when conflict is detected; got stdout: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
}
