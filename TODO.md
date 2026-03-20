# TODO

Implementation tasks, ready to build. The `/build` command picks up the next task from here.

---

## F-109 + F-110: Output Directory Hardening

**Spec:** `specs/observability/audit-logs.md`

**Summary:** Two small security improvements: (1) create output directories with mode 0700 so only the owner can read run logs, and (2) reject `output_dir` values containing `..` to prevent path traversal.

### Task 1: Restricted directory permissions

**Files:** `src/main.rs` (or wherever `create_dir_all` is called for the output directory)

**Steps:**
- [x] Find all calls to `std::fs::create_dir_all` for the output/run directory
- [x] On Unix: after creating the directory, set permissions to 0700 using `std::fs::set_permissions` with `std::os::unix::fs::PermissionsExt`
- [x] Use `#[cfg(unix)]` guard — on non-Unix platforms, skip the permission change (document this limitation)
- [x] Ensure the permission is set on the run-specific directory, not the parent `~/.local/share/rings/runs/`

**Tests:**
- [x] Created run directory has mode 0700 on Unix
- [x] Parent directory permissions are not changed
- [x] Non-Unix builds compile without error (cfg guard works)

---

### Task 2: Path traversal protection

**Files:** `src/main.rs` or `src/workflow.rs`

**Steps:**
- [ ] Before using any `output_dir` value (from CLI `--output-dir` or workflow TOML), check if the path contains `..` components
- [ ] If `..` is found: print `Error: output_dir must not contain '..' components` and exit 2
- [ ] Apply the check in both `run_inner` (for `--output-dir` flag) and workflow parsing (for TOML `output_dir`)
- [ ] Use `std::path::Path::components()` and check for `Component::ParentDir`

**Tests:**
- [ ] `--output-dir /tmp/safe/path` is accepted
- [ ] `--output-dir /tmp/../etc/rings` is rejected with exit code 2
- [ ] TOML `output_dir = "../outside"` is rejected at workflow parse time
- [ ] Paths with `.` (current dir) are allowed (only `..` is dangerous)

---

## F-089: `--strict-parsing` CLI Flag

**Spec:** `specs/cli/commands-and-flags.md` lines 65–67

**Summary:** When `--strict-parsing` is set, treat cost parse confidence of `Low` or `None` as a hard error — halt execution, save state, exit code 2. Currently cost parsing failures are just warnings.

### Task 1: Add flag and enforcement logic

**Files:** `src/cli.rs`, `src/main.rs`, `src/engine.rs`

**Steps:**
- [ ] Add `--strict-parsing` flag to `RunArgs` in `src/cli.rs`: `pub strict_parsing: bool`
- [ ] Pass it through to `EngineConfig` as `strict_parsing: bool`
- [ ] In the engine, after cost parsing for each run: if `strict_parsing` and confidence is `Low` or `None`:
  1. Save state (same as budget cap flow)
  2. Print error: `Strict parsing enabled: cost confidence too low ({confidence}) on run {N}. Halting.`
  3. Set exit code to 2
  4. Break out of the run loop
- [ ] In JSONL mode, emit a `fatal_error` event before exiting

**Tests:**
- [ ] `--strict-parsing` with `Full` confidence: run continues normally
- [ ] `--strict-parsing` with `Partial` confidence: run continues (only Low/None trigger halt)
- [ ] `--strict-parsing` with `Low` confidence: run halts, state saved, exit 2
- [ ] `--strict-parsing` with `None` confidence: run halts, state saved, exit 2
- [ ] Without `--strict-parsing`: low confidence produces a warning but run continues (existing behavior)

---

## Tech Debt: Remove `unwrap()`/`expect()` from Production Code

**Ref:** CLAUDE.md rule — "No `unwrap()` or `expect()` in production code — all errors propagate via `?` and `anyhow`"

**Summary:** Audit found two `unwrap()`/`expect()` calls in production code paths that could cause hard panics instead of graceful errors.

### Task 1: Replace `.expect()` in Ctrl+C handler setup

**Files:** `src/main.rs`

**Steps:**
- [ ] Replace `.expect("Failed to install Ctrl+C handler")` on line 43 with proper error handling
- [ ] Since `main()` currently returns `()`, either: (a) convert main to return `Result<()>` via `process::exit` wrapper, or (b) use an `if let Err(e)` block that prints the error to stderr and exits with code 2
- [ ] Verify that failure to install the handler produces a clear user-facing error message, not a panic backtrace

**Tests:**
- [ ] Existing tests continue to pass (`just validate`)

---

### Task 2: Replace `.unwrap()` in dry-run phase position lookup

**Files:** `src/main.rs`

**Steps:**
- [ ] Replace `.unwrap()` on line 170 (phase position lookup in dry-run output) with `.unwrap_or(0)` or a safe fallback that cannot panic
- [ ] The current code iterates `plan.phases` and looks up each phase's index by name within the same collection — logically infallible, but should still be defended

**Tests:**
- [ ] Existing dry-run tests continue to pass
- [ ] `just validate` clean

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
