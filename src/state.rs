use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};

static TEMP_FILE_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Run status enum for serialization/deserialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Running,
    Completed,
    Canceled,
    Failed,
    #[serde(rename = "incomplete")]
    Incomplete,
    #[serde(rename = "stopped")]
    Stopped,
}

impl fmt::Display for RunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunStatus::Running => write!(f, "running"),
            RunStatus::Completed => write!(f, "completed"),
            RunStatus::Canceled => write!(f, "canceled"),
            RunStatus::Failed => write!(f, "failed"),
            RunStatus::Incomplete => write!(f, "incomplete"),
            RunStatus::Stopped => write!(f, "stopped"),
        }
    }
}

impl FromStr for RunStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "running" => Ok(RunStatus::Running),
            "completed" => Ok(RunStatus::Completed),
            "canceled" => Ok(RunStatus::Canceled),
            "failed" => Ok(RunStatus::Failed),
            "incomplete" => Ok(RunStatus::Incomplete),
            "stopped" => Ok(RunStatus::Stopped),
            _ => Err(anyhow::anyhow!(
                "invalid run status: {}; expected one of: running, completed, canceled, failed, incomplete, stopped",
                s
            )),
        }
    }
}

/// Error classification for executor failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailureReason {
    Quota,
    Auth,
    Timeout,
    Unknown,
}

/// Ancestry information for runs in a chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AncestryInfo {
    #[serde(default)]
    pub parent_run_id: Option<String>,
    #[serde(default)]
    pub continuation_of: Option<String>,
    #[serde(default)]
    pub ancestry_depth: u32,
}

/// Control-flow type for state loading with recovery fallback.
/// Do NOT derive Serialize/Deserialize.
#[derive(Debug)]
pub enum StateLoadResult {
    Ok(StateFile),
    Recovered {
        state: StateFile,
        warning: String,
    },
    Unrecoverable {
        state_path: PathBuf,
        costs_path: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateFile {
    pub schema_version: u32,
    pub run_id: String,
    pub workflow_file: String,
    pub last_completed_run: u32,
    pub last_completed_cycle: u32,
    pub last_completed_phase_index: usize,
    pub last_completed_iteration: u32,
    pub total_runs_completed: u32,
    pub cumulative_cost_usd: f64,
    #[serde(default)]
    pub claude_resume_commands: Vec<String>,
    pub canceled_at: Option<String>,
    #[serde(default)]
    pub failure_reason: Option<FailureReason>,
    #[serde(default)]
    pub ancestry: Option<AncestryInfo>,
}

impl StateFile {
    pub fn read(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read state file: {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse state file: {}", path.display()))
    }

    /// Load state.json; on failure, attempt recovery from costs.jsonl.
    /// Returns StateLoadResult based on success, recovery, or unrecoverability.
    pub fn load_or_recover(state_path: &Path, costs_path: &Path) -> StateLoadResult {
        // Try to read state.json
        match Self::read(state_path) {
            Ok(state) => StateLoadResult::Ok(state),
            Err(_read_err) => {
                // State file read failed; attempt recovery from costs.jsonl
                match crate::audit::recover_last_run_from_costs(costs_path) {
                    Ok(Some(last_run)) => {
                        // Successfully recovered last_completed_run from costs
                        let state = StateFile {
                            schema_version: 1,
                            run_id: String::new(), // Will be populated by caller
                            workflow_file: String::new(),
                            last_completed_run: last_run,
                            last_completed_cycle: 0,
                            last_completed_phase_index: 0,
                            last_completed_iteration: 0,
                            total_runs_completed: last_run,
                            cumulative_cost_usd: 0.0,
                            claude_resume_commands: vec![],
                            canceled_at: None,
                            failure_reason: None,
                            ancestry: None,
                        };
                        let warning = format!(
                            "Warning: state.json is corrupt but costs.jsonl has {} completed run(s). \
                             Resuming from run {}.",
                            last_run, last_run
                        );
                        StateLoadResult::Recovered { state, warning }
                    }
                    Ok(None) => {
                        // costs.jsonl is absent or empty
                        StateLoadResult::Unrecoverable {
                            state_path: state_path.to_path_buf(),
                            costs_path: costs_path.to_path_buf(),
                        }
                    }
                    Err(_recover_err) => {
                        // Error reading costs.jsonl or recovering from it
                        StateLoadResult::Unrecoverable {
                            state_path: state_path.to_path_buf(),
                            costs_path: costs_path.to_path_buf(),
                        }
                    }
                }
            }
        }
    }

    /// Atomic write: write to a temp file then rename into place.
    pub fn write_atomic(&self, path: &Path) -> Result<()> {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_path = path.with_extension(format!("{}.{}.tmp", std::process::id(), counter));
        let content = serde_json::to_string_pretty(self).context("Failed to serialize state")?;
        std::fs::write(&tmp_path, &content)
            .with_context(|| format!("Failed to write temp state file: {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, path)
            .inspect_err(|_| {
                // Delete temp file on rename failure to avoid orphans
                let _ = std::fs::remove_file(&tmp_path);
            })
            .with_context(|| format!("Failed to rename state file into place: {}", path.display()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMeta {
    pub run_id: String,
    pub workflow_file: String,
    pub started_at: String,
    pub rings_version: String,
    pub status: RunStatus,
    #[serde(default)]
    pub phase_fingerprint: Option<Vec<String>>,
    #[serde(default)]
    pub parent_run_id: Option<String>,
    #[serde(default)]
    pub continuation_of: Option<String>,
    #[serde(default)]
    pub ancestry_depth: u32,
    #[serde(default)]
    pub context_dir: Option<String>,
}

impl RunMeta {
    pub fn read(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read run.toml: {}", path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse run.toml: {}", path.display()))
    }

    /// Atomic write: write to a temp file then rename into place.
    pub fn write(&self, path: &Path) -> Result<()> {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_path = path.with_extension(format!("{}.{}.tmp", std::process::id(), counter));
        let content = toml::to_string_pretty(self).context("Failed to serialize run.toml")?;
        std::fs::write(&tmp_path, &content)
            .with_context(|| format!("Failed to write temp run.toml: {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, path)
            .inspect_err(|_| {
                // Delete temp file on rename failure to avoid orphans
                let _ = std::fs::remove_file(&tmp_path);
            })
            .with_context(|| format!("Failed to rename run.toml into place: {}", path.display()))
    }

    pub fn update_status(&mut self, path: &Path, status: RunStatus) -> Result<()> {
        self.status = status;
        self.write(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_meta(context_dir: Option<String>) -> RunMeta {
        RunMeta {
            run_id: "test-run".to_string(),
            workflow_file: "/tmp/workflow.rings.toml".to_string(),
            started_at: "2026-01-01T00:00:00Z".to_string(),
            rings_version: "0.1.0".to_string(),
            status: RunStatus::Completed,
            phase_fingerprint: None,
            parent_run_id: None,
            continuation_of: None,
            ancestry_depth: 0,
            context_dir,
        }
    }

    #[test]
    fn run_meta_serializes_context_dir_when_set() {
        let meta = minimal_meta(Some("/home/user/project".to_string()));
        let serialized = toml::to_string_pretty(&meta).unwrap();
        assert!(
            serialized.contains("context_dir"),
            "expected context_dir in serialized output: {serialized}"
        );
        assert!(
            serialized.contains("/home/user/project"),
            "expected context_dir value in serialized output: {serialized}"
        );
    }

    #[test]
    fn run_meta_deserializes_without_context_dir() {
        let toml_str = r#"
run_id = "test-run"
workflow_file = "/tmp/workflow.rings.toml"
started_at = "2026-01-01T00:00:00Z"
rings_version = "0.1.0"
status = "completed"
ancestry_depth = 0
"#;
        let meta: RunMeta = toml::from_str(toml_str).unwrap();
        assert_eq!(meta.context_dir, None);
    }

    #[test]
    fn run_meta_round_trips_context_dir() {
        let meta = minimal_meta(Some("/absolute/path/to/project".to_string()));
        let serialized = toml::to_string_pretty(&meta).unwrap();
        let deserialized: RunMeta = toml::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized.context_dir,
            Some("/absolute/path/to/project".to_string())
        );
    }
}
