# TODO

Implementation tasks, ready to build. The `/build` command picks up the next task from here.

---

## Tech Debt: Remove `unwrap()`/`expect()` from Production Code

**Ref:** CLAUDE.md rule — "No `unwrap()` or `expect()` in production code — all errors propagate via `?` and `anyhow`"

**Summary:** Audit found two `unwrap()`/`expect()` calls in production code paths that could cause hard panics instead of graceful errors.

### Task 1: Replace `.expect()` in Ctrl+C handler setup

**Files:** `src/main.rs`

**Steps:**
- [x] Replace `.expect("Failed to install Ctrl+C handler")` on line 43 with proper error handling
- [x] Since `main()` currently returns `()`, either: (a) convert main to return `Result<()>` via `process::exit` wrapper, or (b) use an `if let Err(e)` block that prints the error to stderr and exits with code 2
- [x] Verify that failure to install the handler produces a clear user-facing error message, not a panic backtrace

**Tests:**
- [x] Existing tests continue to pass (`just validate`)

---

### Task 2: Replace `.unwrap()` in dry-run phase position lookup

**Files:** `src/main.rs`

**Steps:**
- [x] Replace `.unwrap()` on line 170 (phase position lookup in dry-run output) with `.unwrap_or(0)` or a safe fallback that cannot panic
- [x] The current code iterates `plan.phases` and looks up each phase's index by name within the same collection — logically infallible, but should still be defended

**Tests:**
- [x] Existing dry-run tests continue to pass
- [x] `just validate` clean

---

## Tech Debt: Harden `costs.jsonl` Append Against Partial Writes

**Ref:** `specs/observability/audit-logs.md`

**Summary:** `append_cost_entry()` in `src/audit.rs` opens the file in append mode and writes a JSON line. If the process is killed mid-write (e.g., SIGKILL, OOM kill), the file can be left with a truncated JSON line. On resume, `recover_last_run_from_costs()` already skips malformed lines, but a partial line could still corrupt the next append if it doesn't end with a newline.

### Task 1: Atomic-ish cost entry append

**Files:** `src/audit.rs`

**Steps:**
- [ ] Serialize the full line (JSON + newline) to a `String` first (already done)
- [ ] Write the entire serialized bytes in a single `write_all()` call instead of `writeln!()` (which may split the write into data + newline)
- [ ] Call `file.sync_data()` after the write to flush to disk before returning
- [ ] Add a recovery safeguard: when reading `costs.jsonl` for resume, if the last line does not end with `\n`, truncate the file to remove the partial line before appending

**Tests:**
- [ ] Existing cost parsing and state recovery tests continue to pass
- [ ] Test that a costs.jsonl with a truncated last line (no trailing newline) is handled gracefully on read
- [ ] `just validate` clean

---

## Tech Debt: Validate Parsed Cost Values Are Non-Negative

**Ref:** `specs/observability/cost-tracking.md`, `specs/execution/output-parsing.md`

**Summary:** `parse_cost_from_output()` in `src/cost.rs` accepts any dollar amount matched by the regex, including negative values. A malformed or adversarial executor output like `Cost: $-10.00` would parse as `cost_usd: Some(-10.0)`, which would subtract from cumulative cost and could allow budget cap bypass.

### Task 1: Add non-negative validation to cost parser

**Files:** `src/cost.rs`

**Steps:**
- [ ] After extracting `cost_usd` from any regex match, clamp or reject negative values: if `cost < 0.0`, treat as `ParseConfidence::None` with `cost_usd: None`
- [ ] Also reject `NaN` and `Infinity` values (defense in depth against malformed f64 parsing)
- [ ] Log a warning when a negative/invalid cost is encountered (similar to low-confidence warning)

**Tests:**
- [ ] `parse_cost_from_output("Cost: $-10.00 ...")` returns confidence `None`, cost `None`
- [ ] `parse_cost_from_output("Cost: $0.00 ...")` still works (zero is valid)
- [ ] Existing cost parsing tests continue to pass
- [ ] `just validate` clean

---

## Bug: Timeout Deadline Not Reset After Quota Backoff Retry

**Ref:** `specs/execution/engine.md`, `specs/execution/rate-limiting.md`

**Summary:** In `engine.rs`, the per-run timeout deadline is computed from `run_start` (line 874), which is set once before the retry loop. After a quota backoff wait (which could be 300+ seconds), the retry re-enters the loop but the deadline is already expired, causing the retry to immediately timeout without executing. This effectively breaks `timeout_per_run_secs` when combined with quota backoff.

### Task 1: Reset timeout deadline on retry

**Files:** `src/engine.rs`

**Steps:**
- [ ] Move `run_start = std::time::Instant::now()` inside the `'retry_loop` (before line 940), so each retry attempt gets a fresh timeout deadline
- [ ] Alternatively, recompute `timeout_deadline` at the top of each retry iteration (after the `continue 'retry_loop` at line 1190 re-enters)
- [ ] Ensure `elapsed_secs` at line 1198 still reflects total wall-clock time (for display purposes), so keep the original `run_start` for display and add a separate `attempt_start` for timeout

**Tests:**
- [ ] A run with `timeout_per_run_secs = 10` and quota backoff delay of 5s: retry attempt gets a fresh 10s timeout, not an expired one
- [ ] A run with no timeout: retry behavior unchanged
- [ ] A run with timeout and no retries: timeout still fires correctly
- [ ] `just validate` clean

---

## Bug: Cost Entry Written Too Late in Success Path — Crash Window

**Ref:** `specs/observability/audit-logs.md`, `specs/state/cancellation-resume.md`

**Summary:** In the success path of `engine.rs`, state.json is written at line 1503 (including cumulative cost), but `costs.jsonl` is not appended until line 1594 — after manifest computation (lines 1515-1549), contract checks (1551-1574), and JSONL event emission (1576-1591). If the process crashes in this ~90-line window, the cost for that run is lost from `costs.jsonl`. On resume, cost reconstruction from `costs.jsonl` (lines 627-653) will produce a lower cumulative cost than reality. The error path (lines 1459-1478) does this correctly — cost is appended immediately after state.

Additionally, the resume cost reconstruction at lines 627-653 does not deduplicate entries by run number. If a cost entry was appended to `costs.jsonl` but the subsequent state write failed (e.g., disk full), the run would be re-executed on resume, creating a duplicate cost entry that gets double-counted.

### Task 1: Move cost append immediately after state write

**Files:** `src/engine.rs`

**Steps:**
- [ ] Move the `append_cost_entry()` call (currently at line 1594-1612) to immediately after `state.write_atomic(&state_path)?` (line 1503), before manifest computation
- [ ] The cost entry will need `files_added/modified/deleted/changed` set to 0 initially, then updated if manifest info is computed later — OR collect manifest data before writing cost (less desirable since it widens the state-before-costs gap)
- [ ] Simpler approach: accept that the cost entry written early won't have file counts, and emit a separate `manifest_diff` entry or update later — OR split the cost entry write to happen early (with cost data) and keep file counts in the JSONL event only
- [ ] Verify the error path (lines 1459-1478) still follows the same pattern

### Task 2: Deduplicate cost entries on resume reconstruction

**Files:** `src/engine.rs`

**Steps:**
- [ ] In the resume cost reconstruction loop (lines 627-653), track seen run numbers in a `HashSet<u32>`
- [ ] If a run number has already been seen, skip the duplicate entry (use the first occurrence)
- [ ] This handles the edge case where cost was appended but state write failed, causing a re-execution that appends a second entry for the same run

**Tests:**
- [ ] Resume with a costs.jsonl containing duplicate run entries: cumulative cost counts each run only once
- [ ] Resume with clean costs.jsonl: behavior unchanged
- [ ] Cost entry is appended immediately after state write (no crash window for manifest computation)
- [ ] `just validate` clean

---

## CI: Transition Release Pipeline from Push-Triggered to Cron-Triggered

**Ref:** `.github/workflows/ci.yml`

**Summary:** Every push to `main` currently triggers a full build+release pipeline, creating a version-bump commit (`chore: bump version to vX.Y.Z [skip ci]`) on every code push. This pollutes the git history with mechanical commits. Instead, split CI into two workflows: a push-triggered check-only workflow and a cron-triggered release workflow that wakes up hourly, compares the latest release tag to `HEAD`, and only bumps/builds/releases if there are unreleased code changes.

### Task 1: Split `ci.yml` into check-only and release workflows

**Files:** `.github/workflows/ci.yml`, `.github/workflows/release.yml` (new)

**Steps:**
- [ ] Strip the `changes`, `bump`, `build`, and `release` jobs from `ci.yml`, leaving only the `check` job
- [ ] Remove the `workflow_dispatch` trigger from `ci.yml` (it's only useful for manual releases)
- [ ] Create `.github/workflows/release.yml` containing the `changes`, `bump`, `build`, and `release` jobs (copied from current `ci.yml`)
- [ ] In `release.yml`, add a `check` job as the first step (same as current `ci.yml` check job) so releases are never built from code that doesn't pass CI

### Task 2: Configure cron + workflow_dispatch triggers on `release.yml`

**Files:** `.github/workflows/release.yml`

**Steps:**
- [ ] Set triggers to `schedule: [{cron: '0 * * * *'}]` (every hour on the hour) and `workflow_dispatch`
- [ ] Do not include `push` or `pull_request` triggers
- [ ] Confirm that `[skip ci]` in bump commits only suppresses `push`-triggered runs, not `schedule`-triggered ones (this is the documented GitHub Actions behavior)

### Task 3: Replace change detection with tag-based diff

**Files:** `.github/workflows/release.yml`

**Steps:**
- [ ] In the `changes` job, replace `fetch-depth: 2` with `fetch-depth: 0` so all tags and history are available
- [ ] Replace the `HEAD~1` vs `HEAD` diff with a tag-based comparison:
  1. Find the latest `v*` tag: `LATEST_TAG=$(git describe --tags --match 'v*' --abbrev=0 2>/dev/null || echo '')`
  2. If no tag exists, set `should_release=true` (first release)
  3. If a tag exists, diff `$LATEST_TAG..HEAD` and filter out docs-only changes (same grep pattern as current)
  4. If code changes exist between the tag and HEAD, set `should_release=true`; otherwise `should_release=false`
- [ ] Keep the `workflow_dispatch` override that always sets `should_release=true`

### Task 4: Clean up `ci.yml`

**Files:** `.github/workflows/ci.yml`

**Steps:**
- [ ] Remove the `if: github.ref == 'refs/heads/main'` condition that was only relevant for gating release jobs
- [ ] Verify the `check` job still runs on both `push` and `pull_request` to `main`
- [ ] Verify no leftover `needs:` references to removed jobs

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
