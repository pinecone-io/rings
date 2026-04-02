// Gate logging tests — human output format, JSONL events, and run log files.
// Run with: cargo test --features testing
use rings::cli::OutputFormat;
use rings::engine::{
    format_cycle_gate_line, format_phase_gate_line, run_workflow, truncate_gate_command,
    EngineConfig,
};
use rings::events;
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::workflow::{
    CompiledErrorProfile, CompletionSignalMode, GateAction, GateConfig, PhaseConfig, Workflow,
};
use tempfile::tempdir;

fn default_compiled_error_profile() -> CompiledErrorProfile {
    CompiledErrorProfile {
        quota_regexes: vec![],
        auth_regexes: vec![],
    }
}

fn no_signal_output() -> ExecutorOutput {
    ExecutorOutput {
        combined: "no signal".to_string(),
        exit_code: 0,
    }
}

fn make_workflow_with_cycle_gate(cycle_gate: Option<GateConfig>, max_cycles: u32) -> Workflow {
    Workflow {
        completion_signal: "RINGS_DONE".to_string(),
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
        cycle_gate,
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
            executor: None,
            gate: None,
            gate_each_run: false,
        }],
    }
}

// ── truncate_gate_command ─────────────────────────────────────────────────────

#[test]
fn truncate_gate_command_short_unchanged() {
    let cmd = "test -f foo.txt";
    assert_eq!(truncate_gate_command(cmd, 80), cmd);
}

#[test]
fn truncate_gate_command_exactly_max_unchanged() {
    let cmd = "a".repeat(80);
    assert_eq!(truncate_gate_command(&cmd, 80), cmd);
}

#[test]
fn truncate_gate_command_over_max_appends_ellipsis() {
    let cmd = "a".repeat(81);
    let result = truncate_gate_command(&cmd, 80);
    assert!(result.ends_with("..."), "should end with ...: {result}");
    assert_eq!(&result[..80], &cmd[..80]);
}

// ── format_cycle_gate_line ────────────────────────────────────────────────────

#[test]
fn format_cycle_gate_line_pass() {
    let line = format_cycle_gate_line(3, "true", 0, true, None);
    assert!(
        line.contains("[cycle 3]"),
        "should contain cycle number: {line}"
    );
    assert!(line.contains("cycle gate"), "should say cycle gate: {line}");
    assert!(line.contains("`true`"), "should contain command: {line}");
    assert!(line.contains("exit 0"), "should contain exit code: {line}");
    assert!(line.contains("pass"), "should say pass: {line}");
}

#[test]
fn format_cycle_gate_line_fail_with_action() {
    let line = format_cycle_gate_line(1, "false", 1, false, Some("stop"));
    assert!(line.contains("exit 1"), "should contain exit code: {line}");
    assert!(line.contains("fail → stop"), "should show action: {line}");
}

#[test]
fn format_cycle_gate_line_command_truncated_at_80() {
    let long_cmd = "x".repeat(100);
    let line = format_cycle_gate_line(1, &long_cmd, 0, true, None);
    // The displayed command should be truncated with ...
    assert!(
        line.contains("..."),
        "long command should be truncated: {line}"
    );
}

// ── format_phase_gate_line ────────────────────────────────────────────────────

#[test]
fn format_phase_gate_line_contains_phase_name() {
    let line = format_phase_gate_line(2, "reviewer", "test -f foo", 0, true, None);
    assert!(
        line.contains("phase \"reviewer\""),
        "should contain phase name: {line}"
    );
    assert!(
        line.contains("`test -f foo`"),
        "should contain command: {line}"
    );
    assert!(line.contains("exit 0"), "should contain exit code: {line}");
    assert!(line.contains("pass"), "should say pass: {line}");
}

#[test]
fn format_phase_gate_line_fail_shows_action() {
    let line = format_phase_gate_line(1, "builder", "false", 1, false, Some("skip"));
    assert!(line.contains("fail → skip"), "should show action: {line}");
}

// ── JSONL events ──────────────────────────────────────────────────────────────

#[test]
fn jsonl_gate_result_event_passing_cycle_gate() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "true".to_string(),
        on_fail: Some(GateAction::Stop),
        timeout: None,
    };
    let workflow = make_workflow_with_cycle_gate(Some(gate), 1);
    let executor = MockExecutor::new(vec![no_signal_output()]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        output_format: OutputFormat::Jsonl,
        ..Default::default()
    };

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let captured = events::stop_capture();

    let gate_events: Vec<serde_json::Value> = captured
        .iter()
        .filter_map(|s| serde_json::from_str(s).ok())
        .filter(|v: &serde_json::Value| v["event"] == "gate_result")
        .collect();

    assert!(
        !gate_events.is_empty(),
        "expected at least one gate_result event"
    );
    let ev = &gate_events[0];
    assert_eq!(ev["event"], "gate_result");
    assert_eq!(ev["scope"], "cycle");
    assert_eq!(ev["phase"], serde_json::Value::Null);
    assert_eq!(ev["command"], "true");
    assert_eq!(ev["exit_code"], 0);
    assert_eq!(ev["passed"], true);
    assert_eq!(ev["action"], serde_json::Value::Null);
    assert!(ev["run_id"].is_string());
    assert!(ev["timestamp"].is_string());
}

#[test]
fn jsonl_gate_result_event_failing_cycle_gate_with_action() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "false".to_string(),
        on_fail: Some(GateAction::Stop),
        timeout: None,
    };
    let workflow = make_workflow_with_cycle_gate(Some(gate), 5);
    let executor = MockExecutor::new(vec![]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        output_format: OutputFormat::Jsonl,
        ..Default::default()
    };

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let captured = events::stop_capture();

    let gate_events: Vec<serde_json::Value> = captured
        .iter()
        .filter_map(|s| serde_json::from_str(s).ok())
        .filter(|v: &serde_json::Value| v["event"] == "gate_result")
        .collect();

    assert!(
        !gate_events.is_empty(),
        "expected gate_result event on failure"
    );
    let ev = &gate_events[0];
    assert_eq!(ev["passed"], false);
    assert_eq!(ev["action"], "stop");
    assert!(ev["exit_code"].as_i64().unwrap() != 0);
}

#[test]
fn jsonl_gate_result_event_phase_gate_contains_phase_name() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "true".to_string(),
        on_fail: Some(GateAction::Skip),
        timeout: None,
    };
    let phase = PhaseConfig {
        name: "reviewer".to_string(),
        prompt: None,
        prompt_text: Some("do work".to_string()),
        runs_per_cycle: 1,
        budget_cap_usd: None,
        timeout_per_run_secs: None,
        consumes: vec![],
        produces: vec![],
        produces_required: false,
        executor: None,
        gate: Some(gate),
        gate_each_run: false,
    };
    let workflow = Workflow {
        completion_signal: "RINGS_DONE".to_string(),
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
        lock_name: None,
        cycle_gate: None,
        phases: vec![phase],
    };
    let executor = MockExecutor::new(vec![no_signal_output()]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        output_format: OutputFormat::Jsonl,
        ..Default::default()
    };

    events::start_capture();
    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    let captured = events::stop_capture();

    let gate_events: Vec<serde_json::Value> = captured
        .iter()
        .filter_map(|s| serde_json::from_str(s).ok())
        .filter(|v: &serde_json::Value| v["event"] == "gate_result")
        .collect();

    assert!(
        !gate_events.is_empty(),
        "expected gate_result event for phase gate"
    );
    let ev = &gate_events[0];
    assert_eq!(ev["scope"], "phase");
    assert_eq!(ev["phase"], "reviewer");
    assert_eq!(ev["passed"], true);
    assert_eq!(ev["action"], serde_json::Value::Null);
}

// ── Gate log files ────────────────────────────────────────────────────────────

#[test]
fn cycle_gate_stdout_stderr_written_to_log_file() {
    let dir = tempdir().unwrap();
    // Use a command that produces stdout
    let gate = GateConfig {
        command: "echo hello_gate_output".to_string(),
        on_fail: Some(GateAction::Stop),
        timeout: None,
    };
    let workflow = make_workflow_with_cycle_gate(Some(gate), 1);
    let executor = MockExecutor::new(vec![no_signal_output()]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();

    let runs_dir = dir.path().join("runs");
    // Gate log should exist: 001-gate-cycle.log
    let gate_log = runs_dir.join("001-gate-cycle.log");
    assert!(
        gate_log.exists(),
        "gate log file should exist at {}: found files: {:?}",
        gate_log.display(),
        std::fs::read_dir(&runs_dir)
            .map(|d| d
                .filter_map(|e| e.ok().map(|e| e.file_name()))
                .collect::<Vec<_>>())
            .unwrap_or_default()
    );
    let content = std::fs::read_to_string(&gate_log).unwrap();
    assert!(
        content.contains("hello_gate_output"),
        "gate log should contain stdout: {content}"
    );
}

#[test]
fn phase_gate_stdout_stderr_written_to_log_file() {
    let dir = tempdir().unwrap();
    let gate = GateConfig {
        command: "echo phase_gate_ran".to_string(),
        on_fail: Some(GateAction::Skip),
        timeout: None,
    };
    let phase = PhaseConfig {
        name: "myreview".to_string(),
        prompt: None,
        prompt_text: Some("do work".to_string()),
        runs_per_cycle: 1,
        budget_cap_usd: None,
        timeout_per_run_secs: None,
        consumes: vec![],
        produces: vec![],
        produces_required: false,
        executor: None,
        gate: Some(gate),
        gate_each_run: false,
    };
    let workflow = Workflow {
        completion_signal: "RINGS_DONE".to_string(),
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
        lock_name: None,
        cycle_gate: None,
        phases: vec![phase],
    };
    let executor = MockExecutor::new(vec![no_signal_output()]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        run_id: "test-run".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        ..Default::default()
    };

    let _ = run_workflow(&workflow, &executor, &config, None, None).unwrap();

    let runs_dir = dir.path().join("runs");
    // Gate log should exist: 001-gate-myreview.log
    let gate_log = runs_dir.join("001-gate-myreview.log");
    assert!(
        gate_log.exists(),
        "phase gate log file should exist at {}: found files: {:?}",
        gate_log.display(),
        std::fs::read_dir(&runs_dir)
            .map(|d| d
                .filter_map(|e| e.ok().map(|e| e.file_name()))
                .collect::<Vec<_>>())
            .unwrap_or_default()
    );
    let content = std::fs::read_to_string(&gate_log).unwrap();
    assert!(
        content.contains("phase_gate_ran"),
        "gate log should contain stdout: {content}"
    );
}
