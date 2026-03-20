use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    pub change_type: ChangeType,
}

/// Render the declared data-flow graph from a list of `DeclaredFlow` entries.
///
/// Output format:
/// ```text
/// Data flow (declared):
///   specs/**/*.md  ──→  [builder]  ──→  src/**/*.rs
///                                       tests/**/*.rs
///   src/**/*.rs   ──→  [reviewer] ──→  review-notes.md
///   tests/**/*.rs ──→  [reviewer]
/// ```
pub fn render_data_flow_declared(phases: &[DeclaredFlow]) -> String {
    let mut lines = vec!["Data flow (declared):".to_string()];

    if phases.is_empty() {
        return lines.join("\n") + "\n";
    }

    // Compute global column widths.
    let left_width = phases
        .iter()
        .flat_map(|p| p.consumes.iter().map(String::as_str))
        .map(str::len)
        .max()
        .unwrap_or(0);

    for phase in phases {
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
/// Output format:
/// ```text
/// Data flow (actual):
///   src/main.rs      modified by builder (run 5)
///   src/engine.rs    modified by builder (runs 6, 7)
///   review-notes.md  created by reviewer (run 8)
/// ```
pub fn render_data_flow_actual(changes: &[ActualFileChange]) -> String {
    if changes.is_empty() {
        return "Data flow (actual):\n  (no file changes recorded)\n".to_string();
    }

    let mut lines = vec!["Data flow (actual):".to_string()];

    // Group by (path, phase, change_type) and collect run numbers.
    // Use BTreeMap for deterministic ordering.
    let mut grouped: BTreeMap<(String, String, String), Vec<u32>> = BTreeMap::new();

    for change in changes {
        let ct = match change.change_type {
            ChangeType::Added => "created",
            ChangeType::Modified => "modified",
            ChangeType::Deleted => "deleted",
        };
        grouped
            .entry((change.path.clone(), change.phase.clone(), ct.to_string()))
            .or_default()
            .push(change.run);
    }

    let path_width = changes.iter().map(|c| c.path.len()).max().unwrap_or(0);

    for ((path, phase, ct), runs) in &grouped {
        let mut runs_sorted = runs.clone();
        runs_sorted.sort();
        runs_sorted.dedup();

        let runs_str = if runs_sorted.len() == 1 {
            format!("run {}", runs_sorted[0])
        } else {
            let nums: Vec<String> = runs_sorted.iter().map(|r| r.to_string()).collect();
            format!("runs {}", nums.join(", "))
        };

        lines.push(format!(
            "  {:<pw$}  {} by {} ({})",
            path,
            ct,
            phase,
            runs_str,
            pw = path_width
        ));
    }

    lines.join("\n") + "\n"
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
                    change_type: ChangeType::Added,
                });
            }
            for path in diff.modified {
                changes.push(ActualFileChange {
                    path,
                    phase: entry.phase.clone(),
                    cycle: entry.cycle,
                    run: entry.run,
                    change_type: ChangeType::Modified,
                });
            }
            for path in diff.deleted {
                changes.push(ActualFileChange {
                    path,
                    phase: entry.phase.clone(),
                    cycle: entry.cycle,
                    run: entry.run,
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
}
