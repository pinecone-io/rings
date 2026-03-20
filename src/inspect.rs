use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// One run entry within a cycle for the cycles view.
#[derive(Debug, Clone, Serialize)]
pub struct CycleRunEntry {
    pub run: u32,
    pub phase: String,
    pub iteration: u32,
    pub cost_usd: Option<f64>,
    pub files_changed: u32,
    pub signal_detected: bool,
}

/// Summary of one cycle for the cycles view.
#[derive(Debug, Clone, Serialize)]
pub struct CycleSummary {
    pub cycle: u32,
    pub runs: Vec<CycleRunEntry>,
    pub total_cost_usd: f64,
}

/// One row entry for the costs view (used for JSONL output).
#[derive(Debug, Clone, Serialize)]
pub struct CostRowEntry {
    pub run: u32,
    pub cycle: u32,
    pub phase: String,
    pub iteration: u32,
    pub cost_usd: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub confidence: String,
}

/// Render the per-run cost breakdown view.
///
/// Displays a table with columns: Run, Cycle, Phase, Iter, Input Tok, Output Tok, Confidence, Cost.
/// A totals row is shown at the bottom.
/// If `phase_filter` is Some(name), only runs for that phase are shown.
/// In JSONL mode, emits one JSON object per run.
pub fn render_costs(
    cost_entries: &[crate::audit::CostEntry],
    phase_filter: Option<&str>,
    output_format: crate::cli::OutputFormat,
) -> String {
    let filtered: Vec<&crate::audit::CostEntry> = cost_entries
        .iter()
        .filter(|e| phase_filter.is_none_or(|p| e.phase == p))
        .collect();

    if output_format == crate::cli::OutputFormat::Jsonl {
        let mut out = String::new();
        for entry in &filtered {
            let row = CostRowEntry {
                run: entry.run,
                cycle: entry.cycle,
                phase: entry.phase.clone(),
                iteration: entry.iteration,
                cost_usd: entry.cost_usd,
                input_tokens: entry.input_tokens,
                output_tokens: entry.output_tokens,
                confidence: entry.cost_confidence.clone(),
            };
            if let Ok(json) = serde_json::to_string(&row) {
                out.push_str(&json);
                out.push('\n');
            }
        }
        return out;
    }

    // Human-readable table output.
    if filtered.is_empty() {
        return "No cost data found.\n".to_string();
    }

    let sep = "─".repeat(70);
    let mut out = String::new();
    out.push_str("Cost breakdown:\n");
    out.push_str(&format!(
        "  {:>4}  {:>5}  {:<10}  {:>4}  {:>11}  {:>11}  {:>10}  {:<10}\n",
        "Run", "Cycle", "Phase", "Iter", "Input Tok", "Output Tok", "Cost", "Confidence"
    ));
    out.push_str(&format!("  {}\n", sep));

    let mut total_cost: f64 = 0.0;
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;

    for entry in &filtered {
        let cost_str = match entry.cost_usd {
            Some(c) => {
                total_cost += c;
                format!("${:.3}", c)
            }
            None => "—".to_string(),
        };
        let input_str = match entry.input_tokens {
            Some(t) => {
                total_input_tokens += t;
                format_tokens(t)
            }
            None => "—".to_string(),
        };
        let output_str = match entry.output_tokens {
            Some(t) => {
                total_output_tokens += t;
                format_tokens(t)
            }
            None => "—".to_string(),
        };

        out.push_str(&format!(
            "  {:>4}  {:>5}  {:<10}  {:>4}  {:>11}  {:>11}  {:>10}  {:<10}\n",
            entry.run,
            entry.cycle,
            entry.phase,
            entry.iteration,
            input_str,
            output_str,
            cost_str,
            entry.cost_confidence,
        ));
    }

    out.push_str(&format!("  {}\n", sep));
    out.push_str(&format!(
        "  {:>4}  {:>5}  {:<10}  {:>4}  {:>11}  {:>11}  {:>10}\n",
        "Total",
        "",
        "",
        "",
        format_tokens(total_input_tokens),
        format_tokens(total_output_tokens),
        format!("${:.3}", total_cost),
    ));

    out
}

/// Format a token count with comma separators (e.g. 1234 → "1,234").
fn format_tokens(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

/// Render the per-cycle breakdown view.
///
/// Groups cost entries by cycle, then renders each cycle with its runs.
/// If `cycle_filter` is Some(n), only that cycle is shown.
/// If `signal_run` is Some(run_number), that run gets "✓ SIGNAL" marker.
pub fn render_cycles(
    cost_entries: &[crate::audit::CostEntry],
    cycle_filter: Option<u32>,
    signal_run: Option<u32>,
    output_format: crate::cli::OutputFormat,
) -> String {
    // Group entries by cycle using BTreeMap for sorted output.
    let mut cycles: BTreeMap<u32, Vec<&crate::audit::CostEntry>> = BTreeMap::new();
    for entry in cost_entries {
        cycles.entry(entry.cycle).or_default().push(entry);
    }

    if output_format == crate::cli::OutputFormat::Jsonl {
        let mut out = String::new();
        for (cycle_num, entries) in &cycles {
            if let Some(f) = cycle_filter {
                if *cycle_num != f {
                    continue;
                }
            }
            let cycle_summary = build_cycle_summary(*cycle_num, entries, signal_run);
            if let Ok(json) = serde_json::to_string(&cycle_summary) {
                out.push_str(&json);
                out.push('\n');
            }
        }
        return out;
    }

    // Human-readable output
    let mut out = String::new();
    for (cycle_num, entries) in &cycles {
        if let Some(f) = cycle_filter {
            if *cycle_num != f {
                continue;
            }
        }

        let cycle_summary = build_cycle_summary(*cycle_num, entries, signal_run);

        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("Cycle {}:\n", cycle_num));

        // Determine column widths for alignment.
        let max_run_digits = cycle_summary
            .runs
            .iter()
            .map(|r| r.run.to_string().len())
            .max()
            .unwrap_or(1);
        let max_phase_len = cycle_summary
            .runs
            .iter()
            .map(|r| r.phase.len())
            .max()
            .unwrap_or(4);

        for run_entry in &cycle_summary.runs {
            let cost_str = match run_entry.cost_usd {
                Some(c) => format!("${:.3}", c),
                None => "—".to_string(),
            };
            let files_str = match run_entry.files_changed {
                0 => "no files changed".to_string(),
                1 => "1 file changed".to_string(),
                n => format!("{} files changed", n),
            };
            let signal_str = if run_entry.signal_detected {
                "  ✓ SIGNAL"
            } else {
                ""
            };
            out.push_str(&format!(
                "  Run {:>rw$}  {:<pw$}  iter {:>3}   {:>8}   {:<20}{}\n",
                run_entry.run,
                run_entry.phase,
                run_entry.iteration,
                cost_str,
                files_str,
                signal_str,
                rw = max_run_digits,
                pw = max_phase_len,
            ));
        }

        if cycle_summary.total_cost_usd > 0.0 {
            out.push_str(&format!(
                "  Subtotal: ${:.3}\n",
                cycle_summary.total_cost_usd
            ));
        }
    }

    if out.is_empty() {
        "No cycle data found.\n".to_string()
    } else {
        out
    }
}

fn build_cycle_summary(
    cycle_num: u32,
    entries: &[&crate::audit::CostEntry],
    signal_run: Option<u32>,
) -> CycleSummary {
    let mut runs: Vec<CycleRunEntry> = entries
        .iter()
        .map(|e| CycleRunEntry {
            run: e.run,
            phase: e.phase.clone(),
            iteration: e.iteration,
            cost_usd: e.cost_usd,
            files_changed: e.files_changed,
            signal_detected: signal_run == Some(e.run),
        })
        .collect();
    // Sort by run number for consistent display.
    runs.sort_by_key(|r| r.run);

    let total_cost_usd: f64 = runs.iter().filter_map(|r| r.cost_usd).sum();

    CycleSummary {
        cycle: cycle_num,
        runs,
        total_cost_usd,
    }
}

/// Declared data flow for a single phase (sourced from workflow_contracts.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclaredFlow {
    pub phase: String,
    pub consumes: Vec<String>,
    pub produces: Vec<String>,
}

/// The kind of file change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

/// A single file change attributed to a phase/cycle/run.
#[derive(Debug, Clone)]
pub struct ActualFileChange {
    pub path: String,
    pub phase: String,
    pub cycle: u32,
    pub run: u32,
    pub iteration: u32,
    pub change_type: ChangeType,
}

/// One file change event for JSONL output in the files-changed view.
#[derive(Debug, Clone, Serialize)]
pub struct FileChangeEntry {
    pub path: String,
    pub change_type: String,
    pub run: u32,
    pub cycle: u32,
    pub phase: String,
    pub iteration: u32,
}

/// JSONL record for a declared phase flow entry.
#[derive(Debug, Clone, Serialize)]
pub struct DeclaredFlowEntry {
    pub phase: String,
    pub consumes: Vec<String>,
    pub produces: Vec<String>,
}

/// JSONL record for an actual file attribution entry.
#[derive(Debug, Clone, Serialize)]
pub struct ActualFileEntry {
    pub path: String,
    pub phase: String,
    pub cycles: Vec<u32>,
}

/// Render the declared data-flow graph from a list of `DeclaredFlow` entries.
///
/// Supports optional `phase_filter` to restrict to one phase.
/// In JSONL mode, emits one object per phase with `phase`, `consumes`, and `produces` fields.
///
/// Human output format:
/// ```text
/// Declared data flow (from phase contracts):
///   specs/**/*.md  ──→  [builder]  ──→  src/**/*.rs
///                                       tests/**/*.rs
///   src/**/*.rs   ──→  [reviewer] ──→  review-notes.md
///   tests/**/*.rs ──→  [reviewer]
/// ```
pub fn render_data_flow_declared(
    phases: &[DeclaredFlow],
    phase_filter: Option<&str>,
    output_format: crate::cli::OutputFormat,
) -> String {
    let filtered: Vec<&DeclaredFlow> = phases
        .iter()
        .filter(|p| phase_filter.is_none_or(|f| p.phase == f))
        .collect();

    if output_format == crate::cli::OutputFormat::Jsonl {
        let mut out = String::new();
        for phase in &filtered {
            let entry = DeclaredFlowEntry {
                phase: phase.phase.clone(),
                consumes: phase.consumes.clone(),
                produces: phase.produces.clone(),
            };
            if let Ok(json) = serde_json::to_string(&entry) {
                out.push_str(&json);
                out.push('\n');
            }
        }
        return out;
    }

    let mut lines = vec!["Declared data flow (from phase contracts):".to_string()];

    if filtered.is_empty() {
        lines.push("  (no contracts declared)".to_string());
        return lines.join("\n") + "\n";
    }

    // Compute global column widths.
    let left_width = filtered
        .iter()
        .flat_map(|p| p.consumes.iter().map(String::as_str))
        .map(str::len)
        .max()
        .unwrap_or(0);

    for phase in filtered {
        let phase_label = format!("[{}]", phase.phase);
        let n = phase.consumes.len();
        let m = phase.produces.len();
        let rows = n.max(m).max(1);

        for i in 0..rows {
            let left = phase.consumes.get(i).map(String::as_str);
            // Phase label shown on: every row with a consumes item, or row 0 unconditionally.
            let show_phase = left.is_some() || i == 0;
            let right = phase.produces.get(i).map(String::as_str);

            let line: String = match (left, show_phase, right) {
                // consumes[i] ──→ [phase] ──→ produces[i]
                (Some(l), _, Some(r)) => format!(
                    "  {:<lw$}  \u{2500}\u{2500}\u{2192}  {}  \u{2500}\u{2500}\u{2192}  {}",
                    l,
                    phase_label,
                    r,
                    lw = left_width
                ),
                // consumes[i] ──→ [phase]
                (Some(l), _, None) => format!(
                    "  {:<lw$}  \u{2500}\u{2500}\u{2192}  {}",
                    l,
                    phase_label,
                    lw = left_width
                ),
                // Row 0, no consumes at all for this phase, has produces: [phase] ──→ produces[0]
                (None, true, Some(r)) if n == 0 => {
                    format!("  {}  \u{2500}\u{2500}\u{2192}  {}", phase_label, r)
                }
                // Row 0, no consumes at all for this phase, no produces: [phase]
                (None, true, None) => format!("  {}", phase_label),
                // Continuation produces (i >= n, i > 0): only right column
                (None, false, Some(r)) => {
                    let indent = if n > 0 {
                        // Produces column starts after: "  " + left_width + "  ──→  " + phase_label + "  ──→  "
                        2 + left_width + 7 + phase_label.len() + 7
                    } else {
                        // No consumes: "  " + phase_label + "  ──→  "
                        2 + phase_label.len() + 7
                    };
                    format!("{}{}", " ".repeat(indent), r)
                }
                // Should not occur in practice.
                _ => continue,
            };

            lines.push(line);
        }
    }

    lines.join("\n") + "\n"
}

/// Render actual file attribution grouped by (path, phase) with cycle numbers.
///
/// Supports `cycle_filter` and `phase_filter` to restrict the output.
/// In JSONL mode, emits one object per (path, phase) group with `path`, `phase`, and `cycles` fields.
///
/// Human output format (per spec):
/// ```text
/// Actual file attribution (this run):
///   src/main.rs       builder  (cycles 1, 2)
///   src/engine.rs     builder  (cycles 1, 2)
///   review-notes.md   reviewer (cycle 1)
/// ```
pub fn render_data_flow_actual(
    changes: &[ActualFileChange],
    cycle_filter: Option<u32>,
    phase_filter: Option<&str>,
    output_format: crate::cli::OutputFormat,
) -> String {
    let filtered: Vec<&ActualFileChange> = changes
        .iter()
        .filter(|c| {
            cycle_filter.is_none_or(|cy| c.cycle == cy)
                && phase_filter.is_none_or(|ph| c.phase == ph)
        })
        .collect();

    // Group by (path, phase) and collect unique cycle numbers.
    let mut grouped: BTreeMap<(String, String), BTreeSet<u32>> = BTreeMap::new();
    for change in &filtered {
        grouped
            .entry((change.path.clone(), change.phase.clone()))
            .or_default()
            .insert(change.cycle);
    }

    if output_format == crate::cli::OutputFormat::Jsonl {
        let mut out = String::new();
        for ((path, phase), cycles) in &grouped {
            let entry = ActualFileEntry {
                path: path.clone(),
                phase: phase.clone(),
                cycles: cycles.iter().copied().collect(),
            };
            if let Ok(json) = serde_json::to_string(&entry) {
                out.push_str(&json);
                out.push('\n');
            }
        }
        return out;
    }

    let header = "Actual file attribution (this run):";

    if grouped.is_empty() {
        return format!("{}\n  (no file changes recorded)\n", header);
    }

    let path_width = grouped.keys().map(|(p, _)| p.len()).max().unwrap_or(0);
    let phase_width = grouped.keys().map(|(_, ph)| ph.len()).max().unwrap_or(0);

    let mut lines = vec![header.to_string()];

    for ((path, phase), cycles) in &grouped {
        let mut cycles_sorted: Vec<u32> = cycles.iter().copied().collect();
        cycles_sorted.sort();

        let cycles_str = if cycles_sorted.len() == 1 {
            format!("cycle {}", cycles_sorted[0])
        } else {
            let nums: Vec<String> = cycles_sorted.iter().map(|c| c.to_string()).collect();
            format!("cycles {}", nums.join(", "))
        };

        lines.push(format!(
            "  {:<pw$}  {:<phw$}  ({})",
            path,
            phase,
            cycles_str,
            pw = path_width,
            phw = phase_width,
        ));
    }

    lines.join("\n") + "\n"
}

/// Render the files-changed view, showing which runs touched each file.
///
/// Groups changes by file path and shows each run that touched it with
/// tree-style connectors. Supports `--cycle N` and `--phase NAME` filters.
/// If no manifest data is present, prints a helpful message.
/// In JSONL mode, emits one object per file-change event.
pub fn render_files_changed(
    changes: &[ActualFileChange],
    cycle_filter: Option<u32>,
    phase_filter: Option<&str>,
    output_format: crate::cli::OutputFormat,
) -> String {
    // Apply filters.
    let filtered: Vec<&ActualFileChange> = changes
        .iter()
        .filter(|c| {
            cycle_filter.is_none_or(|cy| c.cycle == cy)
                && phase_filter.is_none_or(|ph| c.phase == ph)
        })
        .collect();

    if output_format == crate::cli::OutputFormat::Jsonl {
        let mut out = String::new();
        for change in &filtered {
            let entry = FileChangeEntry {
                path: change.path.clone(),
                change_type: match change.change_type {
                    ChangeType::Added => "created".to_string(),
                    ChangeType::Modified => "modified".to_string(),
                    ChangeType::Deleted => "deleted".to_string(),
                },
                run: change.run,
                cycle: change.cycle,
                phase: change.phase.clone(),
                iteration: change.iteration,
            };
            if let Ok(json) = serde_json::to_string(&entry) {
                out.push_str(&json);
                out.push('\n');
            }
        }
        return out;
    }

    if filtered.is_empty() {
        if changes.is_empty() {
            return "No file change data available. Enable `manifest_enabled = true` in your workflow.\n".to_string();
        }
        return "No file changes match the specified filters.\n".to_string();
    }

    // Group by file path preserving insertion order via BTreeMap (sorted by path).
    let mut by_path: BTreeMap<String, Vec<&ActualFileChange>> = BTreeMap::new();
    for change in &filtered {
        by_path.entry(change.path.clone()).or_default().push(change);
    }

    let mut out = String::new();
    out.push_str("File change history:\n");

    for (path, events) in &by_path {
        out.push_str(&format!("  {}\n", path));
        let n = events.len();
        for (i, event) in events.iter().enumerate() {
            let connector = if i + 1 == n { "└─" } else { "├─" };
            let ct = match event.change_type {
                ChangeType::Added => "created ",
                ChangeType::Modified => "modified",
                ChangeType::Deleted => "deleted ",
            };
            out.push_str(&format!(
                "    {}  {}  run {:>2}  cycle {}  {}  iter {}\n",
                connector, ct, event.run, event.cycle, event.phase, event.iteration,
            ));
        }
        out.push('\n');
    }

    out
}

/// One run's claude output entry for JSONL mode.
#[derive(Debug, Clone, Serialize)]
pub struct ClaudeOutputEntry {
    pub run: u32,
    pub cycle: Option<u32>,
    pub phase: Option<String>,
    pub log: String,
}

/// Render the claude output view.
///
/// Scans the `runs/` subdirectory for log files (named like `001.log`, `001-retry-1.log`, etc.)
/// and prints each with a run header. Supports `--cycle N` and `--phase NAME` filters
/// by cross-referencing with `cost_entries`.
///
/// In JSONL mode, emits one JSON object per log file.
/// Missing log files for filtered runs are reported with a placeholder message.
pub fn render_claude_output(
    run_dir: &Path,
    cost_entries: &[crate::audit::CostEntry],
    cycle_filter: Option<u32>,
    phase_filter: Option<&str>,
    output_format: crate::cli::OutputFormat,
) -> Result<String> {
    let runs_dir = run_dir.join("runs");

    // Build a map from run number -> (cycle, phase) from cost entries.
    let mut run_meta: BTreeMap<u32, (u32, String)> = BTreeMap::new();
    for entry in cost_entries {
        run_meta.insert(entry.run, (entry.cycle, entry.phase.clone()));
    }

    // Determine which run numbers are allowed by the active filters.
    let filter_runs: Option<BTreeSet<u32>> = if cycle_filter.is_some() || phase_filter.is_some() {
        let matching: BTreeSet<u32> = run_meta
            .iter()
            .filter(|(_, (cycle, phase))| {
                cycle_filter.is_none_or(|c| *cycle == c) && phase_filter.is_none_or(|p| phase == p)
            })
            .map(|(run, _)| *run)
            .collect();
        Some(matching)
    } else {
        None
    };

    // When filters are active, iterate over allowed run numbers and check for log files.
    // When no filters, scan the directory for all log files.
    let mut log_files: Vec<(u32, std::path::PathBuf, bool)> = Vec::new(); // (run_num, path, exists)

    if let Some(ref allowed) = filter_runs {
        // For each allowed run number, look for its primary log file.
        for run_num in allowed {
            let path = runs_dir.join(format!("{run_num:03}.log"));
            log_files.push((*run_num, path.clone(), path.exists()));
        }
    } else if runs_dir.exists() {
        // Scan directory for all .log files.
        let mut entries: Vec<(u32, std::path::PathBuf)> = std::fs::read_dir(&runs_dir)?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let fname = e.file_name();
                let name = fname.to_string_lossy().into_owned();
                if !name.ends_with(".log") || name.len() < 3 {
                    return None;
                }
                let run_num: u32 = name[..3].parse().ok()?;
                Some((run_num, e.path()))
            })
            .collect();
        // Sort by filename for deterministic order.
        entries.sort_by(|a, b| a.1.file_name().cmp(&b.1.file_name()));
        for (run_num, path) in entries {
            log_files.push((run_num, path, true));
        }
    }

    if log_files.is_empty() {
        if output_format == crate::cli::OutputFormat::Jsonl {
            return Ok(String::new());
        }
        return Ok("No run logs found.\n".to_string());
    }

    let mut out = String::new();
    for (run_num, log_path, exists) in &log_files {
        let meta = run_meta.get(run_num);
        let log_content = if *exists {
            std::fs::read_to_string(log_path)?
        } else {
            "(log not found)\n".to_string()
        };

        if output_format == crate::cli::OutputFormat::Jsonl {
            let entry = ClaudeOutputEntry {
                run: *run_num,
                cycle: meta.map(|(c, _)| *c),
                phase: meta.map(|(_, p)| p.clone()),
                log: log_content,
            };
            if let Ok(json) = serde_json::to_string(&entry) {
                out.push_str(&json);
                out.push('\n');
            }
        } else {
            let header = match meta {
                Some((cycle, phase)) => {
                    format!("=== Run {:3} | Cycle {} | {} ===\n", run_num, cycle, phase)
                }
                None => format!("=== Run {:3} ===\n", run_num),
            };
            out.push_str(&header);
            out.push_str(&log_content);
            if !log_content.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
    }

    Ok(out)
}

/// Load declared flow from `workflow_contracts.json` in `run_dir`.
/// Returns an empty vec if the file does not exist.
pub fn load_declared_flow(run_dir: &Path) -> Result<Vec<DeclaredFlow>> {
    let path = run_dir.join("workflow_contracts.json");
    if !path.exists() {
        return Ok(vec![]);
    }
    let json = std::fs::read_to_string(&path)?;
    let flows: Vec<DeclaredFlow> = serde_json::from_str(&json)?;
    Ok(flows)
}

/// Load actual file changes for a run by correlating costs.jsonl with manifests.
pub fn load_actual_changes(run_dir: &Path) -> Result<Vec<ActualFileChange>> {
    use crate::manifest::{diff_manifests, read_manifest_gz};

    let costs_path = run_dir.join("costs.jsonl");
    let manifests_dir = run_dir.join("manifests");

    if !costs_path.exists() || !manifests_dir.exists() {
        return Ok(vec![]);
    }

    let mut changes = Vec::new();

    // Load the initial before-manifest.
    let before_path = manifests_dir.join("000-before.json.gz");
    let mut prev_manifest = if before_path.exists() {
        read_manifest_gz(&before_path).ok()
    } else {
        None
    };

    // Iterate cost entries in order to correlate with manifests.
    let entries: Vec<crate::audit::CostEntry> = crate::audit::stream_cost_entries(&costs_path)?
        .filter_map(|r| r.ok())
        .collect();

    for entry in &entries {
        let after_path = manifests_dir.join(format!("{:03}-after.json.gz", entry.run));
        if !after_path.exists() {
            continue;
        }

        let after_manifest = match read_manifest_gz(&after_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if let Some(ref prev) = prev_manifest {
            let diff = diff_manifests(prev, &after_manifest);
            for path in diff.added {
                changes.push(ActualFileChange {
                    path,
                    phase: entry.phase.clone(),
                    cycle: entry.cycle,
                    run: entry.run,
                    iteration: entry.iteration,
                    change_type: ChangeType::Added,
                });
            }
            for path in diff.modified {
                changes.push(ActualFileChange {
                    path,
                    phase: entry.phase.clone(),
                    cycle: entry.cycle,
                    run: entry.run,
                    iteration: entry.iteration,
                    change_type: ChangeType::Modified,
                });
            }
            for path in diff.deleted {
                changes.push(ActualFileChange {
                    path,
                    phase: entry.phase.clone(),
                    cycle: entry.cycle,
                    run: entry.run,
                    iteration: entry.iteration,
                    change_type: ChangeType::Deleted,
                });
            }
        }

        prev_manifest = Some(after_manifest);
    }

    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::CostEntry;
    use crate::cli::OutputFormat;

    fn make_entry(
        run: u32,
        cycle: u32,
        phase: &str,
        iteration: u32,
        cost: Option<f64>,
        files_changed: u32,
    ) -> CostEntry {
        CostEntry {
            run,
            cycle,
            phase: phase.to_string(),
            iteration,
            cost_usd: cost,
            input_tokens: None,
            output_tokens: None,
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: files_changed,
            files_deleted: 0,
            files_changed,
            event: None,
            produces_violations: vec![],
        }
    }

    #[test]
    fn render_cycles_shows_per_cycle_breakdown() {
        let entries = vec![
            make_entry(1, 1, "builder", 1, Some(0.092), 3),
            make_entry(2, 1, "builder", 2, Some(0.088), 2),
            make_entry(3, 2, "builder", 1, Some(0.087), 4),
        ];
        let output = render_cycles(&entries, None, None, OutputFormat::Human);
        assert!(output.contains("Cycle 1:"), "should show Cycle 1 header");
        assert!(output.contains("Cycle 2:"), "should show Cycle 2 header");
        assert!(output.contains("builder"), "should show phase name");
        assert!(output.contains("$0.092"), "should show run cost");
        assert!(output.contains("3 files changed"), "should show file count");
    }

    #[test]
    fn render_cycles_cycle_filter_shows_only_requested_cycle() {
        let entries = vec![
            make_entry(1, 1, "builder", 1, Some(0.092), 3),
            make_entry(2, 2, "builder", 1, Some(0.087), 4),
        ];
        let output = render_cycles(&entries, Some(2), None, OutputFormat::Human);
        assert!(!output.contains("Cycle 1:"), "should not show Cycle 1");
        assert!(output.contains("Cycle 2:"), "should show Cycle 2");
        assert!(output.contains("$0.087"), "should show cycle 2 cost");
    }

    #[test]
    fn render_cycles_no_cost_shows_dash() {
        let entries = vec![make_entry(1, 1, "builder", 1, None, 0)];
        let output = render_cycles(&entries, None, None, OutputFormat::Human);
        assert!(output.contains("—"), "should show dash for missing cost");
    }

    #[test]
    fn render_cycles_jsonl_mode_emits_structured_data() {
        let entries = vec![
            make_entry(1, 1, "builder", 1, Some(0.092), 3),
            make_entry(2, 1, "reviewer", 1, Some(0.104), 1),
        ];
        let output = render_cycles(&entries, None, None, OutputFormat::Jsonl);
        let line = output
            .lines()
            .next()
            .expect("should emit at least one line");
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["cycle"], 1);
        assert!(json["runs"].is_array(), "should have runs array");
        assert!(
            json["total_cost_usd"].is_number(),
            "should have total_cost_usd"
        );
    }

    #[test]
    fn render_cycles_signal_run_shows_signal_marker() {
        let entries = vec![
            make_entry(1, 1, "builder", 1, Some(0.090), 2),
            make_entry(2, 1, "builder", 2, Some(0.091), 1),
        ];
        let output = render_cycles(&entries, None, Some(2), OutputFormat::Human);
        assert!(
            output.contains("✓ SIGNAL"),
            "should show SIGNAL marker on signal run"
        );
        // Run 1 should not have the signal marker
        assert!(
            !output.contains("Run  1") || {
                let lines: Vec<&str> = output.lines().collect();
                !lines
                    .iter()
                    .any(|l| l.contains("Run  1") && l.contains("SIGNAL"))
            },
            "should not show SIGNAL on non-signal run"
        );
    }

    fn make_entry_with_tokens(
        run: u32,
        cycle: u32,
        phase: &str,
        iteration: u32,
        cost: Option<f64>,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        confidence: &str,
    ) -> CostEntry {
        CostEntry {
            run,
            cycle,
            phase: phase.to_string(),
            iteration,
            cost_usd: cost,
            input_tokens,
            output_tokens,
            cost_confidence: confidence.to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        }
    }

    #[test]
    fn render_costs_displays_per_run_table() {
        let entries = vec![
            make_entry_with_tokens(
                1,
                1,
                "builder",
                1,
                Some(0.092),
                Some(1234),
                Some(567),
                "full",
            ),
            make_entry_with_tokens(
                2,
                1,
                "builder",
                2,
                Some(0.088),
                Some(1198),
                Some(534),
                "full",
            ),
        ];
        let output = render_costs(&entries, None, OutputFormat::Human);
        assert!(output.contains("Cost breakdown:"), "should show header");
        assert!(output.contains("$0.092"), "should show run 1 cost");
        assert!(output.contains("$0.088"), "should show run 2 cost");
        assert!(output.contains("builder"), "should show phase name");
        assert!(output.contains("Total"), "should show totals row");
        assert!(output.contains("$0.180"), "should show total cost");
    }

    #[test]
    fn render_costs_phase_filter_shows_only_matching_phase() {
        let entries = vec![
            make_entry_with_tokens(
                1,
                1,
                "builder",
                1,
                Some(0.089),
                Some(1000),
                Some(500),
                "full",
            ),
            make_entry_with_tokens(
                2,
                1,
                "reviewer",
                1,
                Some(0.105),
                Some(900),
                Some(400),
                "full",
            ),
        ];
        let output = render_costs(&entries, Some("builder"), OutputFormat::Human);
        assert!(output.contains("builder"), "should show builder phase");
        assert!(
            !output.contains("reviewer"),
            "should not show reviewer phase"
        );
        assert!(output.contains("$0.089"), "should show builder cost");
        assert!(!output.contains("$0.105"), "should not show reviewer cost");
    }

    #[test]
    fn render_costs_totals_row_sums_correctly() {
        let entries = vec![
            make_entry_with_tokens(
                1,
                1,
                "builder",
                1,
                Some(0.100),
                Some(1000),
                Some(500),
                "full",
            ),
            make_entry_with_tokens(
                2,
                1,
                "builder",
                2,
                Some(0.200),
                Some(2000),
                Some(1000),
                "full",
            ),
        ];
        let output = render_costs(&entries, None, OutputFormat::Human);
        assert!(
            output.contains("$0.300"),
            "totals should sum cost correctly"
        );
        // Total input tokens = 3000
        assert!(output.contains("3,000"), "totals should sum input tokens");
        // Total output tokens = 1500
        assert!(output.contains("1,500"), "totals should sum output tokens");
    }

    #[test]
    fn render_costs_jsonl_mode_emits_one_object_per_run() {
        let entries = vec![
            make_entry_with_tokens(
                1,
                1,
                "builder",
                1,
                Some(0.092),
                Some(1234),
                Some(567),
                "full",
            ),
            make_entry_with_tokens(
                2,
                1,
                "reviewer",
                1,
                Some(0.104),
                Some(900),
                Some(400),
                "partial",
            ),
        ];
        let output = render_costs(&entries, None, OutputFormat::Jsonl);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2, "should emit one JSON line per run");
        let obj0: serde_json::Value =
            serde_json::from_str(lines[0]).expect("line 0 should be valid JSON");
        assert_eq!(obj0["run"], 1);
        assert_eq!(obj0["phase"], "builder");
        assert_eq!(obj0["confidence"], "full");
        let obj1: serde_json::Value =
            serde_json::from_str(lines[1]).expect("line 1 should be valid JSON");
        assert_eq!(obj1["run"], 2);
        assert_eq!(obj1["confidence"], "partial");
    }

    // ── render_claude_output tests ──────────────────────────────────────────

    fn make_run_dir_with_logs(dir: &std::path::Path, logs: &[(u32, &str)]) -> std::path::PathBuf {
        let runs_dir = dir.join("runs");
        std::fs::create_dir_all(&runs_dir).unwrap();
        for (run_num, content) in logs {
            let path = runs_dir.join(format!("{run_num:03}.log"));
            std::fs::write(path, content).unwrap();
        }
        dir.to_path_buf()
    }

    fn cost_entry(run: u32, cycle: u32, phase: &str) -> CostEntry {
        CostEntry {
            run,
            cycle,
            phase: phase.to_string(),
            iteration: 1,
            cost_usd: None,
            input_tokens: None,
            output_tokens: None,
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
    fn render_claude_output_displays_log_contents_with_headers() {
        let tmp = tempfile::tempdir().unwrap();
        let run_dir = make_run_dir_with_logs(
            tmp.path(),
            &[(1, "hello from run 1"), (2, "hello from run 2")],
        );
        let entries = vec![cost_entry(1, 1, "builder"), cost_entry(2, 1, "reviewer")];
        let output =
            render_claude_output(&run_dir, &entries, None, None, OutputFormat::Human).unwrap();
        assert!(output.contains("Run   1"), "should show run 1 header");
        assert!(output.contains("Run   2"), "should show run 2 header");
        assert!(output.contains("hello from run 1"), "should show run 1 log");
        assert!(output.contains("hello from run 2"), "should show run 2 log");
    }

    #[test]
    fn render_claude_output_cycle_filter_shows_only_matching_cycle() {
        let tmp = tempfile::tempdir().unwrap();
        let run_dir = make_run_dir_with_logs(tmp.path(), &[(1, "cycle 1 log"), (2, "cycle 2 log")]);
        let entries = vec![cost_entry(1, 1, "builder"), cost_entry(2, 2, "builder")];
        let output =
            render_claude_output(&run_dir, &entries, Some(1), None, OutputFormat::Human).unwrap();
        assert!(output.contains("cycle 1 log"), "should show cycle 1 log");
        assert!(
            !output.contains("cycle 2 log"),
            "should not show cycle 2 log"
        );
    }

    #[test]
    fn render_claude_output_phase_filter_shows_only_matching_phase() {
        let tmp = tempfile::tempdir().unwrap();
        let run_dir =
            make_run_dir_with_logs(tmp.path(), &[(1, "builder output"), (2, "reviewer output")]);
        let entries = vec![cost_entry(1, 1, "builder"), cost_entry(2, 1, "reviewer")];
        let output = render_claude_output(
            &run_dir,
            &entries,
            None,
            Some("builder"),
            OutputFormat::Human,
        )
        .unwrap();
        assert!(output.contains("builder output"), "should show builder log");
        assert!(
            !output.contains("reviewer output"),
            "should not show reviewer log"
        );
    }

    #[test]
    fn render_claude_output_missing_log_file_shows_graceful_message() {
        let tmp = tempfile::tempdir().unwrap();
        // Create runs dir but no log file for run 1
        std::fs::create_dir_all(tmp.path().join("runs")).unwrap();
        let entries = vec![cost_entry(1, 1, "builder")];
        let output =
            render_claude_output(tmp.path(), &entries, Some(1), None, OutputFormat::Human).unwrap();
        assert!(
            output.contains("log not found"),
            "should show graceful message for missing log"
        );
    }

    #[test]
    fn render_claude_output_jsonl_mode_emits_structured_output() {
        let tmp = tempfile::tempdir().unwrap();
        let run_dir = make_run_dir_with_logs(tmp.path(), &[(1, "output line 1\noutput line 2\n")]);
        let entries = vec![cost_entry(1, 1, "builder")];
        let output =
            render_claude_output(&run_dir, &entries, None, None, OutputFormat::Jsonl).unwrap();
        let line = output
            .lines()
            .next()
            .expect("should emit at least one line");
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["run"], 1);
        assert_eq!(json["cycle"], 1);
        assert_eq!(json["phase"], "builder");
        assert!(
            json["log"].as_str().unwrap().contains("output line 1"),
            "should include log content"
        );
    }

    // ── render_files_changed tests ──────────────────────────────────────────

    fn make_file_change(
        path: &str,
        run: u32,
        cycle: u32,
        phase: &str,
        iteration: u32,
        change_type: ChangeType,
    ) -> ActualFileChange {
        ActualFileChange {
            path: path.to_string(),
            run,
            cycle,
            phase: phase.to_string(),
            iteration,
            change_type,
        }
    }

    #[test]
    fn render_files_changed_shows_files_attributed_to_runs() {
        let changes = vec![
            make_file_change("src/main.rs", 1, 1, "builder", 1, ChangeType::Modified),
            make_file_change("src/main.rs", 5, 2, "builder", 1, ChangeType::Modified),
            make_file_change("review.md", 4, 1, "reviewer", 1, ChangeType::Added),
        ];
        let output = render_files_changed(&changes, None, None, OutputFormat::Human);
        assert!(
            output.contains("File change history:"),
            "should show header"
        );
        assert!(output.contains("src/main.rs"), "should show main.rs");
        assert!(output.contains("review.md"), "should show review.md");
        assert!(output.contains("modified"), "should show change type");
        assert!(output.contains("created"), "should show added as created");
        assert!(output.contains("run  1"), "should show run 1");
        assert!(output.contains("run  5"), "should show run 5");
    }

    #[test]
    fn render_files_changed_cycle_filter_shows_only_cycle_1() {
        let changes = vec![
            make_file_change("src/a.rs", 1, 1, "builder", 1, ChangeType::Modified),
            make_file_change("src/a.rs", 3, 2, "builder", 1, ChangeType::Modified),
        ];
        let output = render_files_changed(&changes, Some(1), None, OutputFormat::Human);
        assert!(output.contains("run  1"), "should show run 1");
        assert!(
            !output.contains("run  3"),
            "should not show run 3 (cycle 2)"
        );
    }

    #[test]
    fn render_files_changed_no_manifest_data_shows_helpful_message() {
        let output = render_files_changed(&[], None, None, OutputFormat::Human);
        assert!(
            output.contains("manifest_enabled"),
            "should mention manifest_enabled setting"
        );
    }

    #[test]
    fn render_files_changed_jsonl_mode_emits_structured_output() {
        let changes = vec![
            make_file_change("src/lib.rs", 2, 1, "builder", 2, ChangeType::Modified),
            make_file_change("tests/foo.rs", 4, 1, "reviewer", 1, ChangeType::Added),
        ];
        let output = render_files_changed(&changes, None, None, OutputFormat::Jsonl);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2, "should emit one line per change event");
        let obj0: serde_json::Value =
            serde_json::from_str(lines[0]).expect("line 0 should be valid JSON");
        assert_eq!(obj0["path"], "src/lib.rs");
        assert_eq!(obj0["change_type"], "modified");
        assert_eq!(obj0["run"], 2);
        assert_eq!(obj0["cycle"], 1);
        assert_eq!(obj0["phase"], "builder");
        assert_eq!(obj0["iteration"], 2);
        let obj1: serde_json::Value =
            serde_json::from_str(lines[1]).expect("line 1 should be valid JSON");
        assert_eq!(obj1["change_type"], "created");
        assert_eq!(obj1["phase"], "reviewer");
    }

    // ── render_data_flow_declared tests ──────────────────────────────────────

    fn make_declared_flow(phase: &str, consumes: &[&str], produces: &[&str]) -> DeclaredFlow {
        DeclaredFlow {
            phase: phase.to_string(),
            consumes: consumes.iter().map(|s| s.to_string()).collect(),
            produces: produces.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn render_data_flow_declared_shows_consumes_and_produces() {
        let phases = vec![
            make_declared_flow("builder", &["specs/**/*.md"], &["src/**/*.rs"]),
            make_declared_flow("reviewer", &["src/**/*.rs"], &["review-notes.md"]),
        ];
        let output = render_data_flow_declared(&phases, None, OutputFormat::Human);
        assert!(
            output.contains("Declared data flow"),
            "should show declared header"
        );
        assert!(output.contains("builder"), "should show builder phase");
        assert!(output.contains("reviewer"), "should show reviewer phase");
        assert!(output.contains("specs/**/*.md"), "should show consumes");
        assert!(
            output.contains("src/**/*.rs"),
            "should show produces/consumes"
        );
        assert!(
            output.contains("review-notes.md"),
            "should show reviewer produces"
        );
    }

    #[test]
    fn render_data_flow_declared_empty_shows_no_contracts() {
        let output = render_data_flow_declared(&[], None, OutputFormat::Human);
        assert!(
            output.contains("no contracts declared"),
            "should show no contracts message"
        );
    }

    #[test]
    fn render_data_flow_declared_phase_filter_shows_only_matching_phase() {
        let phases = vec![
            make_declared_flow("builder", &["specs/**/*.md"], &["src/**/*.rs"]),
            make_declared_flow("reviewer", &["src/**/*.rs"], &["review-notes.md"]),
        ];
        let output = render_data_flow_declared(&phases, Some("builder"), OutputFormat::Human);
        assert!(output.contains("builder"), "should show builder");
        assert!(!output.contains("reviewer"), "should not show reviewer");
    }

    #[test]
    fn render_data_flow_declared_jsonl_mode_emits_one_object_per_phase() {
        let phases = vec![
            make_declared_flow("builder", &["specs/**/*.md"], &["src/**/*.rs"]),
            make_declared_flow("reviewer", &["src/**/*.rs"], &["review-notes.md"]),
        ];
        let output = render_data_flow_declared(&phases, None, OutputFormat::Jsonl);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2, "should emit one line per phase");
        let obj0: serde_json::Value =
            serde_json::from_str(lines[0]).expect("line 0 should be valid JSON");
        assert_eq!(obj0["phase"], "builder");
        assert!(obj0["consumes"].is_array(), "should have consumes array");
        assert!(obj0["produces"].is_array(), "should have produces array");
    }

    // ── render_data_flow_actual tests ────────────────────────────────────────

    #[test]
    fn render_data_flow_actual_shows_files_with_cycles() {
        let changes = vec![
            make_file_change("src/main.rs", 1, 1, "builder", 1, ChangeType::Modified),
            make_file_change("src/main.rs", 5, 2, "builder", 1, ChangeType::Modified),
            make_file_change("review.md", 4, 1, "reviewer", 1, ChangeType::Added),
        ];
        let output = render_data_flow_actual(&changes, None, None, OutputFormat::Human);
        assert!(
            output.contains("Actual file attribution"),
            "should show actual header"
        );
        assert!(output.contains("src/main.rs"), "should show main.rs");
        assert!(output.contains("builder"), "should show builder phase");
        assert!(
            output.contains("cycles 1, 2"),
            "should show cycles for repeated file"
        );
        assert!(output.contains("review.md"), "should show review.md");
    }

    #[test]
    fn render_data_flow_actual_empty_shows_no_changes() {
        let output = render_data_flow_actual(&[], None, None, OutputFormat::Human);
        assert!(
            output.contains("no file changes recorded"),
            "should show no changes message"
        );
    }

    #[test]
    fn render_data_flow_actual_cycle_filter_shows_only_matching_cycle() {
        let changes = vec![
            make_file_change("src/a.rs", 1, 1, "builder", 1, ChangeType::Modified),
            make_file_change("src/b.rs", 3, 2, "builder", 1, ChangeType::Modified),
        ];
        let output = render_data_flow_actual(&changes, Some(1), None, OutputFormat::Human);
        assert!(output.contains("src/a.rs"), "should show cycle 1 file");
        assert!(!output.contains("src/b.rs"), "should not show cycle 2 file");
    }

    #[test]
    fn render_data_flow_actual_phase_filter_shows_only_matching_phase() {
        let changes = vec![
            make_file_change("src/main.rs", 1, 1, "builder", 1, ChangeType::Modified),
            make_file_change("review.md", 2, 1, "reviewer", 1, ChangeType::Added),
        ];
        let output = render_data_flow_actual(&changes, None, Some("builder"), OutputFormat::Human);
        assert!(output.contains("src/main.rs"), "should show builder file");
        assert!(
            !output.contains("review.md"),
            "should not show reviewer file"
        );
    }

    #[test]
    fn render_data_flow_actual_jsonl_mode_emits_grouped_by_path_and_phase() {
        let changes = vec![
            make_file_change("src/main.rs", 1, 1, "builder", 1, ChangeType::Modified),
            make_file_change("src/main.rs", 5, 2, "builder", 1, ChangeType::Modified),
            make_file_change("review.md", 4, 1, "reviewer", 1, ChangeType::Added),
        ];
        let output = render_data_flow_actual(&changes, None, None, OutputFormat::Jsonl);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(
            lines.len(),
            2,
            "should emit one line per (path, phase) group"
        );
        // Lines are sorted by (path, phase), so src/main.rs before review.md
        let obj0: serde_json::Value =
            serde_json::from_str(lines[0]).expect("line 0 should be valid JSON");
        assert_eq!(obj0["path"], "review.md");
        assert_eq!(obj0["phase"], "reviewer");
        assert!(obj0["cycles"].is_array(), "should have cycles array");
        let obj1: serde_json::Value =
            serde_json::from_str(lines[1]).expect("line 1 should be valid JSON");
        assert_eq!(obj1["path"], "src/main.rs");
        assert_eq!(obj1["phase"], "builder");
        let cycles = obj1["cycles"].as_array().unwrap();
        assert_eq!(cycles.len(), 2, "should have 2 unique cycles");
    }
}
