use anyhow::{Context, Result};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

lazy_static! {
    // Pattern to extract resume commands from executor output
    static ref RE_RESUME: regex::Regex = regex::Regex::new(
        r"claude resume [a-zA-Z0-9_-]+"
    ).unwrap(); // Safe: compile-time constant regex
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CostEntry {
    pub run: u32,
    pub cycle: u32,
    pub phase: String,
    pub iteration: u32,
    pub cost_usd: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_confidence: String, // "full" | "partial" | "low" | "none"
}

/// Append one line to costs.jsonl (creates file if absent).
pub fn append_cost_entry(costs_path: &Path, entry: &CostEntry) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(costs_path)
        .with_context(|| format!("Failed to open costs.jsonl: {}", costs_path.display()))?;
    let line = serde_json::to_string(entry).context("Failed to serialize cost entry")?;
    writeln!(file, "{line}")
        .with_context(|| format!("Failed to write to costs.jsonl: {}", costs_path.display()))
}

#[derive(Debug, Serialize)]
pub struct BudgetWarningEvent {
    pub event: String,
    pub run_id: String,
    pub cost_usd: f64,
    pub budget_cap_usd: f64,
    pub pct: u8,
    pub scope: String, // "global" or "phase:<name>"
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct BudgetCapEvent {
    pub event: String,
    pub run_id: String,
    pub cost_usd: f64,
    pub budget_cap_usd: f64,
    pub scope: String, // "global" or "phase:<name>"
    pub runs_completed: u32,
    pub timestamp: String,
}

/// Append one line to events.jsonl (creates file if absent).
pub fn append_event(events_path: &Path, event: &serde_json::Value) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_path)
        .with_context(|| format!("Failed to open events.jsonl: {}", events_path.display()))?;
    let line = serde_json::to_string(event).context("Failed to serialize event")?;
    writeln!(file, "{line}")
        .with_context(|| format!("Failed to write to events.jsonl: {}", events_path.display()))
}

/// Write the full raw output of one run to its log file (e.g. runs/001.log).
pub fn write_run_log(runs_dir: &Path, run_number: u32, output: &str) -> Result<()> {
    std::fs::create_dir_all(runs_dir)
        .with_context(|| format!("Failed to create runs dir: {}", runs_dir.display()))?;
    let filename = format!("{run_number:03}.log");
    let path = runs_dir.join(filename);
    std::fs::write(&path, output)
        .with_context(|| format!("Failed to write run log: {}", path.display()))
}

/// Extract `claude resume <id>` commands from executor output.
pub fn extract_resume_commands(output: &str) -> Vec<String> {
    RE_RESUME
        .find_iter(output)
        .map(|m| m.as_str().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn extracts_single_resume_command() {
        let output = "some output\nclaude resume abc-123-def\nmore output";
        let cmds = extract_resume_commands(output);
        assert_eq!(cmds, vec!["claude resume abc-123-def"]);
    }

    #[test]
    fn extracts_multiple_resume_commands() {
        let output = "claude resume aaa-111\nclaude resume bbb-222";
        let cmds = extract_resume_commands(output);
        assert_eq!(cmds.len(), 2);
        assert!(cmds[0].contains("aaa-111"));
        assert!(cmds[1].contains("bbb-222"));
    }

    #[test]
    fn returns_empty_when_no_resume_commands() {
        assert!(extract_resume_commands("no resume here").is_empty());
    }

    #[test]
    fn cost_entry_roundtrips_to_jsonl() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("costs.jsonl");
        let entry = CostEntry {
            run: 1,
            cycle: 1,
            phase: "builder".to_string(),
            iteration: 2,
            cost_usd: Some(0.05),
            input_tokens: Some(1000),
            output_tokens: Some(200),
            cost_confidence: "full".to_string(),
        };
        append_cost_entry(&path, &entry).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["run"], 1);
        assert_eq!(parsed["cost_confidence"], "full");
        assert_eq!(parsed["iteration"], 2);
    }
}
