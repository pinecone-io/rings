use serde::Deserialize;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowFile {
    pub workflow: WorkflowConfig,
    #[serde(default)]
    pub phases: Vec<PhaseConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowConfig {
    pub completion_signal: String,
    pub context_dir: String,
    pub max_cycles: Option<u32>,
    pub output_dir: Option<String>,
    #[serde(default)]
    pub delay_between_runs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PhaseConfig {
    pub name: String,
    pub prompt: Option<String>,
    pub prompt_text: Option<String>,
    #[serde(default = "default_runs_per_cycle")]
    pub runs_per_cycle: u32,
}

fn default_runs_per_cycle() -> u32 {
    1
}

/// Validated, ready-to-use workflow.
#[derive(Debug, Clone)]
pub struct Workflow {
    pub completion_signal: String,
    pub context_dir: String,
    pub max_cycles: u32,
    pub output_dir: Option<String>,
    pub delay_between_runs: u64,
    pub phases: Vec<PhaseConfig>,
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
        }
        Ok(Workflow {
            completion_signal: file.workflow.completion_signal,
            context_dir: file.workflow.context_dir,
            max_cycles,
            output_dir: file.workflow.output_dir,
            delay_between_runs: file.workflow.delay_between_runs,
            phases: file.phases,
        })
    }
}
