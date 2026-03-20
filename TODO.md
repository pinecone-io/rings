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

## F-012: Completion Signal Modes

**Spec:** `specs/execution/completion-detection.md`

**Summary:** Support matching the completion signal by exact substring (default), line anchor, or full regex. Currently only substring matching is implemented. The `completion_signal_mode` field already exists in the workflow config.

### Task 1: Add regex completion signal matching

**Files:** `src/completion.rs`, `src/workflow.rs`

**Steps:**
- [x] Verify the current modes: `"substring"` (default) and `"line"` — both should already work
- [x] Add `"regex"` mode: compile `completion_signal` as a regex at workflow parse time, match against executor output
- [x] Invalid regex in `completion_signal` when mode is `"regex"` produces exit 2 at parse time
- [x] If `"substring"` and `"line"` are already working, only the `"regex"` mode needs implementation

**Tests:**
- [x] `completion_signal_mode = "substring"`: matches signal anywhere in output
- [x] `completion_signal_mode = "line"`: matches only when signal is an entire line
- [x] `completion_signal_mode = "regex"`: matches regex pattern (e.g., `"DONE_\\d+"` matches `"DONE_42"`)
- [x] Invalid regex exits 2 at parse time
- [x] `just validate` clean

---

## F-013: Completion Signal Phase Restriction

**Spec:** `specs/execution/completion-detection.md`

**Summary:** Limit which phases can trigger workflow completion via `completion_signal_phases`. Prevents early phases from accidentally ending the workflow.

### Task 1: Add phase restriction to completion detection

**Files:** `src/workflow.rs`, `src/engine.rs`

**Steps:**
- [ ] Add `completion_signal_phases: Option<Vec<String>>` to the workflow config (may already exist)
- [ ] In the engine's completion signal check: if `completion_signal_phases` is set and non-empty, only check for the signal in output from phases listed in the array
- [ ] If the signal is detected in a non-listed phase, log it but don't trigger completion
- [ ] Validate at startup that all phase names in `completion_signal_phases` actually exist in the workflow

**Tests:**
- [ ] With `completion_signal_phases = ["reviewer"]`: signal in "builder" output is ignored, signal in "reviewer" triggers completion
- [ ] With no restriction (default): any phase can trigger completion
- [ ] Invalid phase name in restriction list exits 2 at startup
- [ ] `just validate` clean

---

## F-014/F-015/F-017: Phase Contracts — Consumes, Produces, and Advisory Warnings

**Spec:** `specs/workflow/phase-contracts.md`

**Summary:** Allow phases to declare which files they read (`consumes`) and write (`produces`). At startup and during execution, rings warns when declared inputs don't exist or declared outputs weren't created. These are advisory warnings — they don't block execution.

### Task 1: Parse consumes/produces in workflow config

**Files:** `src/workflow.rs`

**Steps:**
- [ ] Add `consumes: Vec<String>` and `produces: Vec<String>` fields (with `#[serde(default)]`) to phase config
- [ ] Add `produces_required: bool` field (with `#[serde(default)]`) — when true, missing produces is a hard error not just a warning (F-016)
- [ ] Validate at parse time: patterns should be valid glob strings
- [ ] Store the parsed patterns in `PhaseConfig`

**Tests:**
- [ ] Phase with `consumes = ["src/*.rs"]` parses correctly
- [ ] Phase with `produces = ["output.txt"]` parses correctly
- [ ] Phase with no consumes/produces fields works (empty vecs)
- [ ] Invalid glob pattern produces parse error
- [ ] `just validate` clean

---

### Task 2: Startup consumes validation (F-152)

**Files:** `src/main.rs` (or `src/engine.rs`)

**Steps:**
- [ ] At startup, for each phase with `consumes` patterns:
  1. Scan `context_dir` for files matching each pattern
  2. If no files match AND the pattern is not mentioned in the phase's prompt text, print warning
- [ ] Warning text: `⚠  Phase "{name}" declares consumes = ["{pattern}"] but no matching files exist in context_dir and the pattern is not mentioned in the prompt.`
- [ ] Suppressible with `--no-contract-check` (already exists as a CLI flag)
- [ ] Only warn in human output mode

**Tests:**
- [ ] Consumes pattern with no matching files triggers warning
- [ ] Consumes pattern with matching files produces no warning
- [ ] Pattern mentioned in prompt text suppresses the warning
- [ ] `--no-contract-check` suppresses warning
- [ ] `just validate` clean

---

### Task 3: Post-run produces validation (F-153)

**Files:** `src/engine.rs`

**Steps:**
- [ ] After each run completes, for phases with `produces` patterns:
  1. Scan `context_dir` for files matching each pattern
  2. If no files match any produces pattern, print warning
- [ ] Warning text: `⚠  Phase "{name}" (run {N}): produces = ["{pattern}"] but no matching files were created/modified.`
- [ ] If `produces_required = true`, treat as hard error: save state and exit 2
- [ ] Suppressible with `--no-contract-check`

**Tests:**
- [ ] Phase that produces declared files: no warning
- [ ] Phase that doesn't produce declared files: warning
- [ ] `produces_required = true` with missing produces: exits 2
- [ ] `--no-contract-check` suppresses warning
- [ ] `just validate` clean

---

## F-052: SIGTERM Handling

**Spec:** `specs/state/cancellation-resume.md`

**Summary:** Treat SIGTERM like Ctrl+C — gracefully save state and exit with code 130. This allows process managers (systemd, Docker, supervisord) to stop rings cleanly.

### Task 1: Register SIGTERM handler

**Files:** `src/cancel.rs`, `src/main.rs`

**Steps:**
- [ ] In the signal handler setup, register for both SIGINT (Ctrl+C) and SIGTERM
- [ ] Both signals trigger the same `CancelState` transition (Canceling → ForceKill on second signal)
- [ ] On Unix: use `signal_hook` or `ctrlc` crate's SIGTERM support
- [ ] Verify that the existing graceful shutdown flow (save state, print resume command) works for SIGTERM

**Tests:**
- [ ] SIGTERM triggers graceful cancellation (state saved, resume command printed)
- [ ] Double SIGTERM force-kills (same as double Ctrl+C)
- [ ] Exit code is 130 on SIGTERM
- [ ] `just validate` clean

---

## F-055: Context Directory Lock

**Spec:** `specs/state/cancellation-resume.md`

**Summary:** Prevent two rings instances from running against the same `context_dir` simultaneously. Uses a lock file to detect concurrent access.

### Task 1: Add context_dir lock file

**Files:** `src/lock.rs` (already exists), `src/engine.rs`

**Steps:**
- [ ] Verify that `src/lock.rs` already implements lock file creation/checking
- [ ] Ensure the lock is acquired before the first executor spawn in the engine
- [ ] Lock file should be written to `context_dir/.rings.lock` containing the PID and run ID
- [ ] If lock already exists and the PID is still running: print error and exit 2
- [ ] If lock exists but PID is not running (stale): print warning, remove stale lock, proceed (F-056)
- [ ] Release the lock on all exit paths (normal, Ctrl+C, error)
- [ ] Support `--force-lock` flag (F-091) to override the lock check

**Tests:**
- [ ] Second rings instance against same context_dir is blocked with clear error
- [ ] Stale lock from dead process is removed with warning
- [ ] Lock is released on normal completion
- [ ] Lock is released on Ctrl+C
- [ ] `--force-lock` overrides the lock check
- [ ] `just validate` clean

---

## F-121: mtime Optimization for Manifest Scanning

**Spec:** `specs/observability/file-lineage.md`

**Summary:** Skip re-hashing files whose modification time hasn't changed since the last manifest. Only compute SHA256 for files with updated mtime. Keeps large repos fast.

### Task 1: Add mtime-based hash skipping

**Files:** `src/manifest.rs`

**Steps:**
- [ ] When computing a new manifest, compare each file's mtime against the previous manifest's entry for the same path
- [ ] If the path exists in the previous manifest and mtime is identical, reuse the previous SHA256 hash without reading the file
- [ ] If mtime differs or the file is new, compute the SHA256 hash normally
- [ ] Pass the previous manifest (if available) into the manifest computation function

**Tests:**
- [ ] File with unchanged mtime reuses previous hash (verify by checking that file content is not read)
- [ ] File with changed mtime gets a fresh hash
- [ ] New file (not in previous manifest) gets computed hash
- [ ] First manifest (no previous) computes all hashes
- [ ] `just validate` clean

---
