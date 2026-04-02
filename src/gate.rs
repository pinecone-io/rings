use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::workflow::GateConfig;

/// Result of evaluating a deterministic gate command.
pub struct GateResult {
    pub command: String,
    pub exit_code: i32,
    /// True if the gate passed (exit code 0).
    pub passed: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Default timeout for gate commands when none is specified in the config.
const DEFAULT_GATE_TIMEOUT_SECS: u64 = 30;

/// Grace period between SIGTERM and SIGKILL.
const SIGKILL_GRACE_SECS: u64 = 5;

/// Send SIGTERM to the process group identified by `pgid`.
/// Sending to -pgid kills all processes in that group (the shell and any children it forked).
#[cfg(unix)]
fn sigterm_group(pgid: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    let _ = kill(Pid::from_raw(-(pgid as i32)), Signal::SIGTERM);
}

/// Send SIGKILL to the process group identified by `pgid`.
#[cfg(unix)]
fn sigkill_group(pgid: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    let _ = kill(Pid::from_raw(-(pgid as i32)), Signal::SIGKILL);
}

#[cfg(not(unix))]
fn sigterm_group(_pgid: u32) {}

#[cfg(not(unix))]
fn sigkill_group(_pgid: u32) {}

/// Evaluate a deterministic gate command.
///
/// Spawns `sh -c <command>` in `context_dir`, captures stdout and stderr,
/// and enforces the gate's configured timeout (default: 30 seconds).
/// On timeout: SIGTERM is sent to the process group, followed by SIGKILL after a 5-second
/// grace period. A timed-out gate counts as failure with exit_code -1.
///
/// Gate commands inherit the full process environment.
pub fn evaluate_gate(gate: &GateConfig, context_dir: &Path) -> Result<GateResult> {
    let timeout_secs = match &gate.timeout {
        Some(d) => d.to_secs()?,
        None => DEFAULT_GATE_TIMEOUT_SECS,
    };

    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(&gate.command)
        .current_dir(context_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Place the child in its own process group so we can signal the entire group
    // (sh + any children it forks, e.g. `sleep 60`) on timeout.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn gate command: {:?}", gate.command))?;

    let stdout = child.stdout.take().context("failed to get gate stdout")?;
    let stderr = child.stderr.take().context("failed to get gate stderr")?;

    // The process group ID equals the child's PID because we called process_group(0).
    let pgid = child.id();

    let stdout_buf = Arc::new(Mutex::new(String::new()));
    let stderr_buf = Arc::new(Mutex::new(String::new()));

    let stdout_clone = Arc::clone(&stdout_buf);
    let stdout_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(mut buf) = stdout_clone.lock() {
                buf.push_str(&line);
                buf.push('\n');
            }
        }
    });

    let stderr_clone = Arc::clone(&stderr_buf);
    let stderr_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(mut buf) = stderr_clone.lock() {
                buf.push_str(&line);
                buf.push('\n');
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    let exit_code = loop {
        match child.try_wait().context("failed to poll gate subprocess")? {
            Some(status) => break status.code().unwrap_or(-1),
            None => {
                if Instant::now() >= deadline {
                    // Timeout: SIGTERM the entire process group, then SIGKILL after grace period.
                    sigterm_group(pgid);

                    let kill_deadline = Instant::now() + Duration::from_secs(SIGKILL_GRACE_SECS);
                    loop {
                        match child.try_wait() {
                            Ok(Some(_)) => break,
                            _ => {
                                if Instant::now() >= kill_deadline {
                                    sigkill_group(pgid);
                                    let _ = child.wait();
                                    break;
                                }
                                std::thread::sleep(Duration::from_millis(50));
                            }
                        }
                    }
                    break -1_i32;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };

    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    let stdout_str = stdout_buf
        .lock()
        .map_err(|_| anyhow::anyhow!("gate stdout mutex poisoned"))?
        .clone();
    let stderr_str = stderr_buf
        .lock()
        .map_err(|_| anyhow::anyhow!("gate stderr mutex poisoned"))?
        .clone();

    Ok(GateResult {
        command: gate.command.clone(),
        exit_code,
        passed: exit_code == 0,
        stdout: stdout_str,
        stderr: stderr_str,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::DurationField;
    use crate::workflow::GateConfig;

    fn gate(command: &str) -> GateConfig {
        GateConfig {
            command: command.to_string(),
            on_fail: None,
            timeout: None,
        }
    }

    fn gate_with_timeout(command: &str, secs: u64) -> GateConfig {
        GateConfig {
            command: command.to_string(),
            on_fail: None,
            timeout: Some(DurationField::Secs(secs)),
        }
    }

    #[test]
    fn true_passes_with_exit_code_zero() {
        let result = evaluate_gate(&gate("true"), Path::new("/tmp")).unwrap();
        assert!(result.passed);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn false_fails_with_exit_code_one() {
        let result = evaluate_gate(&gate("false"), Path::new("/tmp")).unwrap();
        assert!(!result.passed);
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn echo_hello_passes_and_stdout_contains_hello() {
        let result = evaluate_gate(&gate("echo hello"), Path::new("/tmp")).unwrap();
        assert!(result.passed);
        assert_eq!(result.exit_code, 0);
        assert!(
            result.stdout.contains("hello"),
            "stdout: {:?}",
            result.stdout
        );
    }

    #[test]
    fn exit_42_fails_with_exit_code_42() {
        let result = evaluate_gate(&gate("exit 42"), Path::new("/tmp")).unwrap();
        assert!(!result.passed);
        assert_eq!(result.exit_code, 42);
    }

    #[test]
    fn timeout_exceeded_returns_failure_with_sentinel_exit_code() {
        // sleep 60 with a 1-second timeout; the process group is killed after timeout.
        let result = evaluate_gate(&gate_with_timeout("sleep 60", 1), Path::new("/tmp")).unwrap();
        assert!(!result.passed);
        assert_eq!(result.exit_code, -1, "timed-out gate must use -1 sentinel");
    }

    #[test]
    fn gate_runs_in_specified_context_dir() {
        let dir = std::env::temp_dir();
        // `pwd` outputs the working directory; verify it matches context_dir.
        let result = evaluate_gate(&gate("pwd"), &dir).unwrap();
        assert!(result.passed);
        let canonical_dir = dir.canonicalize().unwrap();
        let canonical_dir_str = canonical_dir.to_string_lossy();
        let stdout_trimmed = result.stdout.trim();
        assert!(
            stdout_trimmed == canonical_dir_str || stdout_trimmed == dir.to_string_lossy(),
            "expected pwd to be {:?} but got {:?}",
            canonical_dir_str,
            stdout_trimmed
        );
    }
}
