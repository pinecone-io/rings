use crate::duration::DurationField;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;

/// Named error profile.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorProfileName {
    ClaudeCode,
    None,
}

/// Error profile for executor output classification.
/// Can be a named profile or a custom pattern table.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ErrorProfile {
    Named(ErrorProfileName),
    Custom {
        quota_patterns: Vec<String>,
        auth_patterns: Vec<String>,
    },
}

/// Pre-compiled error profile with regex patterns ready for matching.
#[derive(Debug, Clone)]
pub struct CompiledErrorProfile {
    pub quota_regexes: Vec<Regex>,
    pub auth_regexes: Vec<Regex>,
}

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
    /// Delay between full cycles.
    #[serde(default)]
    pub delay_between_cycles: u64,
    /// Enable quota backoff retry logic.
    #[serde(default)]
    pub quota_backoff: bool,
    /// Delay in seconds between quota backoff retries.
    #[serde(default)]
    pub quota_backoff_delay: u64,
    /// Maximum number of quota backoff retries.
    #[serde(default)]
    pub quota_backoff_max_retries: u32,
    /// Enable file manifest collection.
    #[serde(default)]
    pub manifest_enabled: bool,
    /// Glob patterns to exclude from manifests.
    #[serde(default)]
    pub manifest_ignore: Vec<String>,
    /// Use mtime optimization for manifest hashing.
    #[serde(default)]
    pub manifest_mtime_optimization: bool,
    /// Capture directory snapshot at each cycle boundary.
    #[serde(default)]
    pub snapshot_cycles: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutorConfig {
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub error_profile: Option<ErrorProfile>,
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
    pub delay_between_cycles: u64,
    pub phases: Vec<PhaseConfig>,
    pub executor: Option<ExecutorConfig>,
    /// Global budget cap in USD. Stops execution when cumulative cost reaches this amount.
    pub budget_cap_usd: Option<f64>,
    /// Global timeout for each executor subprocess, in seconds.
    pub timeout_per_run_secs: Option<u64>,
    /// Compiled error profile for executor output classification.
    pub compiled_error_profile: CompiledErrorProfile,
    /// Quota backoff configuration.
    pub quota_backoff: bool,
    pub quota_backoff_delay: u64,
    pub quota_backoff_max_retries: u32,
    /// File manifest configuration.
    pub manifest_enabled: bool,
    pub manifest_ignore: Vec<String>,
    pub manifest_mtime_optimization: bool,
    pub snapshot_cycles: bool,
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
    #[error("invalid regex pattern in error profile: {0}")]
    InvalidRegexPattern(String),
}

/// Compile an error profile into regex patterns.
fn compile_error_profile(
    profile: Option<&ErrorProfile>,
) -> Result<CompiledErrorProfile, WorkflowError> {
    match profile {
        None => {
            // Default to ClaudeCode when no executor or error_profile specified
            compile_claude_code_profile()
        }
        Some(ErrorProfile::Named(ErrorProfileName::ClaudeCode)) => compile_claude_code_profile(),
        Some(ErrorProfile::Named(ErrorProfileName::None)) => Ok(CompiledErrorProfile {
            quota_regexes: vec![],
            auth_regexes: vec![],
        }),
        Some(ErrorProfile::Custom {
            quota_patterns,
            auth_patterns,
        }) => {
            let mut quota_regexes = Vec::new();
            for pattern in quota_patterns {
                let regex = Regex::new(&format!("(?i){}", regex::escape(pattern)))
                    .map_err(|e| WorkflowError::InvalidRegexPattern(e.to_string()))?;
                quota_regexes.push(regex);
            }
            let mut auth_regexes = Vec::new();
            for pattern in auth_patterns {
                let regex = Regex::new(&format!("(?i){}", regex::escape(pattern)))
                    .map_err(|e| WorkflowError::InvalidRegexPattern(e.to_string()))?;
                auth_regexes.push(regex);
            }
            Ok(CompiledErrorProfile {
                quota_regexes,
                auth_regexes,
            })
        }
    }
}

/// Compile the built-in Claude Code error profile.
fn compile_claude_code_profile() -> Result<CompiledErrorProfile, WorkflowError> {
    let quota_patterns = vec![
        "usage limit reached",
        "rate limit",
        "quota exceeded",
        "too many requests",
        "429",
        "claude.ai/settings",
    ];
    let auth_patterns = vec![
        "authentication",
        "invalid api key",
        "unauthorized",
        "401",
        "please log in",
        "not logged in",
    ];

    let mut quota_regexes = Vec::new();
    for pattern in quota_patterns {
        let regex = Regex::new(&format!("(?i){}", regex::escape(pattern)))
            .map_err(|e| WorkflowError::InvalidRegexPattern(e.to_string()))?;
        quota_regexes.push(regex);
    }
    let mut auth_regexes = Vec::new();
    for pattern in auth_patterns {
        let regex = Regex::new(&format!("(?i){}", regex::escape(pattern)))
            .map_err(|e| WorkflowError::InvalidRegexPattern(e.to_string()))?;
        auth_regexes.push(regex);
    }

    Ok(CompiledErrorProfile {
        quota_regexes,
        auth_regexes,
    })
}

impl std::str::FromStr for Workflow {
    type Err = WorkflowError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let file: WorkflowFile = toml::from_str(s)?;
        Self::validate(file)
    }
}

impl Workflow {
    /// Return the structural fingerprint: phase names in declaration order.
    pub fn structural_fingerprint(&self) -> Vec<String> {
        self.phases.iter().map(|p| p.name.clone()).collect()
    }

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

        // Compile error profile from executor config or default.
        let compiled_error_profile = compile_error_profile(
            file.executor
                .as_ref()
                .and_then(|e| e.error_profile.as_ref()),
        )?;

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
            delay_between_cycles: file.workflow.delay_between_cycles,
            phases: file.phases,
            executor: file.executor,
            budget_cap_usd: file.workflow.budget_cap_usd,
            timeout_per_run_secs,
            compiled_error_profile,
            quota_backoff: file.workflow.quota_backoff,
            quota_backoff_delay: file.workflow.quota_backoff_delay,
            quota_backoff_max_retries: file.workflow.quota_backoff_max_retries,
            manifest_enabled: file.workflow.manifest_enabled,
            manifest_ignore: file.workflow.manifest_ignore,
            manifest_mtime_optimization: file.workflow.manifest_mtime_optimization,
            snapshot_cycles: file.workflow.snapshot_cycles,
        })
    }
}
