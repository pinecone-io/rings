use anyhow::{Context, Result};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Seek, Write};
use std::path::Path;

/// Parameters for generating a human-readable summary.md at the end of a run.
pub struct SummaryInfo<'a> {
    pub run_id: &'a str,
    pub workflow_file: &'a str,
    /// Human-readable status string: "completed", "canceled", "max_cycles", "budget_cap",
    /// "executor_error", etc.
    pub status: &'a str,
    pub started_at: &'a str,
    pub context_dir: Option<&'a str>,
    pub output_dir: &'a Path,
    pub completed_cycles: u32,
    pub total_runs: u32,
    pub total_cost_usd: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    /// Per-phase cost and run count, in workflow declaration order.
    pub phase_costs: &'a [(String, f64, u32)],
    pub total_elapsed_secs: u64,
    /// If the run completed via signal detection: (cycle, run, phase_name).
    pub completion_info: Option<(u32, u32, String)>,
    /// claude resume commands captured from executor output (for canceled runs).
    pub claude_resume_commands: &'a [String],
}

/// Format total elapsed seconds as a human-readable duration string (e.g., "14m 32s").
fn format_elapsed(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    let mut parts = Vec::new();
    if h > 0 {
        parts.push(format!("{h}h"));
    }
    if m > 0 {
        parts.push(format!("{m}m"));
    }
    if h == 0 {
        // Show seconds unless duration >= 1 hour
        parts.push(format!("{s}s"));
    }
    if parts.is_empty() {
        return "0s".to_string();
    }
    parts.join(" ")
}

/// Extract the basename of a workflow file path (without leading directories).
fn workflow_basename(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

/// Generate a human-readable summary.md in `run_dir`.
///
/// This file is written on all run exit paths and provides a persistent record
/// of the run's status, cost, and output location.
pub fn generate_summary_md(run_dir: &Path, info: &SummaryInfo<'_>) -> Result<()> {
    let mut md = String::new();

    // Title
    md.push_str(&format!("# rings Run Summary: {}\n\n", info.run_id));

    // Header fields
    let status_display = match info.status {
        "completed" => "Completed (signal detected)".to_string(),
        "canceled" => "Canceled".to_string(),
        "max_cycles" => "Stopped (max cycles reached)".to_string(),
        "budget_cap" => "Stopped (budget cap reached)".to_string(),
        "executor_error" => "Failed (executor error)".to_string(),
        other => {
            let mut s = other[..1].to_uppercase();
            s.push_str(&other[1..]);
            s
        }
    };
    md.push_str(&format!("**Status:** {}\n", status_display));
    md.push_str(&format!(
        "**Workflow:** {}\n",
        workflow_basename(info.workflow_file)
    ));
    md.push_str(&format!("**Started:** {}\n", info.started_at));
    md.push_str(&format!(
        "**Duration:** {}\n",
        format_elapsed(info.total_elapsed_secs)
    ));
    if let Some(ctx) = info.context_dir {
        md.push_str(&format!("**Context directory:** {}\n", ctx));
    }
    md.push_str(&format!(
        "**Output directory:** {}\n",
        info.output_dir.display()
    ));
    md.push('\n');

    // Execution summary
    md.push_str("## Execution\n\n");
    md.push_str(&format!("Cycles completed: {}\n", info.completed_cycles));
    md.push_str(&format!("Total runs: {}\n", info.total_runs));
    if let Some((cycle, run, ref phase)) = info.completion_info {
        md.push_str(&format!(
            "\nCompleted on cycle {cycle}, run {run}, phase {phase}.\n"
        ));
    }
    md.push('\n');

    // Cost breakdown table
    md.push_str("## Cost\n\n");
    if !info.phase_costs.is_empty() {
        md.push_str("| Phase | Runs | Cost |\n");
        md.push_str("|-------|------|------|\n");
        for (phase, cost, runs) in info.phase_costs {
            md.push_str(&format!("| {} | {} | ${:.3} |\n", phase, runs, cost));
        }
        md.push_str(&format!(
            "| **Total** | **{}** | **${:.3}** |\n",
            info.total_runs, info.total_cost_usd
        ));
    } else {
        md.push_str(&format!("Total: ${:.3}\n", info.total_cost_usd));
    }
    md.push('\n');

    // Token totals (if any data was captured)
    if info.total_input_tokens > 0 || info.total_output_tokens > 0 {
        md.push_str(&format!(
            "Tokens: {} input / {} output\n\n",
            info.total_input_tokens, info.total_output_tokens
        ));
    }

    // Resume commands (for canceled runs)
    if !info.claude_resume_commands.is_empty() {
        md.push_str("## Resume\n\n");
        md.push_str("To continue partial work from the last Claude session:\n\n");
        for cmd in info.claude_resume_commands {
            md.push_str(&format!("```\n{cmd}\n```\n\n"));
        }
    }

    // Output location
    md.push_str("## Output Location\n\n");
    md.push_str(&format!("Audit logs: {}\n", info.output_dir.display()));

    let summary_path = run_dir.join("summary.md");
    std::fs::write(&summary_path, &md)
        .with_context(|| format!("Failed to write summary.md: {}", summary_path.display()))
}

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

/// Write gate stdout/stderr to a log file in the runs directory.
/// File name: `{run_number:03}-gate-cycle.log` for cycle gates,
/// or `{run_number:03}-gate-{phase_name}.log` for phase gates.
/// The `scope` parameter is either `"cycle"` or the phase name.
pub fn write_gate_log(
    runs_dir: &Path,
    run_number: u32,
    scope: &str,
    stdout: &str,
    stderr: &str,
) -> Result<()> {
    std::fs::create_dir_all(runs_dir)
        .with_context(|| format!("Failed to create runs dir: {}", runs_dir.display()))?;
    let filename = format!("{run_number:03}-gate-{scope}.log");
    let path = runs_dir.join(filename);
    let content = format!("stdout:\n{stdout}\n\nstderr:\n{stderr}\n");
    std::fs::write(&path, &content)
        .with_context(|| format!("Failed to write gate log: {}", path.display()))
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

    fn make_summary_info<'a>(
        run_dir: &'a std::path::Path,
        phase_costs: &'a [(String, f64, u32)],
        resume_cmds: &'a [String],
        status: &'a str,
        exit_code: i32,
        completion_info: Option<(u32, u32, String)>,
    ) -> SummaryInfo<'a> {
        SummaryInfo {
            run_id: "run_20240315_143022_a1b2c3",
            workflow_file: "/home/user/my-task.rings.toml",
            status,
            started_at: "2024-03-15T14:30:22Z",
            context_dir: Some("/home/user/project"),
            output_dir: run_dir,
            completed_cycles: 2,
            total_runs: 6,
            total_cost_usd: 1.10,
            total_input_tokens: 1234,
            total_output_tokens: 567,
            phase_costs,
            total_elapsed_secs: 872, // 14m 32s
            completion_info,
            claude_resume_commands: resume_cmds,
        }
    }

    #[test]
    fn completed_run_produces_summary_with_correct_status_and_cost() {
        let dir = tempdir().unwrap();
        let phase_costs = vec![
            ("builder".to_string(), 0.89, 5u32),
            ("reviewer".to_string(), 0.21, 1u32),
        ];
        let info = make_summary_info(
            dir.path(),
            &phase_costs,
            &[],
            "completed",
            0,
            Some((2, 6, "builder".to_string())),
        );
        generate_summary_md(dir.path(), &info).unwrap();

        let content = std::fs::read_to_string(dir.path().join("summary.md")).unwrap();
        assert!(
            content.contains("Completed (signal detected)"),
            "Expected 'Completed (signal detected)' in summary"
        );
        assert!(content.contains("1.10"), "Expected total cost in summary");
        assert!(
            content.contains("builder"),
            "Expected phase name in summary"
        );
        assert!(
            content.contains("reviewer"),
            "Expected phase name in summary"
        );
    }

    #[test]
    fn canceled_run_produces_summary_with_resume_command() {
        let dir = tempdir().unwrap();
        let phase_costs = vec![("builder".to_string(), 0.50, 3u32)];
        let resume_cmds = vec!["claude resume abc-123-def".to_string()];
        let info = make_summary_info(
            dir.path(),
            &phase_costs,
            &resume_cmds,
            "canceled",
            130,
            None,
        );
        generate_summary_md(dir.path(), &info).unwrap();

        let content = std::fs::read_to_string(dir.path().join("summary.md")).unwrap();
        assert!(content.contains("Canceled"), "Expected 'Canceled' status");
        assert!(
            content.contains("claude resume abc-123-def"),
            "Expected resume command in summary"
        );
    }

    #[test]
    fn summary_contains_phase_cost_breakdown() {
        let dir = tempdir().unwrap();
        let phase_costs = vec![
            ("builder".to_string(), 0.89, 5u32),
            ("reviewer".to_string(), 0.21, 1u32),
        ];
        let info = make_summary_info(
            dir.path(),
            &phase_costs,
            &[],
            "completed",
            0,
            Some((2, 6, "builder".to_string())),
        );
        generate_summary_md(dir.path(), &info).unwrap();

        let content = std::fs::read_to_string(dir.path().join("summary.md")).unwrap();
        assert!(content.contains("## Cost"), "Expected Cost section");
        assert!(
            content.contains("builder"),
            "Expected builder phase in cost table"
        );
        assert!(
            content.contains("reviewer"),
            "Expected reviewer phase in cost table"
        );
        assert!(
            content.contains("0.89"),
            "Expected builder cost in cost table"
        );
        assert!(
            content.contains("0.21"),
            "Expected reviewer cost in cost table"
        );
    }

    #[test]
    fn summary_md_is_valid_markdown_no_broken_formatting() {
        let dir = tempdir().unwrap();
        let phase_costs = vec![("builder".to_string(), 1.23, 4u32)];
        let info = make_summary_info(dir.path(), &phase_costs, &[], "max_cycles", 1, None);
        generate_summary_md(dir.path(), &info).unwrap();

        let content = std::fs::read_to_string(dir.path().join("summary.md")).unwrap();
        // Must start with a heading
        assert!(
            content.starts_with("# rings Run Summary:"),
            "Must start with heading"
        );
        // Must have section headers
        assert!(
            content.contains("## Execution"),
            "Must have Execution section"
        );
        assert!(content.contains("## Cost"), "Must have Cost section");
        assert!(
            content.contains("## Output Location"),
            "Must have Output Location section"
        );
        // No broken table rows (all rows must have proper pipe delimiters)
        for line in content.lines() {
            if line.starts_with('|') {
                let pipes = line.chars().filter(|&c| c == '|').count();
                assert!(pipes >= 2, "Table row must have at least 2 pipes: {line}");
            }
        }
    }
}
