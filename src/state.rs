use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

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
}

impl StateFile {
    pub fn read(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read state file: {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse state file: {}", path.display()))
    }

    /// Atomic write: write to a temp file then rename into place.
    pub fn write_atomic(&self, path: &Path) -> Result<()> {
        let tmp_path = path.with_extension("tmp");
        let content = serde_json::to_string_pretty(self).context("Failed to serialize state")?;
        std::fs::write(&tmp_path, &content)
            .with_context(|| format!("Failed to write temp state file: {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, path)
            .with_context(|| format!("Failed to rename state file into place: {}", path.display()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMeta {
    pub run_id: String,
    pub workflow_file: String,
    pub started_at: String,
    pub rings_version: String,
    pub status: String, // "running" | "completed" | "canceled" | "failed"
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
        let tmp_path = path.with_extension("tmp");
        let content = toml::to_string_pretty(self).context("Failed to serialize run.toml")?;
        std::fs::write(&tmp_path, &content)
            .with_context(|| format!("Failed to write temp run.toml: {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, path)
            .with_context(|| format!("Failed to rename run.toml into place: {}", path.display()))
    }

    pub fn update_status(&mut self, path: &Path, status: &str) -> Result<()> {
        self.status = status.to_string();
        self.write(path)
    }
}
