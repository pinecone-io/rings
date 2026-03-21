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

## F-123: Snapshot Storage Warning

**Spec:** `specs/observability/file-lineage.md`

**Summary:** When `snapshot_cycles = true`, estimate storage usage at startup and warn if it will be unexpectedly large (> 100 MB per snapshot × max_cycles).

### Task 1: Add snapshot storage estimate

**Files:** `src/main.rs` (or `src/engine.rs`)

**Steps:**
- [x] When `snapshot_cycles = true`, at startup: compute the total size of context_dir (excluding ignored files)
- [x] Estimate total snapshot storage: `size_per_snapshot × max_cycles`
- [x] If estimate > 100 MB: print warning `⚠  Cycle snapshots enabled. Estimated storage: {size} ({per_snapshot} × {max_cycles} cycles).\n   Consider reducing max_cycles or using manifest_ignore to exclude large directories.`
- [x] On TTY: prompt `Continue? [Y/n]` — on non-TTY: proceed with warning only

**Tests:**
- [x] Large context_dir with many cycles triggers storage warning
- [x] Small context_dir produces no warning
- [x] `snapshot_cycles = false` skips the check entirely
- [x] `just validate` clean

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
