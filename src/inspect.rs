use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

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
