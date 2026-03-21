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

## F-166: OTel Span Links for Resumed Runs

**Spec:** `specs/observability/opentelemetry.md`, `specs/state/run-ancestry.md`

**Summary:** When resuming a run, link the new trace to the parent run's trace so observability tools can navigate the full history across resume boundaries.

### Task 1: Add span links on resume

**Files:** `src/telemetry.rs`, `src/engine.rs`, `src/state.rs`, `src/main.rs`

**Steps:**
- [x] When a run is resumed (has `parent_run_id` or `continuation_of`), add a span link from the root span to the parent run's trace
- [x] Store the parent run's trace ID in `run.toml` so it can be referenced
- [x] If the parent trace ID is not available (old run without OTel), skip the link gracefully

**Tests:**
- [x] Resumed run's root span has a link to the parent run's trace
- [x] Fresh run (no parent) has no span links
- [x] Missing parent trace ID is handled gracefully
- [x] `just validate` clean

---

## F-167/F-168/F-169/F-170: OTel Metrics, Path Stripping, Init Failure, Endpoint Config

**Spec:** `specs/observability/opentelemetry.md`

**Summary:** Remaining OTel features: emit cost/duration/token metrics (F-167), strip filesystem paths from telemetry for privacy (F-168), handle init failures gracefully (F-169, likely already done), and configure endpoint via standard env var (F-170, likely already done).

### Task 1: Add OTel metrics

**Files:** `src/otel.rs`

**Steps:**
- [ ] When OTel is enabled, create a meter provider alongside the tracer
- [ ] Emit counters: `rings.runs.total`, `rings.cycles.total`
- [ ] Emit histograms: `rings.run.cost_usd`, `rings.run.duration_secs`, `rings.run.input_tokens`, `rings.run.output_tokens`
- [ ] Record metrics after each run completes

**Tests:**
- [ ] Metrics are recorded when OTel is enabled
- [ ] OTel disabled: no metrics overhead
- [ ] `just validate` clean

---

### Task 2: Add path stripping option

**Files:** `src/otel.rs`

**Steps:**
- [ ] Check `RINGS_OTEL_STRIP_PATHS` env var
- [ ] When set to "1": replace all filesystem paths in span attributes with `[redacted]` or just the filename
- [ ] Applies to: `workflow` path, `context_dir`, `output_dir`, file paths in manifest diffs

**Tests:**
- [ ] `RINGS_OTEL_STRIP_PATHS=1`: paths are redacted in span attributes
- [ ] Without the var: full paths are preserved
- [ ] `just validate` clean

---

### Task 3: Verify init failure handling and endpoint config

**Files:** `src/otel.rs`

**Steps:**
- [ ] Verify F-169: if OTel init fails (bad endpoint, network error), rings prints a warning and continues with no-op tracer
- [ ] Verify F-170: `OTEL_EXPORTER_OTLP_ENDPOINT` is read for the collector endpoint
- [ ] If already working (likely done in F-162), mark as COMPLETE

**Tests:**
- [ ] Invalid endpoint URL: warning printed, rings continues normally
- [ ] `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317` is used as the endpoint
- [ ] `just validate` clean

---

---
