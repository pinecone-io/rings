use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

#[cfg(feature = "testing")]
use std::cell::RefCell;

pub struct Invocation {
    pub prompt: String,
    pub context_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ExecutorOutput {
    pub combined: String,
    pub exit_code: i32,
}

pub trait Executor {
    fn run(&self, invocation: &Invocation, verbose: bool) -> Result<ExecutorOutput>;
}

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
        ]
    }
}

impl Executor for ClaudeExecutor {
    fn run(&self, invocation: &Invocation, verbose: bool) -> Result<ExecutorOutput> {
        let mut child = Command::new("claude")
            .args(Self::build_args())
            .current_dir(&invocation.context_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn claude subprocess")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(invocation.prompt.as_bytes())
                .context("Failed to write prompt to claude stdin")?;
            // stdin dropped here → EOF sent to claude
        }

        if verbose {
            // Live streaming mode: read lines as they come and print to stderr
            let stdout = child.stdout.take().context("Failed to get stdout")?;
            let stderr = child.stderr.take().context("Failed to get stderr")?;

            let stdout_output = Arc::new(Mutex::new(String::new()));
            let stderr_output = Arc::new(Mutex::new(String::new()));

            let stdout_accum = Arc::clone(&stdout_output);
            let stderr_accum = Arc::clone(&stderr_output);

            let stdout_thread = std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    eprintln!("{}", line);
                    if let Ok(mut acc) = stdout_accum.lock() {
                        acc.push_str(&line);
                        acc.push('\n');
                    }
                }
            });

            let stderr_thread = std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    eprintln!("{}", line);
                    if let Ok(mut acc) = stderr_accum.lock() {
                        acc.push_str(&line);
                        acc.push('\n');
                    }
                }
            });

            let output = child
                .wait_with_output()
                .context("Failed to wait for claude subprocess")?;

            // Wait for threads to finish reading
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();

            let stdout_str = stdout_output
                .lock()
                .map_err(|_| anyhow::anyhow!("stdout accumulator mutex was poisoned"))?
                .clone();
            let stderr_str = stderr_output
                .lock()
                .map_err(|_| anyhow::anyhow!("stderr accumulator mutex was poisoned"))?
                .clone();
            let combined = format!("{}\n{}", stdout_str, stderr_str);

            Ok(ExecutorOutput {
                combined,
                exit_code: output.status.code().unwrap_or(-1),
            })
        } else {
            // Non-verbose mode: capture output and return after process completes
            let output = child
                .wait_with_output()
                .context("Failed to wait for claude subprocess")?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{stdout}\n{stderr}");

            Ok(ExecutorOutput {
                combined,
                exit_code: output.status.code().unwrap_or(-1),
            })
        }
    }
}

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
    fn run(&self, invocation: &Invocation, verbose: bool) -> Result<ExecutorOutput> {
        let mut child = Command::new(&self.binary)
            .args(&self.args)
            .current_dir(&invocation.context_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn executor: {}", self.binary))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(invocation.prompt.as_bytes())
                .context("Failed to write prompt to executor stdin")?;
        }

        if verbose {
            let stdout = child.stdout.take().context("Failed to get stdout")?;
            let stderr = child.stderr.take().context("Failed to get stderr")?;

            let stdout_output = Arc::new(Mutex::new(String::new()));
            let stderr_output = Arc::new(Mutex::new(String::new()));

            let stdout_accum = Arc::clone(&stdout_output);
            let stderr_accum = Arc::clone(&stderr_output);

            let stdout_thread = std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    eprintln!("{}", line);
                    if let Ok(mut acc) = stdout_accum.lock() {
                        acc.push_str(&line);
                        acc.push('\n');
                    }
                }
            });

            let stderr_thread = std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    eprintln!("{}", line);
                    if let Ok(mut acc) = stderr_accum.lock() {
                        acc.push_str(&line);
                        acc.push('\n');
                    }
                }
            });

            let output = child
                .wait_with_output()
                .context("Failed to wait for executor subprocess")?;

            let _ = stdout_thread.join();
            let _ = stderr_thread.join();

            let stdout_str = stdout_output
                .lock()
                .map_err(|_| anyhow::anyhow!("stdout accumulator mutex was poisoned"))?
                .clone();
            let stderr_str = stderr_output
                .lock()
                .map_err(|_| anyhow::anyhow!("stderr accumulator mutex was poisoned"))?
                .clone();
            let combined = format!("{}\n{}", stdout_str, stderr_str);

            Ok(ExecutorOutput {
                combined,
                exit_code: output.status.code().unwrap_or(-1),
            })
        } else {
            let output = child
                .wait_with_output()
                .context("Failed to wait for executor subprocess")?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{stdout}\n{stderr}");

            Ok(ExecutorOutput {
                combined,
                exit_code: output.status.code().unwrap_or(-1),
            })
        }
    }
}

/// Test-only executor: returns pre-configured outputs in sequence.
#[cfg(feature = "testing")]
pub struct MockExecutor {
    outputs: RefCell<Vec<ExecutorOutput>>,
}

#[cfg(feature = "testing")]
impl MockExecutor {
    pub fn new(mut outputs: Vec<ExecutorOutput>) -> Self {
        outputs.reverse(); // pop from back = FIFO
        Self {
            outputs: RefCell::new(outputs),
        }
    }
}

#[cfg(feature = "testing")]
impl Executor for MockExecutor {
    fn run(&self, _invocation: &Invocation, _verbose: bool) -> Result<ExecutorOutput> {
        self.outputs
            .borrow_mut()
            .pop()
            .ok_or_else(|| anyhow::anyhow!("MockExecutor: no more outputs configured"))
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
