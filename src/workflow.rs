use crate::duration::DurationField;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;

/// How the completion signal is matched against executor output.
#[derive(Debug, Clone, Default)]
pub enum CompletionSignalMode {
    /// Signal is a substring of the output (default).
    #[default]
    Substring,
    /// Signal must appear alone on a trimmed line.
    Line,
    /// Signal is a compiled regex matched against the full output.
    Regex(Regex),
}

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
    pub delay_between_runs: Option<DurationField>,
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
    pub delay_between_cycles: Option<DurationField>,
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
    /// Glob patterns that this phase expects to be present in context_dir.
    #[serde(default)]
    pub consumes: Vec<String>,
    /// Glob patterns that this phase is expected to produce (add or modify).
    #[serde(default)]
    pub produces: Vec<String>,
    /// If true, failure to satisfy any produces pattern causes a hard exit.
    #[serde(default)]
    pub produces_required: bool,
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
    /// Compiled completion signal match mode.
    pub completion_signal_mode: CompletionSignalMode,
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
    #[error("budget_cap_usd must be a finite positive number")]
    InvalidBudgetCap,
    #[error("invalid duration for {field}: {message}")]
    InvalidDuration { field: String, message: String },
    #[error("context_dir does not exist or is not a directory: {0}")]
    ContextDirNotFound(String),
    #[error("TOML parse error: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("invalid regex pattern in error profile: {0}")]
    InvalidRegexPattern(String),
    #[error("invalid completion_signal_mode: '{0}'; expected 'substring', 'line', or 'regex'")]
    InvalidCompletionSignalMode(String),
    #[error("invalid regex in completion_signal: {0}")]
    InvalidSignalRegex(String),
    #[error("completion_signal_phases references unknown phase: '{0}'")]
    UnknownCompletionSignalPhase(String),
    #[error("phase '{0}' has produces_required = true but manifest_enabled = false")]
    ProducesRequiredWithoutManifest(String),
    #[error("output_dir contains path traversal ('..') which is not allowed")]
    OutputDirContainsParentDir,
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

    /// Detect the model name from executor args.
    ///
    /// Scans `executor.args` for `--model` followed by a value. Returns `Some(model_name)`
    /// when a single consistent model is found, or `None` when no `--model` flag is present.
    pub fn detect_model_name(&self) -> Option<String> {
        let args = self.executor.as_ref().map(|e| e.args.as_slice())?;
        let mut i = 0;
        while i < args.len() {
            if args[i] == "--model" {
                if i + 1 < args.len() {
                    return Some(args[i + 1].clone());
                }
            } else if let Some(val) = args[i].strip_prefix("--model=") {
                return Some(val.to_string());
            }
            i += 1;
        }
        None
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

        // Validate output_dir does not contain '..' path traversal.
        if let Some(ref dir) = file.workflow.output_dir {
            if Path::new(dir)
                .components()
                .any(|c| c == std::path::Component::ParentDir)
            {
                return Err(WorkflowError::OutputDirContainsParentDir);
            }
        }

        // Validate and resolve global budget_cap_usd.
        if let Some(cap) = file.workflow.budget_cap_usd {
            if cap.is_nan() || cap.is_infinite() || cap <= 0.0 {
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

        // Parse and compile completion_signal_mode.
        let completion_signal_mode = match file
            .workflow
            .completion_signal_mode
            .as_deref()
            .unwrap_or("substring")
        {
            "substring" => CompletionSignalMode::Substring,
            "line" => CompletionSignalMode::Line,
            "regex" => {
                let re = Regex::new(&file.workflow.completion_signal)
                    .map_err(|e| WorkflowError::InvalidSignalRegex(e.to_string()))?;
                CompletionSignalMode::Regex(re)
            }
            other => {
                return Err(WorkflowError::InvalidCompletionSignalMode(
                    other.to_string(),
                ))
            }
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
                if cap.is_nan() || cap.is_infinite() || cap <= 0.0 {
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
            // Validate produces_required requires manifest_enabled.
            if phase.produces_required && !file.workflow.manifest_enabled {
                return Err(WorkflowError::ProducesRequiredWithoutManifest(
                    phase.name.clone(),
                ));
            }
        }

        // Validate completion_signal_phases: each name must be a known phase.
        for phase_name in &file.workflow.completion_signal_phases {
            if !seen.contains(phase_name) {
                return Err(WorkflowError::UnknownCompletionSignalPhase(
                    phase_name.clone(),
                ));
            }
        }

        let delay_between_runs = match file.workflow.delay_between_runs {
            None => 0,
            Some(d) => d.to_secs().map_err(|e| WorkflowError::InvalidDuration {
                field: "delay_between_runs".to_string(),
                message: e.to_string(),
            })?,
        };
        let delay_between_cycles = match file.workflow.delay_between_cycles {
            None => 0,
            Some(d) => d.to_secs().map_err(|e| WorkflowError::InvalidDuration {
                field: "delay_between_cycles".to_string(),
                message: e.to_string(),
            })?,
        };

        Ok(Workflow {
            completion_signal: file.workflow.completion_signal,
            continue_signal: file.workflow.continue_signal,
            completion_signal_phases: file.workflow.completion_signal_phases,
            completion_signal_mode,
            context_dir: file.workflow.context_dir,
            max_cycles,
            output_dir: file.workflow.output_dir,
            delay_between_runs,
            delay_between_cycles,
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use tempfile::tempdir;

    /// Build a minimal valid TOML string for testing, using a temp dir as context_dir.
    fn make_toml(context_dir: &str, extra: &str) -> String {
        format!(
            r#"
[workflow]
completion_signal = "DONE"
context_dir = "{}"
max_cycles = 3
{}

[[phases]]
name = "builder"
prompt_text = "Do the work. When done, print DONE."
"#,
            context_dir, extra
        )
    }

    #[test]
    fn regex_mode_valid_regex_parses() {
        let dir = tempdir().unwrap();
        let toml = make_toml(
            dir.path().to_str().unwrap(),
            r#"completion_signal_mode = "regex""#,
        );
        let wf = Workflow::from_str(&toml).unwrap();
        assert!(matches!(
            wf.completion_signal_mode,
            CompletionSignalMode::Regex(_)
        ));
    }

    #[test]
    fn regex_mode_invalid_regex_errors() {
        let dir = tempdir().unwrap();
        let toml = format!(
            r#"
[workflow]
completion_signal = "["
context_dir = "{}"
max_cycles = 3
completion_signal_mode = "regex"

[[phases]]
name = "builder"
prompt_text = "Do work."
"#,
            dir.path().to_str().unwrap()
        );
        let err = Workflow::from_str(&toml).unwrap_err();
        assert!(matches!(err, WorkflowError::InvalidSignalRegex(_)));
    }

    #[test]
    fn bogus_mode_errors() {
        let dir = tempdir().unwrap();
        let toml = make_toml(
            dir.path().to_str().unwrap(),
            r#"completion_signal_mode = "bogus""#,
        );
        let err = Workflow::from_str(&toml).unwrap_err();
        assert!(matches!(err, WorkflowError::InvalidCompletionSignalMode(_)));
    }

    #[test]
    fn unknown_completion_signal_phase_errors() {
        let dir = tempdir().unwrap();
        let toml = make_toml(
            dir.path().to_str().unwrap(),
            r#"completion_signal_phases = ["nonexistent"]"#,
        );
        let err = Workflow::from_str(&toml).unwrap_err();
        assert!(
            matches!(err, WorkflowError::UnknownCompletionSignalPhase(ref name) if name == "nonexistent")
        );
    }

    #[test]
    fn known_completion_signal_phase_parses() {
        let dir = tempdir().unwrap();
        let toml = make_toml(
            dir.path().to_str().unwrap(),
            r#"completion_signal_phases = ["builder"]"#,
        );
        let wf = Workflow::from_str(&toml).unwrap();
        assert_eq!(wf.completion_signal_phases, vec!["builder"]);
    }

    #[test]
    fn produces_required_without_manifest_errors() {
        let dir = tempdir().unwrap();
        let toml = format!(
            r#"
[workflow]
completion_signal = "DONE"
context_dir = "{}"
max_cycles = 3

[[phases]]
name = "builder"
prompt_text = "Do work."
produces_required = true
"#,
            dir.path().to_str().unwrap()
        );
        let err = Workflow::from_str(&toml).unwrap_err();
        assert!(
            matches!(err, WorkflowError::ProducesRequiredWithoutManifest(ref name) if name == "builder")
        );
    }

    #[test]
    fn output_dir_with_dotdot_is_rejected() {
        let dir = tempdir().unwrap();
        let toml = make_toml(dir.path().to_str().unwrap(), r#"output_dir = "../outside""#);
        let err = Workflow::from_str(&toml).unwrap_err();
        assert!(matches!(err, WorkflowError::OutputDirContainsParentDir));
    }

    #[test]
    fn output_dir_with_dotdot_in_middle_is_rejected() {
        let dir = tempdir().unwrap();
        let toml = make_toml(
            dir.path().to_str().unwrap(),
            r#"output_dir = "/tmp/foo/../bar""#,
        );
        let err = Workflow::from_str(&toml).unwrap_err();
        assert!(matches!(err, WorkflowError::OutputDirContainsParentDir));
    }

    #[test]
    fn output_dir_with_single_dot_is_allowed() {
        let dir = tempdir().unwrap();
        let toml = make_toml(
            dir.path().to_str().unwrap(),
            r#"output_dir = "./valid/path""#,
        );
        let result = Workflow::from_str(&toml);
        assert!(result.is_ok(), "single-dot paths must be accepted");
    }

    #[test]
    fn phase_contract_fields_default_when_absent() {
        let dir = tempdir().unwrap();
        let toml = make_toml(dir.path().to_str().unwrap(), "");
        let wf = Workflow::from_str(&toml).unwrap();
        let phase = &wf.phases[0];
        assert!(phase.consumes.is_empty());
        assert!(phase.produces.is_empty());
        assert!(!phase.produces_required);
    }

    #[test]
    fn detect_model_name_returns_some_when_flag_present() {
        let dir = tempdir().unwrap();
        let toml = format!(
            r#"
[workflow]
completion_signal = "DONE"
context_dir = "{}"
max_cycles = 3

[[phases]]
name = "builder"
prompt_text = "Do work."

[executor]
binary = "claude"
args = ["--model", "claude-sonnet-4-5", "--output-format", "stream-json"]
"#,
            dir.path().to_str().unwrap()
        );
        let wf = Workflow::from_str(&toml).unwrap();
        assert_eq!(
            wf.detect_model_name(),
            Some("claude-sonnet-4-5".to_string())
        );
    }

    #[test]
    fn detect_model_name_returns_none_when_no_flag() {
        let dir = tempdir().unwrap();
        let toml = format!(
            r#"
[workflow]
completion_signal = "DONE"
context_dir = "{}"
max_cycles = 3

[[phases]]
name = "builder"
prompt_text = "Do work."

[executor]
binary = "claude"
args = ["--output-format", "stream-json"]
"#,
            dir.path().to_str().unwrap()
        );
        let wf = Workflow::from_str(&toml).unwrap();
        assert_eq!(wf.detect_model_name(), None);
    }

    #[test]
    fn detect_model_name_returns_none_when_no_executor() {
        let dir = tempdir().unwrap();
        let toml = make_toml(dir.path().to_str().unwrap(), "");
        let wf = Workflow::from_str(&toml).unwrap();
        assert_eq!(wf.detect_model_name(), None);
    }

    #[test]
    fn global_budget_cap_nan_is_rejected() {
        let dir = tempdir().unwrap();
        let toml = make_toml(dir.path().to_str().unwrap(), "budget_cap_usd = nan");
        let err = Workflow::from_str(&toml).unwrap_err();
        assert!(matches!(err, WorkflowError::InvalidBudgetCap));
    }

    #[test]
    fn global_budget_cap_inf_is_rejected() {
        let dir = tempdir().unwrap();
        let toml = make_toml(dir.path().to_str().unwrap(), "budget_cap_usd = inf");
        let err = Workflow::from_str(&toml).unwrap_err();
        assert!(matches!(err, WorkflowError::InvalidBudgetCap));
    }

    #[test]
    fn global_budget_cap_positive_finite_is_accepted() {
        let dir = tempdir().unwrap();
        let toml = make_toml(dir.path().to_str().unwrap(), "budget_cap_usd = 10.0");
        let wf = Workflow::from_str(&toml).unwrap();
        assert_eq!(wf.budget_cap_usd, Some(10.0));
    }

    #[test]
    fn phase_budget_cap_nan_is_rejected() {
        let dir = tempdir().unwrap();
        let toml = format!(
            r#"
[workflow]
completion_signal = "DONE"
context_dir = "{}"
max_cycles = 3

[[phases]]
name = "builder"
prompt_text = "Do work."
budget_cap_usd = nan
"#,
            dir.path().to_str().unwrap()
        );
        let err = Workflow::from_str(&toml).unwrap_err();
        assert!(matches!(err, WorkflowError::InvalidBudgetCap));
    }

    #[test]
    fn delay_between_runs_integer_parses_as_seconds() {
        let dir = tempdir().unwrap();
        let toml = make_toml(dir.path().to_str().unwrap(), "delay_between_runs = 30");
        let wf = Workflow::from_str(&toml).unwrap();
        assert_eq!(wf.delay_between_runs, 30);
    }

    #[test]
    fn delay_between_runs_string_seconds() {
        let dir = tempdir().unwrap();
        let toml = make_toml(
            dir.path().to_str().unwrap(),
            r#"delay_between_runs = "30s""#,
        );
        let wf = Workflow::from_str(&toml).unwrap();
        assert_eq!(wf.delay_between_runs, 30);
    }

    #[test]
    fn delay_between_runs_string_minutes() {
        let dir = tempdir().unwrap();
        let toml = make_toml(dir.path().to_str().unwrap(), r#"delay_between_runs = "5m""#);
        let wf = Workflow::from_str(&toml).unwrap();
        assert_eq!(wf.delay_between_runs, 300);
    }

    #[test]
    fn delay_between_runs_string_combined() {
        let dir = tempdir().unwrap();
        let toml = make_toml(
            dir.path().to_str().unwrap(),
            r#"delay_between_runs = "1h30m""#,
        );
        let wf = Workflow::from_str(&toml).unwrap();
        assert_eq!(wf.delay_between_runs, 5400);
    }

    #[test]
    fn delay_between_runs_default_is_zero() {
        let dir = tempdir().unwrap();
        let toml = make_toml(dir.path().to_str().unwrap(), "");
        let wf = Workflow::from_str(&toml).unwrap();
        assert_eq!(wf.delay_between_runs, 0);
    }

    #[test]
    fn delay_between_runs_invalid_string_is_error() {
        let dir = tempdir().unwrap();
        let toml = make_toml(
            dir.path().to_str().unwrap(),
            r#"delay_between_runs = "5 minutes""#,
        );
        let err = Workflow::from_str(&toml).unwrap_err();
        assert!(matches!(err, WorkflowError::InvalidDuration { .. }));
    }

    #[test]
    fn delay_between_cycles_string_hours() {
        let dir = tempdir().unwrap();
        let toml = make_toml(
            dir.path().to_str().unwrap(),
            r#"delay_between_cycles = "1h""#,
        );
        let wf = Workflow::from_str(&toml).unwrap();
        assert_eq!(wf.delay_between_cycles, 3600);
    }
}
