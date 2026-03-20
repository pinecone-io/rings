use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

#[cfg(feature = "testing")]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub struct Invocation {
    pub prompt: String,
    pub context_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ExecutorOutput {
    pub combined: String,
    pub exit_code: i32,
}

/// A handle to a running executor subprocess.
pub trait RunHandle: Send {
    fn wait(&mut self) -> Result<ExecutorOutput>;
    /// Non-blocking wait: returns Some if process exited, None if still running.
    fn try_wait(&mut self) -> Result<Option<ExecutorOutput>>;
    fn pid(&self) -> u32;
    fn send_sigterm(&self) -> Result<()>;
    fn send_sigkill(&self) -> Result<()>;
    fn partial_output(&self) -> Result<String>;
}

pub trait Executor: Send + Sync {
    fn spawn(&self, invocation: &Invocation, verbose: bool) -> Result<Box<dyn RunHandle>>;

    /// Convenience wrapper: spawn and wait for completion.
    fn run(&self, invocation: &Invocation, verbose: bool) -> Result<ExecutorOutput> {
        let mut handle = self.spawn(invocation, verbose)?;
        handle.wait()
    }
}

// ─── Signal helpers ────────────────────────────────────────────────────────

enum SignalKind {
    Term,
    Kill,
}

#[cfg(unix)]
fn send_signal(pid: u32, kind: SignalKind) -> Result<()> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    let sig = match kind {
        SignalKind::Term => Signal::SIGTERM,
        SignalKind::Kill => Signal::SIGKILL,
    };
    match kill(Pid::from_raw(pid as i32), sig) {
        Ok(_) => Ok(()),
        Err(nix::errno::Errno::ESRCH) => Ok(()), // process already gone
        Err(e) => Err(anyhow::anyhow!("Failed to send signal: {e}")),
    }
}

#[cfg(not(unix))]
fn send_signal(_pid: u32, _kind: SignalKind) -> Result<()> {
    Ok(())
}

// ─── ClaudeRunHandle ───────────────────────────────────────────────────────

pub struct ClaudeRunHandle {
    child: std::process::Child,
    stdout_output: Arc<Mutex<String>>,
    stderr_output: Arc<Mutex<String>>,
    stdout_thread: Option<std::thread::JoinHandle<()>>,
    stderr_thread: Option<std::thread::JoinHandle<()>>,
    verbose: bool,
}

impl RunHandle for ClaudeRunHandle {
    fn wait(&mut self) -> Result<ExecutorOutput> {
        let exit_code = if self.verbose {
            // Blocking wait; reader threads drain output concurrently.
            let status = self.child.wait().context("Failed to wait for subprocess")?;
            // Join reader threads after the process exits so all buffered data is consumed.
            if let Some(t) = self.stdout_thread.take() {
                let _ = t.join();
            }
            if let Some(t) = self.stderr_thread.take() {
                let _ = t.join();
            }
            status.code().unwrap_or(-1)
        } else {
            // Non-blocking poll in 100ms slices so future tasks can check cancel flags.
            loop {
                match self.child.try_wait().context("Failed to poll subprocess")? {
                    Some(status) => {
                        if let Some(t) = self.stdout_thread.take() {
                            let _ = t.join();
                        }
                        if let Some(t) = self.stderr_thread.take() {
                            let _ = t.join();
                        }
                        break status.code().unwrap_or(-1);
                    }
                    None => std::thread::sleep(std::time::Duration::from_millis(100)),
                }
            }
        };

        let stdout_str = self
            .stdout_output
            .lock()
            .map_err(|_| anyhow::anyhow!("stdout mutex poisoned"))?
            .clone();
        let stderr_str = self
            .stderr_output
            .lock()
            .map_err(|_| anyhow::anyhow!("stderr mutex poisoned"))?
            .clone();

        Ok(ExecutorOutput {
            combined: format!("{stdout_str}\n{stderr_str}"),
            exit_code,
        })
    }

    fn try_wait(&mut self) -> Result<Option<ExecutorOutput>> {
        match self.child.try_wait().context("Failed to poll subprocess")? {
            Some(status) => {
                // Process exited; join reader threads to collect all output.
                if let Some(t) = self.stdout_thread.take() {
                    let _ = t.join();
                }
                if let Some(t) = self.stderr_thread.take() {
                    let _ = t.join();
                }
                let exit_code = status.code().unwrap_or(-1);
                let stdout_str = self
                    .stdout_output
                    .lock()
                    .map_err(|_| anyhow::anyhow!("stdout mutex poisoned"))?
                    .clone();
                let stderr_str = self
                    .stderr_output
                    .lock()
                    .map_err(|_| anyhow::anyhow!("stderr mutex poisoned"))?
                    .clone();
                Ok(Some(ExecutorOutput {
                    combined: format!("{stdout_str}\n{stderr_str}"),
                    exit_code,
                }))
            }
            None => Ok(None), // Process still running
        }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn send_sigterm(&self) -> Result<()> {
        send_signal(self.child.id(), SignalKind::Term)
    }

    fn send_sigkill(&self) -> Result<()> {
        send_signal(self.child.id(), SignalKind::Kill)
    }

    fn partial_output(&self) -> Result<String> {
        let stdout = self
            .stdout_output
            .lock()
            .map_err(|_| anyhow::anyhow!("stdout mutex poisoned"))?
            .clone();
        let stderr = self
            .stderr_output
            .lock()
            .map_err(|_| anyhow::anyhow!("stderr mutex poisoned"))?
            .clone();
        Ok(format!("{stdout}\n{stderr}"))
    }
}

// ─── Shared spawn helper ───────────────────────────────────────────────────

fn spawn_child(
    binary: &str,
    args: &[String],
    context_dir: &PathBuf,
    prompt: &str,
    verbose: bool,
) -> Result<ClaudeRunHandle> {
    let mut cmd = Command::new(binary);
    cmd.args(args)
        .current_dir(context_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn subprocess: {binary}"))?;

    // Write prompt to stdin then drop (→ EOF to subprocess).
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .context("Failed to write prompt to stdin")?;
    }

    let stdout = child.stdout.take().context("Failed to get stdout")?;
    let stderr = child.stderr.take().context("Failed to get stderr")?;

    let stdout_output = Arc::new(Mutex::new(String::new()));
    let stderr_output = Arc::new(Mutex::new(String::new()));

    let stdout_accum = Arc::clone(&stdout_output);
    let stderr_accum = Arc::clone(&stderr_output);

    let stdout_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if verbose {
                eprintln!("{}", line);
            }
            if let Ok(mut acc) = stdout_accum.lock() {
                acc.push_str(&line);
                acc.push('\n');
            }
        }
    });

    let stderr_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if verbose {
                eprintln!("{}", line);
            }
            if let Ok(mut acc) = stderr_accum.lock() {
                acc.push_str(&line);
                acc.push('\n');
            }
        }
    });

    Ok(ClaudeRunHandle {
        child,
        stdout_output,
        stderr_output,
        stdout_thread: Some(stdout_thread),
        stderr_thread: Some(stderr_thread),
        verbose,
    })
}

// ─── ClaudeExecutor ────────────────────────────────────────────────────────

/// Production executor: spawns `claude --dangerously-skip-permissions -p -`
/// and writes the prompt to stdin.
pub struct ClaudeExecutor;

impl ClaudeExecutor {
    /// Returns the fixed argument list for the claude invocation.
    /// Exposed for testing the security invariant: no prompt in args.
    pub fn build_args() -> Vec<String> {
        vec![
            "--dangerously-skip-permissions".to_string(),
            "-p".to_string(),
            "-".to_string(), // read prompt from stdin
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
        ]
    }
}

/// Extract the plain-text response from claude output.
///
/// When `--output-format stream-json` is used, claude emits one JSON object per line.
/// The final `{"type":"result",...}` line contains the `result` field with the
/// assistant's text response. We scan all lines and use the last result event found.
///
/// Falls back to legacy single-JSON-object parsing (for `--output-format json` and
/// custom executors), and then to the raw output if neither parse succeeds.
pub fn extract_response_text(output: &str) -> String {
    // Try stream-json format: scan for the last {"type":"result",...} line.
    let mut last_result: Option<String> = None;
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if v.get("type").and_then(|t| t.as_str()) == Some("result") {
                if let Some(result) = v.get("result").and_then(|r| r.as_str()) {
                    last_result = Some(result.to_string());
                }
            }
        }
    }
    if let Some(result) = last_result {
        return result;
    }

    // Legacy fallback: single JSON object (--output-format json or custom executors).
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(output.trim()) {
        if let Some(result) = v.get("result").and_then(|r| r.as_str()) {
            return result.to_string();
        }
    }

    output.to_string()
}

impl Executor for ClaudeExecutor {
    fn spawn(&self, invocation: &Invocation, verbose: bool) -> Result<Box<dyn RunHandle>> {
        Ok(Box::new(spawn_child(
            "claude",
            &Self::build_args(),
            &invocation.context_dir,
            &invocation.prompt,
            verbose,
        )?))
    }
}

// ─── ConfigurableExecutor ──────────────────────────────────────────────────

/// Configurable executor: spawns an arbitrary binary with caller-supplied args,
/// writing the prompt to stdin. Used when `[executor]` is set in the workflow TOML.
pub struct ConfigurableExecutor {
    pub binary: String,
    pub args: Vec<String>,
}

impl ConfigurableExecutor {
    /// Returns the args that will be passed to the binary.
    /// Exposed so callers can assert the security invariant (no prompt in args).
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

impl Executor for ConfigurableExecutor {
    fn spawn(&self, invocation: &Invocation, verbose: bool) -> Result<Box<dyn RunHandle>> {
        Ok(Box::new(spawn_child(
            &self.binary,
            &self.args,
            &invocation.context_dir,
            &invocation.prompt,
            verbose,
        )?))
    }
}

// ─── MockRunHandle (test-only) ─────────────────────────────────────────────

#[cfg(feature = "testing")]
pub struct MockRunHandle {
    pub output: ExecutorOutput,
    pub wait_delay_ms: u64,
    pub ignores_sigterm: bool,
    pub sigterm_called: Arc<AtomicBool>,
    pub sigkill_called: Arc<AtomicBool>,
}

#[cfg(feature = "testing")]
impl RunHandle for MockRunHandle {
    fn wait(&mut self) -> Result<ExecutorOutput> {
        if self.wait_delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.wait_delay_ms));
        }
        Ok(self.output.clone())
    }

    fn try_wait(&mut self) -> Result<Option<ExecutorOutput>> {
        if self.wait_delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.wait_delay_ms));
        }
        Ok(Some(self.output.clone()))
    }

    fn pid(&self) -> u32 {
        0
    }

    fn send_sigterm(&self) -> Result<()> {
        self.sigterm_called.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn send_sigkill(&self) -> Result<()> {
        self.sigkill_called.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn partial_output(&self) -> Result<String> {
        Ok(self.output.combined.clone())
    }
}

/// Mock handle that returns None for N calls to try_wait, then returns the output.
/// Used for testing cancellation and timeout grace periods.
#[cfg(feature = "testing")]
pub struct SlowMockRunHandle {
    pub output: ExecutorOutput,
    pub try_wait_returns_none_count: Arc<AtomicU32>,
    pub sigterm_called: Arc<AtomicBool>,
    pub sigkill_called: Arc<AtomicBool>,
}

#[cfg(feature = "testing")]
impl RunHandle for SlowMockRunHandle {
    fn wait(&mut self) -> Result<ExecutorOutput> {
        Ok(self.output.clone())
    }

    fn try_wait(&mut self) -> Result<Option<ExecutorOutput>> {
        let count = self.try_wait_returns_none_count.load(Ordering::SeqCst);
        if count > 0 {
            self.try_wait_returns_none_count
                .store(count - 1, Ordering::SeqCst);
            Ok(None)
        } else {
            Ok(Some(self.output.clone()))
        }
    }

    fn pid(&self) -> u32 {
        0
    }

    fn send_sigterm(&self) -> Result<()> {
        self.sigterm_called.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn send_sigkill(&self) -> Result<()> {
        self.sigkill_called.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn partial_output(&self) -> Result<String> {
        Ok(self.output.combined.clone())
    }
}

// ─── MockExecutor (test-only) ──────────────────────────────────────────────

/// Test-only executor: returns pre-configured outputs in sequence.
#[cfg(feature = "testing")]
pub struct MockExecutor {
    outputs: Mutex<Vec<ExecutorOutput>>,
    side_effect: Option<Arc<Mutex<dyn FnMut(&Invocation) + Send>>>,
}

#[cfg(feature = "testing")]
impl MockExecutor {
    pub fn new(mut outputs: Vec<ExecutorOutput>) -> Self {
        outputs.reverse(); // pop from back = FIFO
        Self {
            outputs: Mutex::new(outputs),
            side_effect: None,
        }
    }

    /// Create a MockExecutor that runs `effect` before returning each output.
    pub fn with_side_effect(
        mut outputs: Vec<ExecutorOutput>,
        effect: impl FnMut(&Invocation) + Send + 'static,
    ) -> Self {
        outputs.reverse();
        Self {
            outputs: Mutex::new(outputs),
            side_effect: Some(Arc::new(Mutex::new(effect))),
        }
    }
}

#[cfg(feature = "testing")]
impl Executor for MockExecutor {
    fn spawn(&self, invocation: &Invocation, _verbose: bool) -> Result<Box<dyn RunHandle>> {
        if let Some(ref effect) = self.side_effect {
            effect
                .lock()
                .map_err(|_| anyhow::anyhow!("MockExecutor side_effect mutex poisoned"))?(
                invocation,
            );
        }
        let output = self
            .outputs
            .lock()
            .map_err(|_| anyhow::anyhow!("MockExecutor mutex poisoned"))?
            .pop()
            .ok_or_else(|| anyhow::anyhow!("MockExecutor: no more outputs configured"))?;
        Ok(Box::new(MockRunHandle {
            output,
            wait_delay_ms: 0,
            ignores_sigterm: false,
            sigterm_called: Arc::new(AtomicBool::new(false)),
            sigkill_called: Arc::new(AtomicBool::new(false)),
        }))
    }
}

#[cfg(test)]
mod extract_tests {
    use super::*;

    #[test]
    fn extract_response_text_stream_json_last_result_event() {
        let output = concat!(
            r#"{"type":"system","subtype":"init","cwd":"/tmp"}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"}]}}"#,
            "\n",
            r#"{"type":"result","subtype":"success","total_cost_usd":0.01,"result":"The answer is 42","usage":{}}"#,
            "\n"
        );
        assert_eq!(extract_response_text(output), "The answer is 42");
    }

    #[test]
    fn extract_response_text_stream_json_uses_last_result() {
        // Multiple result events: use the last one.
        let output = concat!(
            r#"{"type":"result","subtype":"success","total_cost_usd":0.01,"result":"first","usage":{}}"#,
            "\n",
            r#"{"type":"result","subtype":"success","total_cost_usd":0.02,"result":"second","usage":{}}"#,
            "\n"
        );
        assert_eq!(extract_response_text(output), "second");
    }

    #[test]
    fn extract_response_text_legacy_single_json_object() {
        // Legacy --output-format json: single JSON blob.
        let output = r#"{"result":"legacy answer","total_cost_usd":0.01}"#;
        assert_eq!(extract_response_text(output), "legacy answer");
    }

    #[test]
    fn extract_response_text_non_json_fallback() {
        let output = "plain text output";
        assert_eq!(extract_response_text(output), "plain text output");
    }

    #[test]
    fn extract_response_text_stream_json_no_result_event_falls_back() {
        // Stream-json lines but no "type":"result" — fall back to raw output.
        let output = concat!(
            r#"{"type":"system","subtype":"init"}"#,
            "\n",
            r#"{"type":"assistant","message":{}}"#,
            "\n"
        );
        // No result event → legacy single-JSON fallback → also fails → raw output
        assert_eq!(extract_response_text(output), output);
    }
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use super::*;

    #[test]
    fn mock_returns_configured_outputs_in_order() {
        let mock = MockExecutor::new(vec![
            ExecutorOutput {
                combined: "output 1".to_string(),
                exit_code: 0,
            },
            ExecutorOutput {
                combined: "output 2".to_string(),
                exit_code: 0,
            },
        ]);
        let inv = Invocation {
            prompt: "p".to_string(),
            context_dir: ".".into(),
        };
        assert_eq!(mock.run(&inv, false).unwrap().combined, "output 1");
        assert_eq!(mock.run(&inv, false).unwrap().combined, "output 2");
    }

    #[test]
    fn mock_returns_error_on_nonzero_exit_code() {
        let mock = MockExecutor::new(vec![ExecutorOutput {
            combined: "quota exceeded".to_string(),
            exit_code: 1,
        }]);
        let inv = Invocation {
            prompt: "p".to_string(),
            context_dir: ".".into(),
        };
        let out = mock.run(&inv, false).unwrap();
        assert_eq!(out.exit_code, 1);
        assert!(out.combined.contains("quota exceeded"));
    }

    #[test]
    fn mock_returns_err_when_outputs_exhausted() {
        let mock = MockExecutor::new(vec![]);
        let inv = Invocation {
            prompt: "p".to_string(),
            context_dir: ".".into(),
        };
        assert!(mock.run(&inv, false).is_err());
    }
}
