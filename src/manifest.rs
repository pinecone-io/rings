use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

/// A single file entry in a manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub modified: String, // RFC 3339
}

/// A complete manifest of the context directory at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifest {
    pub timestamp: String, // RFC 3339
    pub run: u32,
    pub cycle: u32,
    pub phase: String,
    pub iteration: u32,
    pub root: String,
    pub files: Vec<FileEntry>,
}

/// Differences between two manifests.
#[derive(Debug, Clone, Default)]
pub struct FileDiff {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
    /// Total number of files added + modified + deleted.
    pub files_changed: u32,
}

/// Hardcoded credential file patterns that are never included in manifests,
/// regardless of user-specified `manifest_ignore` settings.
///
/// # Security rationale
///
/// These files commonly contain private keys, TLS certificates, and secrets that
/// must never appear in audit logs or be transmitted to external systems. Excluding
/// them unconditionally prevents accidental exposure even when a user's ignore list
/// is misconfigured or overridden. This exclusion cannot be removed via workflow
/// configuration — if you need to track changes to a key file, use a naming
/// convention that does not match these patterns.
const CREDENTIAL_PATTERNS: &[&str] = &[
    "**/.env",
    "**/.env.*",
    "**/*_rsa",
    "**/*_ed25519",
    "**/*.pem",
    "**/*.key",
    "**/.netrc",
    "**/*.pfx",
    "**/*.p12",
];

/// mtime cache for fast manifest computation.
/// Maps file path to (mtime, sha256_hash).
type MtimeCache = HashMap<PathBuf, (SystemTime, Vec<u8>)>;

/// Compute a manifest of the given context directory.
///
/// # Arguments
///
/// * `context_dir` - The root directory to scan
/// * `output_dir` - The output directory (excluded from manifest)
/// * `run` - Run number
/// * `cycle` - Cycle number
/// * `phase` - Phase name
/// * `iteration` - Iteration number
/// * `user_patterns` - User-specified glob patterns to exclude
/// * `use_mtime_optimization` - Whether to use mtime cache
///
/// Returns a FileManifest with the computed file list.
#[allow(clippy::too_many_arguments)]
pub fn compute_manifest(
    context_dir: &Path,
    output_dir: &Path,
    run: u32,
    cycle: u32,
    phase: &str,
    iteration: u32,
    user_patterns: &[String],
    use_mtime_optimization: bool,
) -> Result<FileManifest> {
    let context_dir = context_dir
        .canonicalize()
        .context("context_dir not found")?;
    let output_dir_canonical = output_dir
        .canonicalize()
        .unwrap_or_else(|_| output_dir.to_path_buf());

    // Build glob set for user-specified exclusions
    let user_globset = build_glob_set(user_patterns)?;
    let cred_globset = build_glob_set(
        &CREDENTIAL_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    )?;

    // Load mtime cache from previous manifest if it exists and optimization enabled
    let _mtime_cache: MtimeCache = HashMap::new();
    if use_mtime_optimization {
        // This would be loaded from disk if needed, but for now we start fresh
        // A real implementation would load the previous manifest file
    }

    let mut entries = Vec::new();
    let mut file_count = 0;

    for entry in WalkDir::new(&context_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip directories
        if path.is_dir() {
            continue;
        }

        // Skip broken symlinks
        if path.is_symlink() && !path.exists() {
            eprintln!("⚠  Skipping broken symlink: {}", path.display());
            continue;
        }

        // Skip if inside output_dir
        if is_inside_path(path, &output_dir_canonical) {
            continue;
        }

        // Get relative path
        let rel_path = path
            .strip_prefix(&context_dir)
            .context("Failed to compute relative path")?;

        // Check against user patterns
        if user_globset.is_match(rel_path) {
            continue;
        }

        // Check against credential patterns
        if cred_globset.is_match(rel_path) {
            continue;
        }

        // Compute SHA256
        let sha256_hash = compute_file_hash(path)?;
        let sha256_hex = hex::encode(&sha256_hash);

        // Get metadata
        let metadata = fs::metadata(path).context("Failed to read file metadata")?;
        let size_bytes = metadata.len();
        let modified: DateTime<Utc> = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .and_then(|d| {
                Utc.timestamp_opt(d.as_secs() as i64, d.subsec_nanos())
                    .single()
            })
            .unwrap_or_else(Utc::now);

        let path_string = rel_path
            .to_str()
            .ok_or_else(|| anyhow!("Non-UTF-8 path: {:?}", rel_path))?
            .to_string();

        entries.push(FileEntry {
            path: path_string,
            sha256: sha256_hex,
            size_bytes,
            modified: modified.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        });

        file_count += 1;
    }

    // Warn if too many files
    if file_count > 10_000 {
        eprintln!("⚠  compute_manifest: found {} files (> 10,000)", file_count);
    }

    // Sort entries for consistent ordering
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(FileManifest {
        timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        run,
        cycle,
        phase: phase.to_string(),
        iteration,
        root: context_dir
            .to_str()
            .ok_or_else(|| anyhow!("Non-UTF-8 context_dir"))?
            .to_string(),
        files: entries,
    })
}

/// Write a manifest to disk with gzip compression.
///
/// Uses atomic write: writes to `.tmp` file first, then renames to final path.
pub fn write_manifest_gz(manifest: &FileManifest, path: &Path) -> Result<()> {
    // Create parent directory
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create manifests directory")?;
    }

    // Serialize manifest
    let json = serde_json::to_vec(manifest).context("Failed to serialize manifest")?;

    // Write to temporary file
    let tmp_path = path.with_extension("tmp");
    let file = File::create(&tmp_path).context("Failed to create temporary manifest file")?;
    let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    encoder
        .write_all(&json)
        .context("Failed to write gzip data")?;
    encoder.finish().context("Failed to finish gzip encoding")?;

    // Atomic rename
    fs::rename(&tmp_path, path).context("Failed to rename manifest file")?;

    Ok(())
}

/// Read and decompress a manifest from disk.
pub fn read_manifest_gz(path: &Path) -> Result<FileManifest> {
    let file = File::open(path).context("Failed to open manifest file")?;
    let decoder = flate2::read::GzDecoder::new(file);
    let manifest: FileManifest =
        serde_json::from_reader(decoder).context("Failed to deserialize manifest")?;
    Ok(manifest)
}

/// Compute the difference between two manifests.
pub fn diff_manifests(before: &FileManifest, after: &FileManifest) -> FileDiff {
    let before_map: HashMap<&str, &FileEntry> =
        before.files.iter().map(|e| (e.path.as_str(), e)).collect();
    let after_map: HashMap<&str, &FileEntry> =
        after.files.iter().map(|e| (e.path.as_str(), e)).collect();

    let mut diff = FileDiff::default();

    // Find added and modified files
    for (path, after_entry) in &after_map {
        if let Some(before_entry) = before_map.get(path) {
            // File exists in both; check if modified
            if before_entry.sha256 != after_entry.sha256 {
                diff.modified.push((*path).to_string());
            }
        } else {
            // File only in after; it was added
            diff.added.push((*path).to_string());
        }
    }

    // Find deleted files
    for path in before_map.keys() {
        if !after_map.contains_key(path) {
            diff.deleted.push((*path).to_string());
        }
    }

    // Sort for consistent output
    diff.added.sort();
    diff.modified.sort();
    diff.deleted.sort();

    diff.files_changed = (diff.added.len() + diff.modified.len() + diff.deleted.len()) as u32;

    diff
}

/// Compute SHA256 hash of a file.
fn compute_file_hash(path: &Path) -> Result<Vec<u8>> {
    let mut file = File::open(path).context("Failed to open file for hashing")?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).context("Failed to hash file")?;
    Ok(hasher.finalize().to_vec())
}

/// Build a compiled glob set from a list of patterns.
fn build_glob_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = GlobBuilder::new(pattern)
            .build()
            .context(format!("Invalid glob pattern: {}", pattern))?;
        builder.add(glob);
    }
    builder.build().context("Failed to build glob set")
}

/// Check if a path is inside another path.
fn is_inside_path(path: &Path, parent: &Path) -> bool {
    path.starts_with(parent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_compute_file_hash() {
        let tmpdir = TempDir::new().unwrap();
        let file_path = tmpdir.path().join("test.txt");
        fs::write(&file_path, "hello world").unwrap();

        let hash = compute_file_hash(&file_path).unwrap();
        let hex = hex::encode(&hash);

        // Known SHA256 of "hello world"
        assert_eq!(
            hex,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_credential_patterns_excluded() {
        let tmpdir = TempDir::new().unwrap();
        let context = tmpdir.path().join("context");
        fs::create_dir(&context).unwrap();

        // Create various files including credential patterns
        fs::write(context.join(".env"), "SECRET=value").unwrap();
        fs::write(context.join("id_rsa"), "key").unwrap();
        fs::write(context.join("cert.pem"), "cert").unwrap();
        fs::write(context.join("normal.txt"), "content").unwrap();

        let output = tmpdir.path().join("output");
        fs::create_dir(&output).unwrap();

        let manifest = compute_manifest(&context, &output, 1, 1, "test", 1, &[], false).unwrap();

        // Only normal.txt should be in the manifest
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.files[0].path, "normal.txt");
    }

    #[test]
    fn test_key_file_excluded() {
        let tmpdir = TempDir::new().unwrap();
        let context = tmpdir.path().join("context");
        fs::create_dir(&context).unwrap();

        fs::write(context.join("server.key"), "private key").unwrap();
        fs::write(context.join("main.rs"), "fn main() {}").unwrap();

        let output = tmpdir.path().join("output");
        fs::create_dir(&output).unwrap();

        let manifest = compute_manifest(&context, &output, 1, 1, "test", 1, &[], false).unwrap();

        // server.key is a credential file and must be excluded
        let paths: Vec<&str> = manifest.files.iter().map(|e| e.path.as_str()).collect();
        assert!(
            !paths.contains(&"server.key"),
            "server.key should be excluded"
        );
        assert!(paths.contains(&"main.rs"), "main.rs should be included");
    }

    #[test]
    fn test_user_ignore_patterns_work_alongside_credential_exclusions() {
        let tmpdir = TempDir::new().unwrap();
        let context = tmpdir.path().join("context");
        fs::create_dir(&context).unwrap();

        fs::write(context.join(".env"), "SECRET=value").unwrap();
        fs::write(context.join("build.log"), "log content").unwrap();
        fs::write(context.join("main.rs"), "fn main() {}").unwrap();

        let output = tmpdir.path().join("output");
        fs::create_dir(&output).unwrap();

        // Exclude .log files via user patterns
        let user_patterns = vec!["**/*.log".to_string()];
        let manifest =
            compute_manifest(&context, &output, 1, 1, "test", 1, &user_patterns, false).unwrap();

        // .env excluded by credential patterns, build.log excluded by user pattern
        let paths: Vec<&str> = manifest.files.iter().map(|e| e.path.as_str()).collect();
        assert!(
            !paths.contains(&".env"),
            ".env should be excluded by credential patterns"
        );
        assert!(
            !paths.contains(&"build.log"),
            "build.log should be excluded by user patterns"
        );
        assert!(paths.contains(&"main.rs"), "main.rs should be included");
    }

    #[test]
    fn test_output_dir_excluded() {
        let tmpdir = TempDir::new().unwrap();
        let context = tmpdir.path();
        let output = context.join("rings-output");
        fs::create_dir(&output).unwrap();

        fs::write(context.join("normal.txt"), "content").unwrap();
        fs::write(output.join("state.json"), "{}").unwrap();

        let manifest = compute_manifest(context, &output, 1, 1, "test", 1, &[], false).unwrap();

        // Only normal.txt should be in the manifest
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.files[0].path, "normal.txt");
    }

    #[test]
    fn test_diff_manifests() {
        let before = FileManifest {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            run: 1,
            cycle: 1,
            phase: "test".to_string(),
            iteration: 1,
            root: ".".to_string(),
            files: vec![
                FileEntry {
                    path: "a.txt".to_string(),
                    sha256: "hash1".to_string(),
                    size_bytes: 10,
                    modified: "2024-01-01T00:00:00Z".to_string(),
                },
                FileEntry {
                    path: "b.txt".to_string(),
                    sha256: "hash2".to_string(),
                    size_bytes: 20,
                    modified: "2024-01-01T00:00:00Z".to_string(),
                },
                FileEntry {
                    path: "c.txt".to_string(),
                    sha256: "hash3".to_string(),
                    size_bytes: 30,
                    modified: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };

        let after = FileManifest {
            timestamp: "2024-01-01T00:01:00Z".to_string(),
            run: 2,
            cycle: 1,
            phase: "test".to_string(),
            iteration: 2,
            root: ".".to_string(),
            files: vec![
                FileEntry {
                    path: "a.txt".to_string(),
                    sha256: "hash1_modified".to_string(),
                    size_bytes: 10,
                    modified: "2024-01-01T00:01:00Z".to_string(),
                },
                FileEntry {
                    path: "b.txt".to_string(),
                    sha256: "hash2".to_string(),
                    size_bytes: 20,
                    modified: "2024-01-01T00:00:00Z".to_string(),
                },
                FileEntry {
                    path: "d.txt".to_string(),
                    sha256: "hash4".to_string(),
                    size_bytes: 40,
                    modified: "2024-01-01T00:01:00Z".to_string(),
                },
            ],
        };

        let diff = diff_manifests(&before, &after);

        assert_eq!(diff.modified, vec!["a.txt"]);
        assert_eq!(diff.added, vec!["d.txt"]);
        assert_eq!(diff.deleted, vec!["c.txt"]);
        // 1 modified + 1 added + 1 deleted = 3 total
        assert_eq!(diff.files_changed, 3);
    }

    #[test]
    fn test_diff_unchanged_files_not_included() {
        // b.txt appears in both with the same hash — must not appear in diff
        let before = FileManifest {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            run: 1,
            cycle: 1,
            phase: "test".to_string(),
            iteration: 1,
            root: ".".to_string(),
            files: vec![
                FileEntry {
                    path: "a.txt".to_string(),
                    sha256: "hash1".to_string(),
                    size_bytes: 10,
                    modified: "2024-01-01T00:00:00Z".to_string(),
                },
                FileEntry {
                    path: "b.txt".to_string(),
                    sha256: "unchanged".to_string(),
                    size_bytes: 20,
                    modified: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };

        let after = FileManifest {
            timestamp: "2024-01-01T00:01:00Z".to_string(),
            run: 2,
            cycle: 1,
            phase: "test".to_string(),
            iteration: 2,
            root: ".".to_string(),
            files: vec![
                FileEntry {
                    path: "a.txt".to_string(),
                    sha256: "hash1_changed".to_string(),
                    size_bytes: 10,
                    modified: "2024-01-01T00:01:00Z".to_string(),
                },
                FileEntry {
                    path: "b.txt".to_string(),
                    sha256: "unchanged".to_string(),
                    size_bytes: 20,
                    modified: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };

        let diff = diff_manifests(&before, &after);

        // Only a.txt was changed; b.txt must not appear anywhere
        assert_eq!(diff.modified, vec!["a.txt"]);
        assert!(diff.added.is_empty());
        assert!(diff.deleted.is_empty());
        assert_eq!(diff.files_changed, 1);
    }

    #[test]
    fn test_manifest_roundtrip() {
        let tmpdir = TempDir::new().unwrap();
        let manifest_path = tmpdir.path().join("test.json.gz");

        let original = FileManifest {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            run: 1,
            cycle: 1,
            phase: "test".to_string(),
            iteration: 1,
            root: ".".to_string(),
            files: vec![FileEntry {
                path: "a.txt".to_string(),
                sha256: "abc123".to_string(),
                size_bytes: 100,
                modified: "2024-01-01T00:00:00Z".to_string(),
            }],
        };

        write_manifest_gz(&original, &manifest_path).unwrap();
        let read_back = read_manifest_gz(&manifest_path).unwrap();

        assert_eq!(original.run, read_back.run);
        assert_eq!(original.files.len(), read_back.files.len());
        assert_eq!(original.files[0].path, read_back.files[0].path);
        assert_eq!(original.files[0].sha256, read_back.files[0].sha256);
    }
}
