use crate::duration::DurationField;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowFile {
    pub workflow: WorkflowConfig,
    #[serde(default)]
    pub phases: Vec<PhaseConfig>,
    pub executor: Option<ExecutorConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowConfig {
    pub completion_signal: String,
    pub context_dir: String,
    pub max_cycles: Option<u32>,
    pub output_dir: Option<String>,
    #[serde(default)]
    pub delay_between_runs: u64,
    /// When a phase emits this signal, skip remaining phases in the current cycle.
    pub continue_signal: Option<String>,
    /// Phase names from which the completion signal may fire. Empty = any phase.
    #[serde(default)]
    pub completion_signal_phases: Vec<String>,
    /// "line" (signal must appear alone on a line) or "substring" (default).
    pub completion_signal_mode: Option<String>,
    /// Stop execution if cumulative cost reaches this amount in USD.
    pub budget_cap_usd: Option<f64>,
    /// Timeout for each individual executor subprocess invocation.
    pub timeout_per_run_secs: Option<DurationField>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutorConfig {
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PhaseConfig {
    pub name: String,
    pub prompt: Option<String>,
    pub prompt_text: Option<String>,
    #[serde(default = "default_runs_per_cycle")]
    pub runs_per_cycle: u32,
    /// Per-phase budget cap in USD.
    pub budget_cap_usd: Option<f64>,
    /// Per-phase subprocess timeout. Overrides the global timeout_per_run_secs for this phase.
    pub timeout_per_run_secs: Option<DurationField>,
}

fn default_runs_per_cycle() -> u32 {
    1
}

/// Validated, ready-to-use workflow.
#[derive(Debug, Clone)]
pub struct Workflow {
    pub completion_signal: String,
    pub continue_signal: Option<String>,
    /// Phase names from which completion may fire. Empty = any phase.
    pub completion_signal_phases: Vec<String>,
    /// "line" or "substring"
    pub completion_signal_mode: String,
    pub context_dir: String,
    pub max_cycles: u32,
    pub output_dir: Option<String>,
    pub delay_between_runs: u64,
    pub phases: Vec<PhaseConfig>,
    pub executor: Option<ExecutorConfig>,
    /// Global budget cap in USD. Stops execution when cumulative cost reaches this amount.
    pub budget_cap_usd: Option<f64>,
    /// Global timeout for each executor subprocess, in seconds.
    pub timeout_per_run_secs: Option<u64>,
}

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("completion_signal must not be empty")]
    EmptyCompletionSignal,
    #[error("workflow must have at least one phase")]
    NoPhases,
    #[error("duplicate phase name: {0}")]
    DuplicatePhaseName(String),
    #[error("runs_per_cycle must be >= 1 for phase '{0}'")]
    InvalidRunsPerCycle(String),
    #[error("phase '{0}' specifies both 'prompt' and 'prompt_text'; use one or the other")]
    AmbiguousPrompt(String),
    #[error("phase '{0}' must specify either 'prompt' or 'prompt_text'")]
    MissingPrompt(String),
    #[error("max_cycles is required in MVP; unlimited mode not yet supported")]
    MissingMaxCycles,
    #[error("budget_cap_usd must be greater than zero")]
    InvalidBudgetCap,
    #[error("invalid duration for {field}: {message}")]
    InvalidDuration { field: String, message: String },
    #[error("context_dir does not exist or is not a directory: {0}")]
    ContextDirNotFound(String),
    #[error("TOML parse error: {0}")]
    ParseError(#[from] toml::de::Error),
}

impl std::str::FromStr for Workflow {
    type Err = WorkflowError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let file: WorkflowFile = toml::from_str(s)?;
        Self::validate(file)
    }
}

impl Workflow {
    fn validate(file: WorkflowFile) -> Result<Self, WorkflowError> {
        if file.workflow.completion_signal.is_empty() {
            return Err(WorkflowError::EmptyCompletionSignal);
        }
        let max_cycles = file
            .workflow
            .max_cycles
            .ok_or(WorkflowError::MissingMaxCycles)?;
        if file.phases.is_empty() {
            return Err(WorkflowError::NoPhases);
        }

        // Validate context_dir exists and is a directory.
        if !Path::new(&file.workflow.context_dir).is_dir() {
            return Err(WorkflowError::ContextDirNotFound(
                file.workflow.context_dir.clone(),
            ));
        }

        // Validate and resolve global budget_cap_usd.
        if let Some(cap) = file.workflow.budget_cap_usd {
            if cap <= 0.0 {
                return Err(WorkflowError::InvalidBudgetCap);
            }
        }

        // Validate and resolve global timeout_per_run_secs.
        let timeout_per_run_secs = match &file.workflow.timeout_per_run_secs {
            Some(d) => Some(d.to_secs().map_err(|e| WorkflowError::InvalidDuration {
                field: "timeout_per_run_secs".to_string(),
                message: e.to_string(),
            })?),
            None => None,
        };

        let mut seen = HashSet::new();
        for phase in &file.phases {
            if !seen.insert(phase.name.clone()) {
                return Err(WorkflowError::DuplicatePhaseName(phase.name.clone()));
            }
            if phase.runs_per_cycle == 0 {
                return Err(WorkflowError::InvalidRunsPerCycle(phase.name.clone()));
            }
            match (&phase.prompt, &phase.prompt_text) {
                (Some(_), Some(_)) => {
                    return Err(WorkflowError::AmbiguousPrompt(phase.name.clone()))
                }
                (None, None) => return Err(WorkflowError::MissingPrompt(phase.name.clone())),
                _ => {}
            }
            // Validate per-phase budget_cap_usd.
            if let Some(cap) = phase.budget_cap_usd {
                if cap <= 0.0 {
                    return Err(WorkflowError::InvalidBudgetCap);
                }
            }
            // Validate per-phase timeout_per_run_secs.
            if let Some(ref d) = phase.timeout_per_run_secs {
                d.to_secs().map_err(|e| WorkflowError::InvalidDuration {
                    field: format!("phase '{}' timeout_per_run_secs", phase.name),
                    message: e.to_string(),
                })?;
            }
        }
        Ok(Workflow {
            completion_signal: file.workflow.completion_signal,
            continue_signal: file.workflow.continue_signal,
            completion_signal_phases: file.workflow.completion_signal_phases,
            completion_signal_mode: file
                .workflow
                .completion_signal_mode
                .unwrap_or_else(|| "substring".to_string()),
            context_dir: file.workflow.context_dir,
            max_cycles,
            output_dir: file.workflow.output_dir,
            delay_between_runs: file.workflow.delay_between_runs,
            phases: file.phases,
            executor: file.executor,
            budget_cap_usd: file.workflow.budget_cap_usd,
            timeout_per_run_secs,
        })
    }
}
