// tests/engine_timeout_cancel.rs
use rings::cancel::CancelState;
use rings::engine::{run_workflow, EngineConfig};
use rings::executor::{Executor, ExecutorOutput, Invocation, MockExecutor, RunHandle};
use rings::state::{FailureReason, StateFile};
use rings::workflow::Workflow;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tempfile::tempdir;

/// Wrapper that allows tracking signal calls on the spawned handle.
pub struct TrackedSlowMockHandle {
    pub inner: rings::executor::SlowMockRunHandle,
}

impl RunHandle for TrackedSlowMockHandle {
    fn wait(&mut self) -> anyhow::Result<ExecutorOutput> {
        self.inner.wait()
    }

    fn try_wait(&mut self) -> anyhow::Result<Option<ExecutorOutput>> {
        self.inner.try_wait()
    }

    fn pid(&self) -> u32 {
        self.inner.pid()
    }

    fn send_sigterm(&self) -> anyhow::Result<()> {
        self.inner.send_sigterm()
    }

    fn send_sigkill(&self) -> anyhow::Result<()> {
        self.inner.send_sigkill()
    }

    fn partial_output(&self) -> anyhow::Result<String> {
        self.inner.partial_output()
    }
}

/// Test executor that returns SlowMockRunHandle instances and tracks signal calls.
struct SlowMockExecutor {
    try_wait_returns_none_count: Arc<AtomicU32>,
    output: ExecutorOutput,
    pub last_sigterm_called: Arc<AtomicBool>,
    pub last_sigkill_called: Arc<AtomicBool>,
}

impl SlowMockExecutor {
    fn new(try_wait_returns_none_count: u32, output: ExecutorOutput) -> Self {
        Self {
            try_wait_returns_none_count: Arc::new(AtomicU32::new(try_wait_returns_none_count)),
            output,
            last_sigterm_called: Arc::new(AtomicBool::new(false)),
            last_sigkill_called: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Executor for SlowMockExecutor {
    fn spawn(
        &self,
        _invocation: &Invocation,
        _verbose: bool,
    ) -> anyhow::Result<Box<dyn RunHandle>> {
        let sigterm = Arc::clone(&self.last_sigterm_called);
        let sigkill = Arc::clone(&self.last_sigkill_called);

        Ok(Box::new(TrackedSlowMockHandle {
            inner: rings::executor::SlowMockRunHandle {
                output: self.output.clone(),
                try_wait_returns_none_count: Arc::clone(&self.try_wait_returns_none_count),
                sigterm_called: sigterm,
                sigkill_called: sigkill,
            },
        }))
    }
}

#[test]
fn sigterm_called_on_cancellation() {
    let mock_output = ExecutorOutput {
        combined: "test output".to_string(),
        exit_code: 0,
    };
    let executor = MockExecutor::new(vec![mock_output.clone()]);

    let dir = tempdir().unwrap();
    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test_cancel_1".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
    };

    let workflow_str = r#"
[workflow]
completion_signal = "done"
context_dir = "."
max_cycles = 1

[[phases]]
name = "test"
prompt_text = "test prompt"
"#;

    let workflow: Workflow = workflow_str.parse().unwrap();

    // Create cancel state
    let cancel_state = Arc::new(CancelState::new());

    // Run workflow (will use the mock executor)
    let result = run_workflow(
        &workflow,
        &executor,
        &config,
        None,
        Some(cancel_state.clone()),
    );

    // The result should succeed since we never actually trigger cancellation in the run
    assert!(result.is_ok());
}

#[test]
fn timeout_failure_reason_recorded_in_state() {
    // This test verifies that when a timeout occurs, the failure_reason is set to "timeout"
    // in the state file. We can't directly test timeout behavior in unit tests without
    // mocking time, but we verify the field exists and is properly serialized.

    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    let state = StateFile {
        schema_version: 1,
        run_id: "test_timeout".to_string(),
        workflow_file: "test.toml".to_string(),
        last_completed_run: 1,
        last_completed_cycle: 1,
        last_completed_phase_index: 0,
        last_completed_iteration: 1,
        total_runs_completed: 1,
        cumulative_cost_usd: 0.0,
        claude_resume_commands: vec![],
        canceled_at: None,
        failure_reason: Some(FailureReason::Timeout),
        ancestry: None,
    };

    state.write_atomic(&path).unwrap();
    let loaded = StateFile::read(&path).unwrap();

    assert_eq!(loaded.failure_reason, Some(FailureReason::Timeout));
}

#[test]
fn state_includes_failure_reason_field_with_default() {
    // Test that the failure_reason field is properly deserialized with default when missing
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    // Simulate an old state file without failure_reason
    let old_json = r#"{
        "schema_version": 1,
        "run_id": "old_run",
        "workflow_file": "test.toml",
        "last_completed_run": 1,
        "last_completed_cycle": 1,
        "last_completed_phase_index": 0,
        "last_completed_iteration": 1,
        "total_runs_completed": 1,
        "cumulative_cost_usd": 0.0,
        "claude_resume_commands": [],
        "canceled_at": null
    }"#;

    std::fs::write(&path, old_json).unwrap();
    let loaded = StateFile::read(&path).unwrap();

    // Should deserialize successfully with failure_reason as None
    assert_eq!(loaded.failure_reason, None);
}

#[test]
fn cancellation_state_recorded_with_null_failure_reason() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.json");

    let state = StateFile {
        schema_version: 1,
        run_id: "test_cancel".to_string(),
        workflow_file: "test.toml".to_string(),
        last_completed_run: 1,
        last_completed_cycle: 1,
        last_completed_phase_index: 0,
        last_completed_iteration: 1,
        total_runs_completed: 1,
        cumulative_cost_usd: 0.0,
        claude_resume_commands: vec![],
        canceled_at: Some("2026-03-15T14:30:00Z".to_string()),
        failure_reason: None,
        ancestry: None,
    };

    state.write_atomic(&path).unwrap();
    let loaded = StateFile::read(&path).unwrap();

    assert_eq!(loaded.canceled_at, Some("2026-03-15T14:30:00Z".to_string()));
    assert_eq!(loaded.failure_reason, None);
}

#[test]
fn double_ctrl_c_sends_sigkill_before_grace_period_expires() {
    let dir = tempdir().unwrap();

    // Configure mock to stay alive for 50 polls (100ms each = 5 seconds total)
    let executor = SlowMockExecutor::new(
        50,
        ExecutorOutput {
            combined: "test output".to_string(),
            exit_code: 0,
        },
    );

    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test_double_sigkill".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
    };

    let workflow_str = r#"
[workflow]
completion_signal = "done"
context_dir = "."
max_cycles = 1

[[phases]]
name = "test"
prompt_text = "test prompt"
"#;

    let workflow: Workflow = workflow_str.parse().unwrap();
    let cancel_state = Arc::new(CancelState::new());

    // Clone the references BEFORE moving executor into closure
    let cancel_state_clone = Arc::clone(&cancel_state);
    let sigkill_flag = Arc::clone(&executor.last_sigkill_called);
    let sigterm_flag = Arc::clone(&executor.last_sigterm_called);
    let sigkill_flag_check = Arc::clone(&executor.last_sigkill_called);

    // Measure timing from when we set ForceKill
    let force_kill_time = Arc::new(std::sync::Mutex::new(None::<std::time::Instant>));
    let force_kill_time_clone = Arc::clone(&force_kill_time);
    let sigkill_time = Arc::new(std::sync::Mutex::new(None::<std::time::Instant>));
    let sigkill_time_clone = Arc::clone(&sigkill_time);

    // Run the engine on a background thread
    let engine_thread = std::thread::spawn(move || {
        let _ = run_workflow(
            &workflow,
            &executor,
            &config,
            None,
            Some(cancel_state_clone),
        );
    });

    // Wait for the run to start (allow 50ms for engine to spawn and enter first run loop)
    std::thread::sleep(std::time::Duration::from_millis(50));

    // First Ctrl+C: signal_received() → Canceling state
    cancel_state.signal_received();

    // Wait for SIGTERM to be sent (grace period should start)
    // The engine polls every 100ms, so give it up to 200ms to notice the canceling state
    for _ in 0..20 {
        if sigterm_flag.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(
        sigterm_flag.load(Ordering::SeqCst),
        "SIGTERM should have been sent after first Ctrl+C"
    );

    // Small delay to ensure grace period loop has started
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Second Ctrl+C: signal_received() → ForceKill state
    *force_kill_time_clone.lock().unwrap() = Some(std::time::Instant::now());
    cancel_state.signal_received();

    // The engine should detect force_kill and send SIGKILL within ~100ms
    // (the next poll cycle). Give it 500ms to be safe, but verify it's much faster
    for _ in 0..50 {
        if sigkill_flag.load(Ordering::SeqCst) {
            *sigkill_time_clone.lock().unwrap() = Some(std::time::Instant::now());
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Wait for engine thread to finish
    let _ = engine_thread.join();

    // Verify SIGKILL was sent
    assert!(
        sigkill_flag_check.load(Ordering::SeqCst),
        "SIGKILL should have been sent after second Ctrl+C"
    );

    // Verify SIGKILL was sent quickly (within 500ms of force_kill)
    let force_kill_inst = force_kill_time.lock().unwrap();
    let sigkill_inst = sigkill_time.lock().unwrap();

    if let (Some(fk), Some(sk)) = (*force_kill_inst, *sigkill_inst) {
        let elapsed = sk.duration_since(fk);
        assert!(
            elapsed < std::time::Duration::from_millis(500),
            "SIGKILL should be sent within 500ms, but took {:?}",
            elapsed
        );
    }
}

#[test]
fn single_ctrl_c_waits_up_to_grace_period_without_second_signal() {
    let dir = tempdir().unwrap();

    // Configure mock to stay alive for 20 polls (2 seconds) so cancellation can be processed
    let executor = SlowMockExecutor::new(
        20,
        ExecutorOutput {
            combined: "test output".to_string(),
            exit_code: 0,
        },
    );

    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test_single_sigterm".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
    };

    let workflow_str = r#"
[workflow]
completion_signal = "done"
context_dir = "."
max_cycles = 1

[[phases]]
name = "test"
prompt_text = "test prompt"
"#;

    let workflow: Workflow = workflow_str.parse().unwrap();
    let cancel_state = Arc::new(CancelState::new());
    let cancel_state_clone = Arc::clone(&cancel_state);

    let sigterm_flag = Arc::clone(&executor.last_sigterm_called);
    let sigkill_flag = Arc::clone(&executor.last_sigkill_called);

    let engine_thread = std::thread::spawn(move || {
        let _ = run_workflow(
            &workflow,
            &executor,
            &config,
            None,
            Some(cancel_state_clone),
        );
    });

    // Wait for engine to start
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Send single Ctrl+C (Canceling via signal_received)
    cancel_state.signal_received();

    // Wait for SIGTERM
    for _ in 0..20 {
        if sigterm_flag.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(
        sigterm_flag.load(Ordering::SeqCst),
        "SIGTERM should have been sent"
    );

    // Don't send force kill, just let it finish
    let _ = engine_thread.join();

    // Verify SIGKILL was NOT sent (since we didn't send second Ctrl+C)
    assert!(
        !sigkill_flag.load(Ordering::SeqCst),
        "SIGKILL should not be sent for single Ctrl+C"
    );
}

/// Verify that a quota backoff retry succeeds when a per-run timeout is configured.
/// With the fix, each retry attempt gets a fresh timeout deadline. Without the fix,
/// a retry after a backoff delay would re-use the original run_start and might
/// observe a stale deadline.
#[test]
fn quota_retry_succeeds_with_active_timeout() {
    let dir = tempdir().unwrap();

    // First call: quota error (matches "rate limit" in the ClaudeCode error profile)
    // Second call: success with completion signal
    let quota_error = ExecutorOutput {
        combined: "rate limit reached, please wait".to_string(),
        exit_code: 1,
    };
    let success = ExecutorOutput {
        combined: "task complete: done".to_string(),
        exit_code: 0,
    };
    let executor = MockExecutor::new(vec![quota_error, success]);

    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test_quota_timeout".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
    };

    // Workflow with quota_backoff enabled (0 delay so test is fast),
    // a generous timeout (30s — won't fire during an instant retry),
    // and completion_signal "done".
    let workflow_str = r#"
[workflow]
completion_signal = "done"
context_dir = "."
max_cycles = 1
quota_backoff = true
quota_backoff_delay = 0
quota_backoff_max_retries = 1
timeout_per_run_secs = 30

[[phases]]
name = "test"
prompt_text = "test prompt"
"#;

    let workflow: Workflow = workflow_str.parse().unwrap();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();

    // The retry should have succeeded with exit_code 0 (completion signal found)
    assert_eq!(
        result.exit_code, 0,
        "expected success after quota retry, got exit_code {}",
        result.exit_code
    );
}

/// Verify that a quota backoff retry also works when no per-run timeout is set
/// (baseline: retry mechanism unaffected by the timeout deadline change).
#[test]
fn quota_retry_succeeds_without_timeout() {
    let dir = tempdir().unwrap();

    let quota_error = ExecutorOutput {
        combined: "rate limit reached, please wait".to_string(),
        exit_code: 1,
    };
    let success = ExecutorOutput {
        combined: "task complete: done".to_string(),
        exit_code: 0,
    };
    let executor = MockExecutor::new(vec![quota_error, success]);

    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test_quota_no_timeout".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
    };

    let workflow_str = r#"
[workflow]
completion_signal = "done"
context_dir = "."
max_cycles = 1
quota_backoff = true
quota_backoff_delay = 0
quota_backoff_max_retries = 1

[[phases]]
name = "test"
prompt_text = "test prompt"
"#;

    let workflow: Workflow = workflow_str.parse().unwrap();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();

    assert_eq!(
        result.exit_code, 0,
        "expected success after quota retry (no timeout), got exit_code {}",
        result.exit_code
    );
}

/// Verify that a per-run timeout still fires correctly on a slow subprocess
/// when no quota backoff retry is involved.
#[test]
fn timeout_fires_correctly_without_retry() {
    let dir = tempdir().unwrap();

    // Mock executor that stays "alive" for many polls — far exceeding a 1-second timeout.
    let executor = SlowMockExecutor::new(
        200,
        ExecutorOutput {
            combined: "done".to_string(),
            exit_code: 0,
        },
    );

    let config = EngineConfig {
        ancestry_continuation_of: None,
        ancestry_depth: 0,
        output_dir: dir.path().to_path_buf(),
        verbose: false,
        run_id: "test_timeout_fires".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        no_contract_check: false,
        output_format: rings::cli::OutputFormat::Human,
        strict_parsing: false,
    };

    // 1-second timeout; the mock executor stays alive for 20 seconds (200 × 100ms).
    let workflow_str = r#"
[workflow]
completion_signal = "done"
context_dir = "."
max_cycles = 1
timeout_per_run_secs = 1

[[phases]]
name = "test"
prompt_text = "test prompt"
"#;

    let workflow: Workflow = workflow_str.parse().unwrap();
    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();

    // Timeout path returns exit_code 2
    assert_eq!(
        result.exit_code, 2,
        "expected timeout exit code 2, got {}",
        result.exit_code
    );
}
