use crate::workflow::{CompletionSignalMode, Workflow};
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
