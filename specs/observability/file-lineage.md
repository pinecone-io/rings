# File Lineage and Artifact Tracking

## Overview

rings operates on a `context_dir` where phases read and write files. Without tracking what changes between runs, the audit log captures only what Claude said — not what actually happened to the codebase. File lineage closes that gap by recording a content fingerprint of `context_dir` before and after every run.

This enables:
- **Reproducibility auditing**: know exactly what state each run operated on
- **Change attribution**: trace which phase/cycle produced a given file state
- **Diff-based debugging**: understand why a run behaved differently from a similar one
- **Cycle snapshots**: full rollback to any cycle boundary

## File Manifest

After each run completes, rings computes a manifest of `context_dir`:

```json
{
  "timestamp": "2024-03-15T14:32:07Z",
  "run": 7,
  "cycle": 2,
  "phase": "builder",
  "iteration": 2,
  "root": "./src",
  "files": [
    { "path": "src/main.rs",      "sha256": "a3f1...", "size_bytes": 4821, "modified": "2024-03-15T14:32:05Z" },
    { "path": "src/engine.rs",    "sha256": "b9c2...", "size_bytes": 2103, "modified": "2024-03-15T14:32:06Z" },
    { "path": "tests/engine.rs",  "sha256": "cc41...", "size_bytes": 891,  "modified": "2024-03-15T14:31:58Z" }
  ]
}
```

Manifests are written to `output_dir/<run-id>/manifests/<run-number>-after.json.gz` (gzip-compressed JSON).

A manifest is also captured **before** the first run as `manifests/000-before.json.gz` (the initial state of `context_dir` when rings started).

## Diff Detection

By comparing consecutive manifests, rings records what changed in each run:

```json
{
  "run": 7,
  "phase": "builder",
  "cycle": 2,
  "changes": {
    "added":    ["src/new_module.rs"],
    "modified": ["src/main.rs", "src/engine.rs"],
    "deleted":  []
  },
  "files_changed": 2
}
```

This diff is:
- Appended to `costs.jsonl` (as part of the `run_end` record)
- Included as a `run_end` JSONL event field (`"files_changed"`, `"added"`, `"modified"`, `"deleted"`)
- Stored as the `rings.files_changed` span attribute in OTel

## Manifest Configuration

By default, file manifests are enabled. Control via workflow TOML:

```toml
[workflow]
completion_signal = "DONE"
context_dir = "./src"

# File manifest behavior (all optional, defaults shown)
manifest_enabled = true        # set false to disable entirely
manifest_ignore = [            # glob patterns to exclude from manifests
  "**/.git/**",
  "**/target/**",
  "**/__pycache__/**",
  "**/*.pyc",
  # Default credential/secret file exclusions (always applied, even if manifest_ignore is overridden)
  # These are listed here for documentation; they cannot be removed via manifest_ignore.
  # "**/.env",
  # "**/.env.*",
  # "**/*_rsa",
  # "**/*_ed25519",
  # "**/*.pem",
  # "**/*.key",
]
snapshot_cycles = false        # set true to copy context_dir at each cycle boundary
```

rings always excludes `.git/` and the rings output directory itself from manifests.

## Cycle Snapshots

When `snapshot_cycles = true`, rings copies the full `context_dir` into `output_dir/<run-id>/snapshots/` at the end of each cycle:

```
output_dir/<run-id>/
  snapshots/
    cycle-000-before/     # state before any run started
      src/main.rs
      src/engine.rs
      ...
    cycle-001-after/      # state after cycle 1 completed
      ...
    cycle-002-after/      # state after cycle 2 completed
      ...
```

Snapshots are a full copy of the files matching `context_dir` (excluding `manifest_ignore` patterns). They are not diffs — they are complete copies for easy inspection and rollback.

**Warning at startup:** If `snapshot_cycles = true` and `context_dir` is large, rings estimates the total snapshot storage requirement and warns the user:

```
⚠  snapshot_cycles is enabled. Estimated storage: ~47 MB per cycle × 50 cycles = ~2.3 GB.
   Continue? [y/N]:
```

If stdin is not a TTY (CI/pipeline context), this prompt is skipped and rings proceeds with snapshots enabled. Use `snapshot_cycles = false` in the workflow file to disable explicitly in automated contexts.

## Manifest Storage Format

Manifests are stored as gzip-compressed JSON to reduce storage overhead:

- `manifests/000-before.json.gz`
- `manifests/001-after.json.gz`
- `manifests/002-after.json.gz`
- ...

The `rings inspect` command handles decompression transparently.

## File Lineage in JSONL Events

When `--output-format jsonl`, the `run_end` event includes file diff data:

```jsonl
{"event":"run_end","run":7,"cycle":2,"phase":"builder","iteration":2,
 "cost_usd":0.023,"exit_code":0,
 "files_added":["src/new_module.rs"],"files_modified":["src/main.rs"],"files_deleted":[],
 "files_changed":2,"timestamp":"..."}
```

## Performance

Manifest computation is done after the `claude` subprocess exits, not during. For a typical Rust project (hundreds of files), SHA256 manifest generation takes < 100ms and does not meaningfully extend cycle time.

**mtime optimization:** rings uses file modification time (`mtime`) as a fast pre-filter before computing SHA256. If a file's `mtime` is unchanged from the previous manifest, its SHA256 is reused without re-reading the file. This makes manifest computation nearly instantaneous for runs where Claude Code changed few files, and O(changed files) rather than O(all files).

**Security note:** mtime-based optimization means manifests trust the filesystem's modification timestamps. On most developer workstations this is appropriate. In adversarial environments where mtime can be forged, set `manifest_mtime_optimization = false` in the workflow to force full SHA256 recomputation on every run.

If `context_dir` is extremely large (> 10,000 files), rings warns at startup and recommends using `manifest_ignore` to narrow scope.

## Credential File Protection

rings always excludes the following patterns from manifests, regardless of `manifest_ignore` configuration:

```
**/.env
**/.env.*
**/*_rsa
**/*_ed25519
**/*.pem
**/*.key
**/.netrc
**/*.pfx
**/*.p12
```

These files commonly contain secrets and should never be captured in audit logs. This exclusion is hardcoded and cannot be overridden. If you genuinely need to track changes to key files, use a naming convention that doesn't match these patterns.
