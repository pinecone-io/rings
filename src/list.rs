use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::duration::SinceSpec;
use crate::state::{RunMeta, RunStatus};

/// Summary information about a single run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub started_at: DateTime<Utc>,
    pub workflow: String,
    pub status: RunStatus,
    pub cycles_completed: u32,
    pub total_cost_usd: Option<f64>,
}

/// Filters for list_runs.
#[derive(Debug)]
pub struct ListFilters {
    pub since: Option<SinceSpec>,
    pub status: Option<RunStatus>,
    pub workflow: Option<String>,
    pub limit: usize,
}

/// List recent workflow runs from base_dir, with optional filtering.
///
/// Strategy: scan directory in reverse chronological order (names are lexicographically
/// ordered as run_YYYYMMDD_...), apply filters inline, break once limit is reached.
pub fn list_runs(filters: &ListFilters, base_dir: &Path) -> Result<Vec<RunSummary>> {
    // If base_dir doesn't exist, return empty list
    if !base_dir.exists() {
        return Ok(vec![]);
    }

    let mut entries = std::fs::read_dir(base_dir)
        .with_context(|| format!("Cannot read directory: {}", base_dir.display()))?
        .filter_map(|e| e.ok())
        .collect::<Vec<_>>();

    // Sort by name in reverse (chronologically descending)
    entries.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

    let mut results = Vec::new();

    for entry in entries {
        if results.len() >= filters.limit {
            break;
        }

        let path = entry.path();

        // Skip non-directories
        if !path.is_dir() {
            continue;
        }

        let run_toml_path = path.join("run.toml");

        // Try to read run.toml
        let meta = match RunMeta::read(&run_toml_path) {
            Ok(m) => m,
            Err(_) => {
                // Skip directories with missing or corrupt run.toml
                continue;
            }
        };

        // Parse started_at
        let started_at = match chrono::DateTime::parse_from_rfc3339(&meta.started_at) {
            Ok(dt) => dt.with_timezone(&Utc),
            Err(_) => {
                // Skip entries with unparseable timestamps
                continue;
            }
        };

        // Apply since filter
        if let Some(ref since) = filters.since {
            let cutoff = since.to_cutoff_datetime();
            if started_at < cutoff {
                continue;
            }
        }

        // Apply status filter
        if let Some(ref target_status) = filters.status {
            if meta.status != *target_status {
                continue;
            }
        }

        // Apply workflow filter (substring match on the file path)
        if let Some(ref target_workflow) = filters.workflow {
            if !meta.workflow_file.contains(target_workflow) {
                continue;
            }
        }

        // Extract workflow name from path (last component)
        let workflow = meta
            .workflow_file
            .split('/')
            .next_back()
            .unwrap_or("unknown")
            .to_string();

        // Try to read state.json to get cycles_completed and cost
        let state_path = path.join("state.json");
        let (cycles_completed, total_cost_usd) = match crate::state::StateFile::read(&state_path) {
            Ok(state) => (state.last_completed_cycle, Some(state.cumulative_cost_usd)),
            Err(_) => (0, None),
        };

        results.push(RunSummary {
            run_id: meta.run_id,
            started_at,
            workflow,
            status: meta.status,
            cycles_completed,
            total_cost_usd,
        });
    }

    Ok(results)
}
