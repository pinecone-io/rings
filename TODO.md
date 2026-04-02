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

## F-175: Cargo Install Support

**Spec:** `specs/cli/distribution.md`

**Summary:** Rust users can install rings with `cargo install rings` without needing pre-built binaries. Requires publishing to crates.io.

### Task 1: Prepare for crates.io publishing

**Files:** `Cargo.toml`

**Steps:**
- [x] Verify `Cargo.toml` has required crates.io fields: `description`, `license`, `repository`, `keywords`, `categories`
- [x] Verify `cargo package` succeeds without errors (all required files included)
- [x] Add `exclude` patterns to keep the crate size reasonable (exclude test fixtures, specs, etc.)
- [x] Test with `cargo install --path .` locally

**Tests:**
- [x] `cargo install --path .` builds and installs successfully
- [x] `cargo package` produces a valid crate
- [x] `just validate` clean

---

## F-177: Reproducible Builds

**Spec:** `specs/cli/distribution.md`

**Summary:** Pin the Rust toolchain and commit Cargo.lock so any developer can reproduce the exact same release binary.

### Task 1: Pin toolchain and verify reproducibility

**Files:** `rust-toolchain.toml`, `Cargo.lock`

**Steps:**
- [x] Verify `rust-toolchain.toml` exists and pins a specific Rust version
- [x] Verify `Cargo.lock` is committed to the repository (not gitignored)
- [x] Document the build command in README or CONTRIBUTING: `cargo build --release --locked`
- [x] If already in place, mark as COMPLETE

**Tests:**
- [x] `cargo build --release --locked` succeeds
- [x] `rust-toolchain.toml` specifies exact version
- [x] `Cargo.lock` is tracked in git
- [x] `just validate` clean

---

## F-199, F-200, F-201: Named Locks for Concurrent Workflows

**Spec:** `specs/state/cancellation-resume.md`, `specs/workflow/workflow-file-format.md`

**Summary:** Allow multiple rings workflows to run concurrently against the same `context_dir` by assigning each a distinct `lock_name`. The lock file becomes `.rings.lock.<name>` instead of `.rings.lock`. Workflows with different lock names never block each other.

**Open Decisions:**
- Use `Option<String>` on `Workflow` (not a `LockName` newtype) — validation happens once in `validate()`, value consumed as `&str` downstream
- `lock_name` is not included in `structural_fingerprint()` — changing it on resume is a non-structural change
- Include lock name in stale-lock warning message for clarity (record in REVIEW.md)

### Task 1: Add `lock_name` field to workflow parsing and validation (F-201)

**Files:** `src/workflow.rs`

**Steps:**
- [x] Add `lock_name: Option<String>` with `#[serde(default)]` to `WorkflowConfig`
- [x] Add `lock_name: Option<String>` to `Workflow`
- [x] Add `WorkflowError::InvalidLockName(String)` variant with message: `invalid lock_name "<value>": must match [a-z0-9_-]+`
- [x] In `Workflow::validate()`, validate `lock_name` if present: reject empty string, validate with byte-level check (`!name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')`)
- [x] Propagate validated `lock_name` from `WorkflowConfig` to `Workflow`

**Tests:**
- [x] Valid names accepted: `"planner"`, `"build-01"`, `"a"`, `"my_workflow_1"`, `"123"`
- [x] Invalid names rejected: `""` (empty), `"Planner"` (uppercase), `"my lock"` (space), `"a.b"` (dot), `"a/b"` (slash), `"a!b"` (punctuation)
- [x] Absent `lock_name` field → `Workflow.lock_name == None`
- [x] Error message includes the offending value and the allowed pattern
- [x] `just validate` clean

### Task 2: Extend `ContextLock::acquire` for named locks (F-199, F-200)

**Files:** `src/lock.rs`

**Steps:**
- [x] Add `lock_name: Option<&str>` parameter to `ContextLock::acquire`
- [x] Extract helper: `fn lock_file_path(context_dir: &Path, lock_name: Option<&str>) -> PathBuf` returning `.rings.lock` or `.rings.lock.<name>`
- [x] Use the helper for path computation in `acquire` (the stored `ContextLock.path` already drives RAII `Drop`)
- [x] Add `lock_name: Option<String>` field to `LockError::ActiveProcess`
- [x] Update `Display` for `ActiveProcess` to branch: `None` → `"is already using"` / `Some(name)` → `"holds lock \"{name}\" on"`
- [x] Update all construction sites of `LockError::ActiveProcess` to pass `lock_name`

**Tests:**
- [x] `lock_name = Some("planner")` creates `.rings.lock.planner`, not `.rings.lock`
- [x] `lock_name = None` still creates `.rings.lock` (regression guard)
- [x] Two different names held simultaneously in the same `context_dir` — both succeed
- [x] Same name conflicts with itself (returns `ActiveProcess`)
- [x] Named lock does not conflict with unnamed lock (and vice versa)
- [x] Stale named lock detected and removed
- [x] Force-lock with a named lock overwrites the correct file
- [x] Drop removes the correct named lock file
- [x] Error message for named lock includes `holds lock "planner"`
- [x] Error message for unnamed lock matches existing format (regression)
- [x] `just validate` clean

### Task 3: Wire `lock_name` through call sites in `main.rs`

**Files:** `src/main.rs`

**Steps:**
- [x] At `run` call site (~line 573): pass `workflow.lock_name.as_deref()` to `ContextLock::acquire`
- [x] At `resume` call site (~line 1033): pass `workflow.lock_name.as_deref()` to `ContextLock::acquire`
- [x] Update stale-lock warning messages at both sites to include lock name when present
- [x] Update existing tests in `tests/stale_lock_detection.rs` to pass `None` as fourth arg

**Tests:**
- [x] Existing stale lock tests still pass with updated signature
- [x] `just validate` clean

---

## F-202 through F-207: Deterministic Gates

**Spec:** `specs/workflow/workflow-file-format.md` (Deterministic Gates section)

**Summary:** Allow workflow authors to attach shell command gates to phases and cycles. A gate runs before execution and its exit code determines whether to proceed, skip, stop, or error. Gates provide deterministic control flow (e.g., "stop planning if TODO.md exceeds 50 lines") without consuming an AI invocation.

### Task 1: Add `GateConfig` type and parse gates from TOML (F-202, F-203, F-204, F-205)

**Files:** `src/workflow.rs`

**Steps:**
- [x] Define `GateConfig` struct: `command: String`, `on_fail: Option<GateAction>`, `timeout: Option<DurationField>`
- [x] Define `GateAction` enum: `Skip`, `Stop`, `Error` with serde deserialization from lowercase strings
- [x] Add `cycle_gate: Option<GateConfig>` with `#[serde(default)]` to `WorkflowConfig`
- [x] Add `gate: Option<GateConfig>` with `#[serde(default)]` to `PhaseConfig`
- [x] Add `gate_each_run: bool` with `#[serde(default)]` to `PhaseConfig`
- [x] Propagate parsed gate fields to `Workflow` struct (add `cycle_gate: Option<GateConfig>`)
- [x] Add validation in `Workflow::validate()`:
  - Gate `command` must be non-empty if gate is present
  - `on_fail` must be `skip`, `stop`, or `error` (enforced by enum deserialization)
  - `skip` on `cycle_gate` is valid: skips all phases for that cycle, applies `delay_between_cycles`, then retries next cycle
  - `timeout` must be a valid duration if present
- [x] Add `WorkflowError` variant: `EmptyGateCommand { scope: String }`
- [x] Default `on_fail` for `cycle_gate`: `Stop`. Default for phase `gate`: `Skip`
- [x] Default `timeout`: 30 seconds
- [x] Include gate config in `structural_fingerprint()` computation

**Tests:**
- [x] Parse a workflow with `cycle_gate = { command = "true", on_fail = "stop" }` — fields present on `Workflow`
- [x] Parse a workflow with phase `gate = { command = "test -f foo" }` — default `on_fail` is `Skip`
- [x] Parse `cycle_gate` without explicit `on_fail` — default is `Stop`
- [x] `cycle_gate` with `on_fail = "skip"` — valid, skips all phases for that cycle
- [x] Reject gate with empty command — `EmptyGateCommand` error
- [x] Parse gate with `timeout = "10s"` — resolves to 10
- [x] Parse gate with `timeout = 60` — resolves to 60
- [x] Gate absent → `None` on both `Workflow` and `PhaseConfig`
- [x] `gate_each_run = true` parses correctly
- [x] `gate_each_run` defaults to `false`
- [x] Structural fingerprint changes when gate is added/removed/modified
- [x] `just validate` clean

### Task 2: Implement gate execution logic (F-202, F-203, F-205)

**Files:** `src/gate.rs` (new), `src/lib.rs`

**Steps:**
- [x] Create `src/gate.rs` module with `pub struct GateResult { pub command: String, pub exit_code: i32, pub passed: bool, pub stdout: String, pub stderr: String }`
- [x] Implement `pub fn evaluate_gate(gate: &GateConfig, context_dir: &Path) -> Result<GateResult>`:
  - Spawn `sh -c <command>` in `context_dir`
  - Capture stdout and stderr
  - Apply timeout: SIGTERM → 5s → SIGKILL (reuse existing timeout pattern from executor)
  - Timeout counts as failure (exit_code = -1 or similar sentinel)
  - Return `GateResult` with exit code and pass/fail
- [x] Ensure gate commands inherit the process environment (same as executor)
- [x] No prompt content in command args (gate commands are author-defined, not user input — but document this)

**Tests:**
- [x] `evaluate_gate` with `command = "true"` → passed, exit_code 0
- [x] `evaluate_gate` with `command = "false"` → not passed, exit_code 1
- [x] `evaluate_gate` with `command = "echo hello"` → passed, stdout contains "hello"
- [x] `evaluate_gate` with `command = "exit 42"` → not passed, exit_code 42
- [x] `evaluate_gate` with timeout exceeded → not passed (use `sleep 60` with 1s timeout)
- [x] `evaluate_gate` runs in the specified `context_dir`
- [x] `just validate` clean

### Task 3: Integrate cycle gate into engine loop (F-203)

**Files:** `src/engine.rs` (or wherever the main cycle loop lives)

**Steps:**
- [x] At the top of each cycle iteration, before any phases run, check `workflow.cycle_gate`
- [x] If present, call `evaluate_gate()` with the gate config and `context_dir`
- [x] If the gate passes (exit 0), continue normally
- [x] If the gate fails:
  - `on_fail = "stop"` → save state, exit with code 0 (same as completion signal)
  - `on_fail = "error"` → save state, exit with code 2
- [x] Log the gate result (human and JSONL — see Task 5)

**Tests:**
- [x] Workflow with `cycle_gate = { command = "true" }` — cycles run normally
- [x] Workflow with `cycle_gate = { command = "false", on_fail = "stop" }` — exits gracefully after gate fails on first cycle
- [x] Workflow with `cycle_gate = { command = "false", on_fail = "error" }` — exits with error code 2
- [x] Workflow with `cycle_gate = { command = "false", on_fail = "skip" }` and `delay_between_cycles` — skips phases, waits delay, retries next cycle
- [x] Cycle gate that passes on first cycle but fails on second — first cycle's phases all execute, second cycle does not start
- [x] `just validate` clean

### Task 4: Integrate phase gate into engine loop (F-202, F-207)

**Files:** `src/engine.rs`

**Steps:**
- [x] Before running a phase's first invocation in a cycle, check `phase.gate`
- [x] If present, call `evaluate_gate()` with the gate config and `context_dir`
- [x] If the gate passes, run the phase normally
- [x] If the gate fails:
  - `on_fail = "skip"` → skip all runs of this phase for this cycle, advance to next phase
  - `on_fail = "stop"` → save state, exit with code 0
  - `on_fail = "error"` → save state, exit with code 2
- [x] If `gate_each_run = true`, evaluate the gate before every individual run within `runs_per_cycle`, not just the first
- [x] Log the gate result for each evaluation

**Tests:**
- [x] Phase with `gate = { command = "true" }` — phase runs normally
- [x] Phase with `gate = { command = "false" }` — phase skipped (default `on_fail = "skip"`), next phase runs
- [x] Phase with `gate = { command = "false", on_fail = "stop" }` — workflow stops gracefully
- [x] Phase with `gate = { command = "false", on_fail = "error" }` — workflow exits with code 2
- [x] Phase with `runs_per_cycle = 3` and gate — gate checked once before first run (default)
- [x] Phase with `runs_per_cycle = 3`, `gate_each_run = true`, and a gate that fails on the second check — first run executes, second is skipped/stopped
- [x] Two phases: first has failing gate (skip), second has no gate — second phase still runs
- [x] `just validate` clean

### Task 5: Gate logging in human and JSONL output (F-206)

**Files:** `src/output.rs` (or equivalent output/event module)

**Steps:**
- [x] Human output format: `[cycle N] cycle gate: \`<command>\` → exit <code> (pass|fail → <action>)`
- [x] Human output format: `[cycle N] phase "<name>" gate: \`<command>\` → exit <code> (pass|fail → <action>)`
- [x] Truncate displayed command to 80 chars if longer, with `...` suffix
- [x] JSONL `gate_result` event: `{"event":"gate_result","run_id":"...","timestamp":"...","scope":"cycle"|"phase","phase":null|"<name>","command":"...","exit_code":<int>,"passed":<bool>,"action":"stop"|"skip"|"error"|null}`
- [x] Gate stdout/stderr captured in run log directory (e.g., `runs/NNN-gate-cycle.log` or `runs/NNN-gate-<phase>.log`)

**Tests:**
- [x] Human output contains gate command and exit code for cycle gate
- [x] Human output contains phase name for phase gate
- [x] JSONL event has correct schema for passing gate
- [x] JSONL event has correct schema for failing gate with action
- [x] `just validate` clean

### Task 6: Dry-run support for gates (F-202, F-203)

**Files:** `src/engine.rs`, `src/output.rs`

**Steps:**
- [x] In `--dry-run` mode, display gate configuration without executing the command
- [x] Format: `[cycle gate] command: \`<command>\`, on_fail: <action>, timeout: <duration>`
- [x] Format: `[phase "<name>" gate] command: \`<command>\`, on_fail: <action>, timeout: <duration>`
- [x] Include gates in startup header display when present

**Tests:**
- [x] Dry run with cycle gate shows gate config
- [x] Dry run with phase gate shows gate config per phase
- [x] Dry run does not execute any gate commands
- [x] `just validate` clean

---

---
