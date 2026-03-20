# TODO

Implementation tasks, ready to build. The `/build` command picks up the next task from here.

---

## CI: Transition Release Pipeline from Push-Triggered to Cron-Triggered

**Ref:** `.github/workflows/ci.yml`

**Summary:** Every push to `main` currently triggers a full build+release pipeline, creating a version-bump commit (`chore: bump version to vX.Y.Z [skip ci]`) on every code push. This pollutes the git history with mechanical commits. Instead, split CI into two workflows: a push-triggered check-only workflow and a cron-triggered release workflow that wakes up hourly, compares the latest release tag to `HEAD`, and only bumps/builds/releases if there are unreleased code changes.

### Task 5: Test the new workflow split — SKIPPED

**Steps:**
- [x] Push the two-file split to `main`
- [ ] Verify a push triggers only the `ci.yml` check job (no release)
- [ ] Trigger `release.yml` manually via `workflow_dispatch` and confirm it detects unreleased changes, bumps version, builds, and publishes
- [ ] Verify that if no new commits exist since the last `v*` tag, the cron/manual run exits early without bumping or building
- [ ] Verify that multiple code pushes between cron ticks result in a single version bump (not one per push)

**Note:** Remaining steps require manual GitHub Actions verification; skipped by automated builder.

---

## F-057: Cross-Machine Resume Documentation

**Spec:** `specs/state/cancellation-resume.md`

**Summary:** Document that resume requires the workflow file at the same absolute path. When paths don't match, print a clear error suggesting `--parent-run` for cross-machine linking.

### Task 1: Add path mismatch check on resume

**Files:** `src/main.rs` (in `resume_inner`)

**Steps:**
- [x] On resume, compare the current workflow file's absolute path against the path stored in `run.toml`
- [x] If paths differ, print a warning (not error): `⚠  Workflow file path has changed:\n   Saved: {old_path}\n   Current: {new_path}\n   This may cause issues if the workflow structure has also changed.`
- [x] The phase fingerprint check (F-050) already catches structural changes — this is for path-only changes (e.g., moved repo)
- [x] If the path is different but fingerprint matches, proceed with warning only

**Tests:**
- [x] Resume with same path: no warning
- [x] Resume with different path but same fingerprint: warning but proceeds
- [x] `just validate` clean

---

## F-122: Cycle Snapshots

**Spec:** `specs/observability/file-lineage.md`

**Summary:** Copy the entire `context_dir` at each cycle boundary so the user can roll back to any prior cycle. Opt-in via `snapshot_cycles = true` in workflow config.

### Task 1: Add cycle snapshot support

**Files:** `src/workflow.rs`, `src/engine.rs`

**Steps:**
- [ ] Add `snapshot_cycles: bool` field (with `#[serde(default)]`) to workflow config
- [ ] At each cycle boundary (after all phases in a cycle complete), if `snapshot_cycles = true`:
  1. Create directory `{output_dir}/snapshots/cycle-{N}/`
  2. Copy all files from `context_dir` to the snapshot directory (respecting manifest_ignore patterns)
- [ ] Print snapshot info: `📸  Snapshot saved: {path} ({size})`
- [ ] Skip credential files from snapshots (reuse F-120 exclusion patterns)

**Tests:**
- [ ] `snapshot_cycles = true` creates snapshot directories at cycle boundaries
- [ ] Snapshot contains all context_dir files except ignored/credential patterns
- [ ] `snapshot_cycles = false` (default) creates no snapshots
- [ ] `just validate` clean

---

## F-123: Snapshot Storage Warning

**Spec:** `specs/observability/file-lineage.md`

**Summary:** When `snapshot_cycles = true`, estimate storage usage at startup and warn if it will be unexpectedly large (> 100 MB per snapshot × max_cycles).

### Task 1: Add snapshot storage estimate

**Files:** `src/main.rs` (or `src/engine.rs`)

**Steps:**
- [ ] When `snapshot_cycles = true`, at startup: compute the total size of context_dir (excluding ignored files)
- [ ] Estimate total snapshot storage: `size_per_snapshot × max_cycles`
- [ ] If estimate > 100 MB: print warning `⚠  Cycle snapshots enabled. Estimated storage: {size} ({per_snapshot} × {max_cycles} cycles).\n   Consider reducing max_cycles or using manifest_ignore to exclude large directories.`
- [ ] On TTY: prompt `Continue? [Y/n]` — on non-TTY: proceed with warning only

**Tests:**
- [ ] Large context_dir with many cycles triggers storage warning
- [ ] Small context_dir produces no warning
- [ ] `snapshot_cycles = false` skips the check entirely
- [ ] `just validate` clean

---

## F-124: Manifest Compression

**Spec:** `specs/observability/file-lineage.md`

**Summary:** Store file manifests as gzip-compressed JSON to keep disk usage low. Manifests are written to `manifests/<run-number>-after.json.gz`.

### Task 1: Add gzip compression to manifest storage

**Files:** `src/manifest.rs`

**Steps:**
- [ ] When writing manifests, use `flate2::write::GzEncoder` to compress the JSON before writing
- [ ] Add `flate2` to `Cargo.toml` dependencies (with `gzip` feature)
- [ ] Write to `.json.gz` extension instead of `.json`
- [ ] When reading manifests (for diff computation, inspect views), detect `.json.gz` and decompress with `flate2::read::GzDecoder`
- [ ] Handle backwards compatibility: if a `.json` file exists (old format), read it uncompressed

**Tests:**
- [ ] Written manifest file has `.json.gz` extension
- [ ] Compressed manifest is valid gzip (can be decompressed with `gunzip`)
- [ ] Reading compressed manifest produces correct data
- [ ] Reading old uncompressed `.json` manifest still works
- [ ] Compressed size is significantly smaller than uncompressed
- [ ] `just validate` clean

---

## F-178: Shell Completions Behavior

**Spec:** `specs/cli/completion-and-manpage.md`

**Summary:** Tab-completion offers `.toml` files for workflow arguments, run IDs for resume/show/inspect arguments, and flag names everywhere. Requires clap_complete's custom completer support.

### Task 1: Add custom completers for arguments

**Files:** `src/cli.rs`, `src/main.rs`

**Steps:**
- [ ] For `<WORKFLOW>` argument in `rings run`: add a custom completer that suggests `.rings.toml` and `.toml` files in the current directory
- [ ] For `<RUN_ID>` arguments in `rings resume`, `rings show`, `rings inspect`, `rings lineage`: add a custom completer that lists run IDs from the output directory
- [ ] Use `clap_complete::engine::ArgValueCompleter` or shell-specific completion scripts
- [ ] Test with `rings completions zsh` and verify completions work in zsh

**Tests:**
- [ ] Generated completion script contains workflow file completion logic
- [ ] Generated completion script contains run ID completion logic
- [ ] `just validate` clean

---

## F-162: OpenTelemetry Opt-In

**Spec:** `specs/observability/opentelemetry.md`

**Summary:** Add opt-in OpenTelemetry tracing, controlled by `RINGS_OTEL_ENABLED=1` environment variable. When enabled, rings exports traces to an OTLP-compatible collector. When disabled (default), no tracing overhead.

### Task 1: Add OTel initialization

**Files:** `src/otel.rs` (new), `src/lib.rs`, `src/engine.rs`, `Cargo.toml`

**Steps:**
- [ ] Add `opentelemetry`, `opentelemetry-otlp`, and `tracing-opentelemetry` to `Cargo.toml` as optional dependencies behind an `otel` feature flag
- [ ] Create `src/otel.rs` with `init_tracer() -> Result<Option<SdkTracerProvider>>`:
  1. Check `RINGS_OTEL_ENABLED` env var — if not set or "0", return `None` (no-op)
  2. Read `OTEL_EXPORTER_OTLP_ENDPOINT` for collector endpoint (F-170)
  3. Initialize OTLP exporter and tracer provider
  4. If init fails, print warning and continue with no-op tracer (F-169)
- [ ] Register `pub mod otel;` in `src/lib.rs`
- [ ] In engine startup: call `init_tracer()`, store the provider for shutdown at exit
- [ ] On exit: call `provider.shutdown()` to flush remaining spans

**Tests:**
- [ ] `RINGS_OTEL_ENABLED=0`: no tracer initialized, no overhead
- [ ] `RINGS_OTEL_ENABLED=1` with no endpoint: warning printed, continues with no-op
- [ ] Feature flag `otel` controls compilation of dependencies
- [ ] `just validate` clean (with and without `otel` feature)

---
