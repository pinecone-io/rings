use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

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
    fn run(&self, invocation: &Invocation) -> Result<ExecutorOutput>;
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
    fn run(&self, invocation: &Invocation) -> Result<ExecutorOutput> {
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
    fn run(&self, _invocation: &Invocation) -> Result<ExecutorOutput> {
        self.outputs
            .borrow_mut()
            .pop()
            .ok_or_else(|| anyhow::anyhow!("MockExecutor: no more outputs configured"))
    }
}

#[cfg(test)]
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
        assert_eq!(mock.run(&inv).unwrap().combined, "output 1");
        assert_eq!(mock.run(&inv).unwrap().combined, "output 2");
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
        let out = mock.run(&inv).unwrap();
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
        assert!(mock.run(&inv).is_err());
    }

    // Security: prompts go via stdin only (see CLAUDE.md and specs/execution/executor-integration.md).
    // This test pins the exact arg list. Any PR that adds a runtime-content arg must update this test,
    // making the security regression visible in review.
    #[test]
    fn claude_executor_never_puts_prompt_in_args() {
        // Build args must not accept a prompt parameter at all.
        // The fixed arg list must not contain any user-controlled content.
        let args = ClaudeExecutor::build_args();
        // Verify the fixed args are exactly what we expect — any addition
        // that accepts user content would be a security regression.
        assert_eq!(
            args,
            vec![
                "--dangerously-skip-permissions".to_string(),
                "-p".to_string(),
                "-".to_string(),
            ]
        );
        // Verify no arg is a template that could accept runtime content.
        for arg in &args {
            assert!(
                !arg.contains('{'),
                "arg contains template placeholder: {arg}"
            );
            assert!(!arg.contains('%'), "arg contains format placeholder: {arg}");
        }
    }
}
