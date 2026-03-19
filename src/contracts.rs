use anyhow::Result;
use globset::{GlobBuilder, GlobSetBuilder};
use std::path::Path;
use walkdir::WalkDir;

/// Extract the literal prefix before the first glob metacharacter (*, ?, [).
/// Returns the full pattern if it contains no metacharacters.
/// Returns "" if the pattern starts with a metacharacter.
pub fn non_glob_prefix(pattern: &str) -> &str {
    match pattern.find(['*', '?', '[']) {
        Some(0) => "",
        Some(idx) => &pattern[..idx],
        None => pattern,
    }
}

#[derive(Debug, Clone)]
pub enum ContractWarning {
    ConsumesNoMatchStartup {
        phase: String,
        pattern: String,
        context_dir: String,
    },
    ConsumesNoMatchRun {
        phase: String,
        pattern: String,
        cycle: u32,
        run: u32,
    },
}

impl ContractWarning {
    pub fn format_message(&self) -> String {
        match self {
            ContractWarning::ConsumesNoMatchStartup {
                phase,
                pattern,
                context_dir,
            } => {
                format!(
                    "⚠  Phase \"{}\" declares consumes = [\"{}\"]\n   but no matching files exist in context_dir (\"{}\")\n   and the pattern is not mentioned in the prompt.\n   This phase may silently do nothing if its expected inputs are never created.\n   Suppress with --no-contract-check or fix the consumes declaration.",
                    phase, pattern, context_dir
                )
            }
            ContractWarning::ConsumesNoMatchRun {
                phase,
                pattern,
                cycle,
                run,
            } => {
                format!(
                    "⚠  Phase \"{}\" (run {}, cycle {}): consumes = [\"{}\"]\n   but no matching files found in context_dir. The phase may operate on missing inputs.",
                    phase, run, cycle, pattern
                )
            }
        }
    }
}

/// Check if any files in context_dir match the given glob pattern.
fn any_files_match(context_dir: &Path, pattern: &str) -> Result<bool> {
    let mut builder = GlobSetBuilder::new();
    let glob = GlobBuilder::new(pattern)
        .build()
        .map_err(|e| anyhow::anyhow!("Invalid glob pattern '{}': {}", pattern, e))?;
    builder.add(glob);
    let globset = builder
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build glob set: {}", e))?;

    if !context_dir.exists() {
        return Ok(false);
    }

    for entry in WalkDir::new(context_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        if let Ok(rel) = path.strip_prefix(context_dir) {
            if globset.is_match(rel) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Startup check per phase: for each consumes pattern, warn if no files in context_dir
/// match AND the pattern's non-glob prefix does not appear as a substring in prompt_text.
pub fn check_consumes_at_startup(
    phase_name: &str,
    consumes: &[String],
    context_dir: &Path,
    prompt_text: &str,
) -> Result<Vec<ContractWarning>> {
    let mut warnings = Vec::new();
    for pattern in consumes {
        let files_exist = any_files_match(context_dir, pattern)?;
        if files_exist {
            continue;
        }
        // No files match — check non-glob prefix in prompt
        let prefix = non_glob_prefix(pattern);
        // Empty prefix: only file existence can suppress (skip prompt check)
        let mentioned_in_prompt = if prefix.is_empty() {
            false
        } else {
            prompt_text.contains(prefix)
        };
        if !mentioned_in_prompt {
            warnings.push(ContractWarning::ConsumesNoMatchStartup {
                phase: phase_name.to_string(),
                pattern: pattern.clone(),
                context_dir: context_dir.display().to_string(),
            });
        }
    }
    Ok(warnings)
}

/// Pre-run check (only called for cycle >= 2): warn if patterns still match nothing.
pub fn check_consumes_pre_run(
    phase_name: &str,
    consumes: &[String],
    context_dir: &Path,
    cycle: u32,
    run: u32,
) -> Result<Vec<ContractWarning>> {
    let mut warnings = Vec::new();
    for pattern in consumes {
        let files_exist = any_files_match(context_dir, pattern)?;
        if !files_exist {
            warnings.push(ContractWarning::ConsumesNoMatchRun {
                phase: phase_name.to_string(),
                pattern: pattern.clone(),
                cycle,
                run,
            });
        }
    }
    Ok(warnings)
}

/// Post-run check: returns patterns that matched no files in added+modified.
/// Deleted files do NOT satisfy a produces pattern.
/// Returns [] when produces is empty.
pub fn check_produces_after_run(
    produces: &[String],
    diff_added: &[String],
    diff_modified: &[String],
) -> Vec<String> {
    if produces.is_empty() {
        return vec![];
    }

    let changed: Vec<&str> = diff_added
        .iter()
        .chain(diff_modified.iter())
        .map(|s| s.as_str())
        .collect();

    let mut violations = Vec::new();
    for pattern in produces {
        let mut builder = GlobSetBuilder::new();
        match GlobBuilder::new(pattern).build() {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(_) => {
                // Invalid pattern — treat as violation
                violations.push(pattern.clone());
                continue;
            }
        }
        let globset = match builder.build() {
            Ok(gs) => gs,
            Err(_) => {
                violations.push(pattern.clone());
                continue;
            }
        };

        let matched = changed.iter().any(|path| globset.is_match(path));
        if !matched {
            violations.push(pattern.clone());
        }
    }
    violations
}
