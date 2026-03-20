use anyhow::{Context, Result};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Seek, Write};
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
    #[serde(default)]
    pub produces_violations: Vec<String>,
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
///
/// Before writing, recovers from partial writes: if the file does not end with `\n`
/// (e.g., due to a crash mid-write), the incomplete last line is truncated so only
/// fully-written entries remain. The new entry is then written as a single `write_all`
/// call (JSON + newline) and flushed to disk with `sync_data`.
pub fn append_cost_entry(costs_path: &Path, entry: &CostEntry) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(costs_path)
        .with_context(|| format!("Failed to open costs.jsonl: {}", costs_path.display()))?;

    // Recovery: if the file has a partial last line (no trailing '\n'), truncate it.
    let len = file
        .metadata()
        .with_context(|| format!("Failed to stat costs.jsonl: {}", costs_path.display()))?
        .len();
    if len > 0 {
        file.seek(std::io::SeekFrom::End(-1))
            .with_context(|| "Failed to seek costs.jsonl")?;
        let mut last_byte = [0u8; 1];
        file.read_exact(&mut last_byte)
            .with_context(|| "Failed to read last byte of costs.jsonl")?;
        if last_byte[0] != b'\n' {
            // Read the tail of the file (up to 8 KiB) to locate the last clean newline.
            let read_start = len.saturating_sub(8192);
            file.seek(std::io::SeekFrom::Start(read_start))
                .with_context(|| "Failed to seek costs.jsonl for recovery")?;
            let mut tail = Vec::new();
            file.read_to_end(&mut tail)
                .with_context(|| "Failed to read tail of costs.jsonl for recovery")?;
            let truncate_to = tail
                .iter()
                .rposition(|&b| b == b'\n')
                .map(|pos| read_start + pos as u64 + 1)
                .unwrap_or(0);
            file.set_len(truncate_to)
                .with_context(|| "Failed to truncate costs.jsonl during recovery")?;
        }
    }

    // Seek to end for append.
    file.seek(std::io::SeekFrom::End(0))
        .with_context(|| "Failed to seek to end of costs.jsonl")?;

    // Write the full line (JSON + newline) atomically in one write_all call.
    let line = serde_json::to_string(entry).context("Failed to serialize cost entry")?;
    let line_with_newline = format!("{line}\n");
    file.write_all(line_with_newline.as_bytes())
        .with_context(|| format!("Failed to write to costs.jsonl: {}", costs_path.display()))?;

    // Flush to disk before returning.
    file.sync_data()
        .with_context(|| format!("Failed to sync costs.jsonl: {}", costs_path.display()))
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
            produces_violations: vec![],
        };
        append_cost_entry(&path, &entry).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["run"], 1);
        assert_eq!(parsed["cost_confidence"], "full");
        assert_eq!(parsed["iteration"], 2);
    }

    #[test]
    fn cost_entry_old_jsonl_without_produces_violations_defaults_to_empty() {
        // Old JSONL line without produces_violations field — must deserialize to empty vec
        let json = r#"{"run":1,"cycle":1,"phase":"builder","iteration":1,"cost_usd":0.05,"input_tokens":100,"output_tokens":20,"cost_confidence":"full","files_added":0,"files_modified":0,"files_deleted":0,"files_changed":0}"#;
        let entry: CostEntry = serde_json::from_str(json).unwrap();
        assert!(entry.produces_violations.is_empty());
    }

    fn make_entry(run: u32) -> CostEntry {
        CostEntry {
            run,
            cycle: 1,
            phase: "builder".to_string(),
            iteration: 1,
            cost_usd: Some(0.01),
            input_tokens: Some(100),
            output_tokens: Some(20),
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        }
    }

    #[test]
    fn append_cost_entry_recovers_truncated_last_line() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("costs.jsonl");

        // Write one valid entry followed by a partial line (no trailing newline).
        let valid_entry = make_entry(1);
        let valid_json = serde_json::to_string(&valid_entry).unwrap();
        let partial = format!("{valid_json}\n{{\"run\":2,\"partial");
        std::fs::write(&path, partial).unwrap();

        // Appending a new entry should truncate the partial line and write cleanly.
        let new_entry = make_entry(3);
        append_cost_entry(&path, &new_entry).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "Should have 2 lines after recovery");

        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["run"], 1);

        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["run"], 3);
    }

    #[test]
    fn append_cost_entry_handles_file_with_no_newline_at_all() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("costs.jsonl");

        // Write only a partial line with no newline anywhere.
        std::fs::write(&path, b"{\"run\":1,\"partial").unwrap();

        let entry = make_entry(2);
        append_cost_entry(&path, &entry).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed["run"], 2);
    }

    #[test]
    fn append_cost_entry_clean_file_unchanged() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("costs.jsonl");

        // Write two valid entries.
        append_cost_entry(&path, &make_entry(1)).unwrap();
        append_cost_entry(&path, &make_entry(2)).unwrap();
        append_cost_entry(&path, &make_entry(3)).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        for (i, line) in lines.iter().enumerate() {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(parsed["run"], (i + 1) as u64);
        }
    }

    #[test]
    fn truncated_last_line_skipped_by_stream_cost_entries() {
        // Verify that stream_cost_entries gracefully skips a malformed last line
        // (e.g., no truncation has happened yet and someone reads the file directly).
        let dir = tempdir().unwrap();
        let path = dir.path().join("costs.jsonl");

        let valid_json = serde_json::to_string(&make_entry(1)).unwrap();
        let content = format!("{valid_json}\n{{\"run\":2,\"partial");
        std::fs::write(&path, content).unwrap();

        let max_run = recover_last_run_from_costs(&path).unwrap();
        assert_eq!(
            max_run,
            Some(1),
            "Partial line should be skipped during recovery"
        );
    }

    #[test]
    fn cost_entry_serialize_always_includes_produces_violations() {
        let entry = CostEntry {
            run: 1,
            cycle: 1,
            phase: "builder".to_string(),
            iteration: 1,
            cost_usd: Some(0.05),
            input_tokens: Some(100),
            output_tokens: Some(20),
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Field must always be present, even when empty
        assert!(parsed.get("produces_violations").is_some());
        assert_eq!(parsed["produces_violations"], serde_json::json!([]));
    }
}
