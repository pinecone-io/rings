use crate::duration::DurationField;
use crate::workflow::{CompletionSignalMode, GateAction, GateConfig, Workflow};
use serde::{Deserialize, Serialize};

/// Result of checking for completion signal in a prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalCheck {
    /// Whether the signal was found
    pub found: bool,
    /// For file-based prompts, line number where signal appears (1-indexed).
    /// For inline `prompt_text`, byte offset within the TOML value.
    pub line_number: Option<u32>,
}

/// Information about one phase in the dry run plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunPhase {
    pub name: String,
    pub prompt_source: String,
    pub runs_per_cycle: u32,
    pub signal_check: SignalCheck,
    pub unknown_vars: Vec<String>,
}

/// Complete dry run execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunPlan {
    pub phases: Vec<DryRunPhase>,
    pub runs_per_cycle_total: u32,
    pub max_cycles: Option<u32>,
    pub max_total_runs: Option<u32>,
    pub completion_signal: String,
}

/// JSONL event wrapper for dry run plan
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DryRunPlanEvent {
    pub event: String,
    pub plan: DryRunPlan,
    pub timestamp: String,
}

impl DryRunPlan {
    /// Build a dry run plan from the workflow
    pub fn from_workflow(workflow: &Workflow, _workflow_file: &str) -> anyhow::Result<DryRunPlan> {
        let mut phases = Vec::new();
        let mut runs_per_cycle_total = 0;

        for phase in &workflow.phases {
            let prompt_source = if let Some(ref prompt_file) = phase.prompt {
                prompt_file.clone()
            } else {
                "<inline prompt_text>".to_string()
            };

            let prompt_content = if let Some(ref prompt_file) = phase.prompt {
                std::fs::read_to_string(prompt_file).map_err(|e| {
                    anyhow::anyhow!("Cannot read prompt file {}: {}", prompt_file, e)
                })?
            } else if let Some(ref prompt_text) = phase.prompt_text {
                prompt_text.clone()
            } else {
                return Err(anyhow::anyhow!("Phase {} has no prompt", phase.name));
            };

            let signal_check = check_completion_signal(
                &workflow.completion_signal,
                &workflow.completion_signal_mode,
                &prompt_content,
            );

            // Scan for unknown variables (F-029)
            let unknown_vars = crate::template::find_unknown_variables(
                &prompt_content,
                crate::template::KNOWN_VARS,
            );

            phases.push(DryRunPhase {
                name: phase.name.clone(),
                prompt_source,
                runs_per_cycle: phase.runs_per_cycle,
                signal_check,
                unknown_vars,
            });

            runs_per_cycle_total += phase.runs_per_cycle;
        }

        let max_total_runs = workflow.max_cycles.checked_mul(runs_per_cycle_total);

        Ok(DryRunPlan {
            phases,
            runs_per_cycle_total,
            max_cycles: Some(workflow.max_cycles),
            max_total_runs,
            completion_signal: workflow.completion_signal.clone(),
        })
    }
}

/// Format a `DurationField` for display in dry-run output.
/// Returns the raw string representation, defaulting to "30s" if absent.
pub fn format_gate_timeout(timeout: Option<&DurationField>) -> String {
    match timeout {
        None => "30s".to_string(),
        Some(DurationField::Secs(n)) => format!("{}s", n),
        Some(DurationField::Str(s)) => s.clone(),
    }
}

/// Format a gate config line for dry-run human output.
///
/// `default_on_fail` is the action name used when `gate.on_fail` is `None`
/// (e.g., `"stop"` for cycle gates, `"skip"` for phase gates).
pub fn format_gate_config_line(gate: &GateConfig, default_on_fail: &str) -> String {
    let on_fail = gate
        .on_fail
        .as_ref()
        .map(|a| a.to_string())
        .unwrap_or_else(|| default_on_fail.to_string());
    let timeout = format_gate_timeout(gate.timeout.as_ref());
    // Truncate command display to 80 chars
    let cmd = if gate.command.len() > 80 {
        format!("{}...", &gate.command[..80])
    } else {
        gate.command.clone()
    };
    format!(
        "command: `{}`, on_fail: {}, timeout: {}",
        cmd, on_fail, timeout
    )
}

/// Returns the display string for a GateAction.
pub fn gate_action_display(action: Option<&GateAction>, default: &str) -> String {
    action
        .map(|a| a.to_string())
        .unwrap_or_else(|| default.to_string())
}

/// Check if completion signal exists in prompt.
/// For regex mode, searches for the literal pattern string as a substring (not running
/// the regex against the prompt text — this is intentional per spec).
fn check_completion_signal(
    signal: &str,
    mode: &CompletionSignalMode,
    prompt_content: &str,
) -> SignalCheck {
    match mode {
        CompletionSignalMode::Line => {
            // Signal must appear on its own line
            for (line_idx, line) in prompt_content.lines().enumerate() {
                if line.trim() == signal {
                    return SignalCheck {
                        found: true,
                        line_number: Some((line_idx + 1) as u32),
                    };
                }
            }
            SignalCheck {
                found: false,
                line_number: None,
            }
        }
        CompletionSignalMode::Substring | CompletionSignalMode::Regex(_) => {
            // Substring: literal substring search.
            // Regex: search for literal pattern string as substring (not running regex against prompt).
            let line_number = prompt_content
                .lines()
                .position(|line| line.contains(signal))
                .map(|idx| (idx + 1) as u32);

            SignalCheck {
                found: line_number.is_some(),
                line_number,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::{GateAction, GateConfig};

    fn make_gate(
        command: &str,
        on_fail: Option<GateAction>,
        timeout: Option<DurationField>,
    ) -> GateConfig {
        GateConfig {
            command: command.to_string(),
            on_fail,
            timeout,
        }
    }

    #[test]
    fn format_gate_timeout_none_returns_30s() {
        assert_eq!(format_gate_timeout(None), "30s");
    }

    #[test]
    fn format_gate_timeout_secs_displays_with_suffix() {
        assert_eq!(format_gate_timeout(Some(&DurationField::Secs(10))), "10s");
    }

    #[test]
    fn format_gate_timeout_str_displays_as_is() {
        assert_eq!(
            format_gate_timeout(Some(&DurationField::Str("5m".to_string()))),
            "5m"
        );
    }

    #[test]
    fn format_gate_config_line_cycle_gate_default_on_fail() {
        let gate = make_gate("true", None, None);
        let line = format_gate_config_line(&gate, "stop");
        assert_eq!(line, "command: `true`, on_fail: stop, timeout: 30s");
    }

    #[test]
    fn format_gate_config_line_phase_gate_default_on_fail() {
        let gate = make_gate("test -f foo", None, None);
        let line = format_gate_config_line(&gate, "skip");
        assert_eq!(line, "command: `test -f foo`, on_fail: skip, timeout: 30s");
    }

    #[test]
    fn format_gate_config_line_explicit_on_fail_stop() {
        let gate = make_gate(
            "./check.sh",
            Some(GateAction::Stop),
            Some(DurationField::Secs(10)),
        );
        let line = format_gate_config_line(&gate, "skip");
        assert_eq!(line, "command: `./check.sh`, on_fail: stop, timeout: 10s");
    }

    #[test]
    fn format_gate_config_line_explicit_on_fail_error() {
        let gate = make_gate("./check.sh", Some(GateAction::Error), None);
        let line = format_gate_config_line(&gate, "stop");
        assert_eq!(line, "command: `./check.sh`, on_fail: error, timeout: 30s");
    }

    #[test]
    fn format_gate_config_line_truncates_long_command() {
        let long_cmd = "a".repeat(100);
        let gate = make_gate(&long_cmd, None, None);
        let line = format_gate_config_line(&gate, "stop");
        // Command should be truncated to 80 chars + "..."
        assert!(
            line.contains("aaa..."),
            "long command should be truncated with ..."
        );
        let displayed_cmd: &str = line
            .trim_start_matches("command: `")
            .split('`')
            .next()
            .unwrap();
        assert_eq!(displayed_cmd.len(), 83); // 80 chars + "..."
    }

    #[test]
    fn format_gate_config_line_does_not_execute_commands() {
        // Verify format_gate_config_line is pure formatting — it returns a string
        // without spawning any processes. We use a side-effectful command as evidence:
        // if it were executed, it would create a file; we verify no such file appears.
        use std::env::temp_dir;
        let sentinel = temp_dir().join("rings_gate_dry_run_sentinel_test");
        let _ = std::fs::remove_file(&sentinel);

        let cmd = format!("touch {}", sentinel.display());
        let gate = make_gate(&cmd, None, None);
        let _ = format_gate_config_line(&gate, "stop");

        assert!(
            !sentinel.exists(),
            "format_gate_config_line must not execute the gate command"
        );
    }

    #[test]
    fn dry_run_plan_cycle_gate_present_in_workflow() {
        use crate::workflow::Workflow;
        use std::str::FromStr;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let ctx = dir.path().to_str().unwrap();
        let toml = format!(
            r#"
[workflow]
completion_signal = "DONE"
context_dir = "{ctx}"
max_cycles = 5
cycle_gate = {{ command = "true", on_fail = "stop" }}

[[phases]]
name = "builder"
prompt_text = "Do work."
"#
        );
        let wf = Workflow::from_str(&toml).unwrap();
        assert!(wf.cycle_gate.is_some());
        let cg = wf.cycle_gate.as_ref().unwrap();
        let line = format_gate_config_line(cg, "stop");
        assert!(
            line.contains("`true`"),
            "cycle gate line should show command"
        );
        assert!(
            line.contains("on_fail: stop"),
            "cycle gate line should show on_fail"
        );
    }

    #[test]
    fn dry_run_plan_phase_gate_present_in_workflow() {
        use crate::workflow::Workflow;
        use std::str::FromStr;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let ctx = dir.path().to_str().unwrap();
        let toml = format!(
            r#"
[workflow]
completion_signal = "DONE"
context_dir = "{ctx}"
max_cycles = 5

[[phases]]
name = "planner"
prompt_text = "Plan."
gate = {{ command = "test -f plan.md" }}

[[phases]]
name = "builder"
prompt_text = "Build."
"#
        );
        let wf = Workflow::from_str(&toml).unwrap();
        let planner = &wf.phases[0];
        assert!(planner.gate.is_some());
        let gate = planner.gate.as_ref().unwrap();
        let line = format_gate_config_line(gate, "skip");
        assert!(
            line.contains("`test -f plan.md`"),
            "phase gate line should show command"
        );
        assert!(
            line.contains("on_fail: skip"),
            "phase gate line should use skip default"
        );

        // Second phase has no gate
        assert!(wf.phases[1].gate.is_none());
    }
}
