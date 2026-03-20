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

## Bug: Workflow `budget_cap_usd = nan` Bypasses Budget Cap Validation

**Ref:** `specs/observability/cost-tracking.md`, `specs/workflow/workflow-file-format.md`

**Summary:** TOML 1.0 supports `nan`, `inf`, `+inf`, and `-inf` as float literals. The workflow validation in `src/workflow.rs` line 347-350 checks `if cap <= 0.0` to reject invalid budget caps, but `NaN <= 0.0` evaluates to `false` in IEEE 754 (all NaN comparisons return false). This means `budget_cap_usd = nan` passes validation.

### Task 1: Reject NaN and Infinity in budget cap validation

**Files:** `src/workflow.rs`

**Steps:**
- [x] In the global `budget_cap_usd` validation, add checks: `if cap.is_nan() || cap.is_infinite() || cap <= 0.0`
- [x] In the per-phase `budget_cap_usd` validation, add the same NaN/Infinity checks
- [x] Use a clear error message: `budget_cap_usd must be a finite positive number`

**Tests:**
- [x] `budget_cap_usd = nan` in TOML is rejected at parse time
- [x] `budget_cap_usd = inf` in TOML is rejected at parse time
- [x] `budget_cap_usd = 10.0` still works (positive finite is valid)
- [x] Per-phase `budget_cap_usd = nan` is also rejected
- [x] `just validate` clean

---

## Bug: Executor Output Reader Drops All Data After First Non-UTF8 Line

**Ref:** `specs/execution/executor-integration.md`

**Summary:** In `executor.rs` lines 231 and 246, the stdout/stderr reader threads use `reader.lines().map_while(Result::ok)` to iterate over output lines. `map_while` stops iteration on the first `Err` — meaning if one line contains invalid UTF-8 bytes, ALL subsequent lines are silently dropped. This is catastrophic for reliability: cost data, completion signals, response text, and resume commands appearing after the bad line are all lost. The workflow would continue running (no completion signal detected) and cost tracking would be incorrect. While Claude Code always outputs valid UTF-8, custom executors (F-022) may not, and even a single corrupted byte from I/O issues would trigger complete data loss.

### Task 1: Replace `map_while` with `filter_map` in reader threads

**Files:** `src/executor.rs`

**Steps:**
- [ ] On line 231, change `reader.lines().map_while(Result::ok)` to `reader.lines().filter_map(Result::ok)`
- [ ] On line 246, make the same change for the stderr reader thread
- [ ] `filter_map(Result::ok)` skips individual bad lines but continues processing subsequent lines, preserving all valid output after the error
- [ ] Optionally: log a warning to stderr when a line is skipped due to UTF-8 decode failure (helps users debug custom executor issues)

**Tests:**
- [ ] Existing verbose rendering and output accumulation tests continue to pass
- [ ] `just validate` clean

---
