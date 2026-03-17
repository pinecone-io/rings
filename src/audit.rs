use anyhow::{Context, Result};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
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
    #[serde(default)]
    pub files_added: u32,
    #[serde(default)]
    pub files_modified: u32,
    #[serde(default)]
    pub files_deleted: u32,
    #[serde(default)]
    pub files_changed: u32,
    #[serde(default)]
    pub event: Option<String>,
}

/// Stream cost entries from a costs.jsonl file without loading the entire file into memory.
/// Returns an iterator that yields Result<CostEntry> for each line.
pub fn stream_cost_entries(path: &Path) -> Result<Box<dyn Iterator<Item = Result<CostEntry>>>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open costs.jsonl: {}", path.display()))?;
    let reader = BufReader::new(file);
    let lines = reader.lines();

    let iter = lines.map(|line_result| {
        let line = line_result.with_context(|| "Failed to read line from costs.jsonl")?;
        serde_json::from_str::<CostEntry>(line.trim())
            .with_context(|| format!("Failed to parse cost entry: {}", line))
    });

    Ok(Box::new(iter))
}

/// Recover the maximum run number from costs.jsonl.
/// Scans all entries, skipping malformed lines, and returns the highest run number.
/// Returns Ok(Some(n)) if at least one valid entry exists, Ok(None) if file is absent or empty.
pub fn recover_last_run_from_costs(path: &Path) -> Result<Option<u32>> {
    match stream_cost_entries(path) {
        Ok(entries) => {
            let max_run = entries
                .filter_map(|entry_result| entry_result.ok())
                .map(|entry| entry.run)
                .max();
            Ok(max_run)
        }
        Err(_) => {
            // File doesn't exist or can't be opened; treat as empty
            Ok(None)
        }
    }
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

#[derive(Debug, Serialize)]
pub struct AdvisoryWarningEvent {
    pub event: String,
    pub run_id: String,
    pub phase: String,
    pub warning_type: String, // "unknown_variable" | "cost_spike" etc.
    pub message: String,
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
/// When `retry_count` is None, writes to `{run_number:03}.log`.
/// When `retry_count` is Some(n), writes to `{run_number:03}-retry-{n}.log`.
pub fn write_run_log(
    runs_dir: &Path,
    run_number: u32,
    output: &str,
    retry_count: Option<u32>,
) -> Result<()> {
    std::fs::create_dir_all(runs_dir)
        .with_context(|| format!("Failed to create runs dir: {}", runs_dir.display()))?;
    let filename = match retry_count {
        None => format!("{run_number:03}.log"),
        Some(n) => format!("{run_number:03}-retry-{n}.log"),
    };
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
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
        };
        append_cost_entry(&path, &entry).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["run"], 1);
        assert_eq!(parsed["cost_confidence"], "full");
        assert_eq!(parsed["iteration"], 2);
    }
}
