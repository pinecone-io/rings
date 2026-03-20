# TODO

Implementation tasks, ready to build. The `/build` command picks up the next task from here.

---

## CI: Transition Release Pipeline from Push-Triggered to Cron-Triggered

**Ref:** `.github/workflows/ci.yml`

**Summary:** Every push to `main` currently triggers a full build+release pipeline, creating a version-bump commit (`chore: bump version to vX.Y.Z [skip ci]`) on every code push. This pollutes the git history with mechanical commits. Instead, split CI into two workflows: a push-triggered check-only workflow and a cron-triggered release workflow that wakes up hourly, compares the latest release tag to `HEAD`, and only bumps/builds/releases if there are unreleased code changes.

### Task 1: Split `ci.yml` into check-only and release workflows

**Files:** `.github/workflows/ci.yml`, `.github/workflows/release.yml` (new)

**Steps:**
- [x] Strip the `changes`, `bump`, `build`, and `release` jobs from `ci.yml`, leaving only the `check` job
- [x] Remove the `workflow_dispatch` trigger from `ci.yml` (it's only useful for manual releases)
- [x] Create `.github/workflows/release.yml` containing the `changes`, `bump`, `build`, and `release` jobs (copied from current `ci.yml`)
- [x] In `release.yml`, add a `check` job as the first step (same as current `ci.yml` check job) so releases are never built from code that doesn't pass CI

### Task 2: Configure cron + workflow_dispatch triggers on `release.yml`

**Files:** `.github/workflows/release.yml`

**Steps:**
- [x] Set triggers to `schedule: [{cron: '0 * * * *'}]` (every hour on the hour) and `workflow_dispatch`
- [x] Do not include `push` or `pull_request` triggers
- [x] Confirm that `[skip ci]` in bump commits only suppresses `push`-triggered runs, not `schedule`-triggered ones (this is the documented GitHub Actions behavior)

### Task 3: Replace change detection with tag-based diff

**Files:** `.github/workflows/release.yml`

**Steps:**
- [x] In the `changes` job, replace `fetch-depth: 2` with `fetch-depth: 0` so all tags and history are available
- [x] Replace the `HEAD~1` vs `HEAD` diff with a tag-based comparison:
  1. Find the latest `v*` tag: `LATEST_TAG=$(git describe --tags --match 'v*' --abbrev=0 2>/dev/null || echo '')`
  2. If no tag exists, set `should_release=true` (first release)
  3. If a tag exists, diff `$LATEST_TAG..HEAD` and filter out docs-only changes (same grep pattern as current)
  4. If code changes exist between the tag and HEAD, set `should_release=true`; otherwise `should_release=false`
- [x] Keep the `workflow_dispatch` override that always sets `should_release=true`

### Task 4: Clean up `ci.yml`

**Files:** `.github/workflows/ci.yml`

**Steps:**
- [x] Remove the `if: github.ref == 'refs/heads/main'` condition that was only relevant for gating release jobs
- [x] Verify the `check` job still runs on both `push` and `pull_request` to `main`
- [x] Verify no leftover `needs:` references to removed jobs

### Task 5: Test the new workflow split

**Steps:**
- [ ] Push the two-file split to `main`
- [ ] Verify a push triggers only the `ci.yml` check job (no release)
- [ ] Trigger `release.yml` manually via `workflow_dispatch` and confirm it detects unreleased changes, bumps version, builds, and publishes
- [ ] Verify that if no new commits exist since the last `v*` tag, the cron/manual run exits early without bumping or building
- [ ] Verify that multiple code pushes between cron ticks result in a single version bump (not one per push)

---

## Bug: JSONL Summary Event Missing on Executor Wait Error

**Ref:** `specs/observability/runtime-output.md` (F-139, F-140)

**Summary:** In `engine.rs` line 1124, if `handle.try_wait()` returns an `Err` (I/O error polling the subprocess), the function returns `Err(e)` directly without emitting a `SummaryEvent`. This breaks the JSONL event contract: every `StartEvent` should eventually be followed by a `SummaryEvent`. The error propagates to `cmd_run` in `main.rs` which emits a `FatalErrorEvent` with `run_id: None` (line 79-80), losing the run correlation. While rare (requires an OS-level process polling failure), JSONL consumers relying on the start/summary pairing will hang or error.

### Task 1: Emit summary before returning executor wait error

**Files:** `src/engine.rs`, `src/main.rs`

**Steps:**
- [ ] At line 1122-1124 in `engine.rs`, before returning `Err(e)`, emit a `SummaryEvent` via `emit_summary_if_jsonl` with status `"executor_error"` and the current run context
- [ ] Alternatively, catch the error in `run_workflow`, emit the summary, then re-return the error
- [ ] In `main.rs` lines 77-86 (cmd_run error handler) and lines 540-551 (cmd_resume error handler): pass the `run_id` to `FatalErrorEvent::new(Some(run_id), ...)` instead of `None` — this requires extracting and storing the run_id before calling `run_workflow`

**Tests:**
- [ ] A mock executor whose `try_wait()` returns `Err`: verify JSONL output contains both `StartEvent` and `SummaryEvent`
- [ ] Verify `FatalErrorEvent` includes the correct `run_id` (not null)
- [ ] `just validate` clean

---

## Bug: Workflow `budget_cap_usd = nan` Bypasses Budget Cap Validation

**Ref:** `specs/observability/cost-tracking.md`, `specs/workflow/workflow-file-format.md`

**Summary:** TOML 1.0 supports `nan`, `inf`, `+inf`, and `-inf` as float literals. The workflow validation in `src/workflow.rs` line 347-350 checks `if cap <= 0.0` to reject invalid budget caps, but `NaN <= 0.0` evaluates to `false` in IEEE 754 (all NaN comparisons return false). This means `budget_cap_usd = nan` passes validation. In the engine, `cumulative_cost >= NaN` is also always false, so the budget cap never triggers — the workflow runs with no spending limit despite having one configured. Similarly, `budget_cap_usd = inf` passes validation (inf > 0.0 is true) but provides no actual protection. The same issue exists for per-phase `budget_cap_usd` at line 406-410.

### Task 1: Reject NaN and Infinity in budget cap validation

**Files:** `src/workflow.rs`

**Steps:**
- [ ] In the global `budget_cap_usd` validation (line 347-350), add checks: `if cap.is_nan() || cap.is_infinite() || cap <= 0.0`
- [ ] In the per-phase `budget_cap_usd` validation (line 406-410), add the same NaN/Infinity checks
- [ ] Use a clear error message: `budget_cap_usd must be a finite positive number`

**Tests:**
- [ ] `budget_cap_usd = nan` in TOML is rejected at parse time
- [ ] `budget_cap_usd = inf` in TOML is rejected at parse time
- [ ] `budget_cap_usd = 10.0` still works (positive finite is valid)
- [ ] Per-phase `budget_cap_usd = nan` is also rejected
- [ ] `just validate` clean

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
