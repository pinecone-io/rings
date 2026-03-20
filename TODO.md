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

## F-099: Inspect Files Changed View

**Spec:** `specs/cli/inspect-command.md` (--show files-changed section)

**Summary:** `rings inspect <RUN_ID> --show files-changed` shows which files were added/modified/deleted in each run, attributed by phase and cycle. Requires manifest data (F-117/F-118).

### Task 1: Implement `--show files-changed` view

**Files:** `src/inspect.rs`, `src/main.rs`

**Steps:**
- [x] In `inspect_inner`, handle `InspectView::FilesChanged`:
  1. Read manifest diffs from costs.jsonl or manifest files
  2. Group changes by file path, showing which run/phase/cycle modified each file
  3. Display as a file-centric table: each file with the list of runs that touched it
- [x] Support `--cycle N` and `--phase NAME` filters
- [x] If no manifest data exists, print a helpful message: "No file change data available. Enable `manifest_enabled = true` in your workflow."
- [x] In JSONL mode, emit structured file change data

**Tests:**
- [x] View shows added/modified/deleted files attributed to correct runs
- [x] `--cycle 1` filters to only cycle 1 changes
- [x] Missing manifest data produces helpful message, not error
- [x] JSONL mode emits structured output
- [x] `just validate` clean

---

## F-120: Credential File Protection in Manifests

**Spec:** `specs/observability/file-lineage.md`

**Summary:** Always exclude credential files (`.env`, `*.key`, `*.pem`, etc.) from file manifests regardless of user ignore patterns. Prevents accidentally recording sensitive file hashes in audit logs.

### Task 1: Add hardcoded credential exclusions

**Files:** `src/manifest.rs`

**Steps:**
- [ ] Define a static list of credential patterns: `.env`, `.env.*`, `*.key`, `*.pem`, `*.p12`, `*.pfx`, `*.jks`, `*.keystore`, `*credentials*`, `*secret*`
- [ ] In the manifest scanning function, apply these exclusions in addition to user-specified ignore patterns
- [ ] These patterns cannot be overridden — they are always excluded
- [ ] Add a comment explaining the security rationale

**Tests:**
- [ ] `.env` file is excluded from manifest even with no user ignore patterns
- [ ] `server.key` is excluded from manifest
- [ ] Normal source files are included
- [ ] User ignore patterns still work alongside credential exclusions
- [ ] `just validate` clean

---

## F-020: Timeout Per Run

**Spec:** `specs/execution/engine.md`

**Summary:** Set a per-run timeout so a hung executor invocation doesn't stall the workflow indefinitely. When a timeout fires, the executor subprocess is killed, the run is logged as timed out, and execution continues to the next run.

### Task 1: Add timeout configuration and enforcement

**Files:** `src/workflow.rs`, `src/engine.rs`, `src/cli.rs`

**Steps:**
- [ ] The `timeout_per_run_secs` field already exists in the workflow config — verify it's wired into the engine's poll loop
- [ ] In the executor poll loop: if `timeout_deadline` is set and `Instant::now() > timeout_deadline`, kill the subprocess and treat as a timeout error
- [ ] Log the timeout in the run's audit entry with `failure_reason: "timeout"`
- [ ] The `FailureReason::Timeout` variant already exists in `state.rs` — verify it's used correctly
- [ ] If already implemented, mark as COMPLETE after verification

**Tests:**
- [ ] Run with `timeout_per_run_secs = 1` and a mock executor that sleeps 5s: executor is killed, run logged as timeout
- [ ] Run without timeout: no timeout behavior
- [ ] Timed-out run saves state for resume
- [ ] `just validate` clean

---

## F-012: Completion Signal Modes

**Spec:** `specs/execution/completion-detection.md`

**Summary:** Support matching the completion signal by exact substring (default), line anchor, or full regex. Currently only substring matching is implemented. The `completion_signal_mode` field already exists in the workflow config.

### Task 1: Add regex completion signal matching

**Files:** `src/completion.rs`, `src/workflow.rs`

**Steps:**
- [ ] Verify the current modes: `"substring"` (default) and `"line"` — both should already work
- [ ] Add `"regex"` mode: compile `completion_signal` as a regex at workflow parse time, match against executor output
- [ ] Invalid regex in `completion_signal` when mode is `"regex"` produces exit 2 at parse time
- [ ] If `"substring"` and `"line"` are already working, only the `"regex"` mode needs implementation

**Tests:**
- [ ] `completion_signal_mode = "substring"`: matches signal anywhere in output
- [ ] `completion_signal_mode = "line"`: matches only when signal is an entire line
- [ ] `completion_signal_mode = "regex"`: matches regex pattern (e.g., `"DONE_\\d+"` matches `"DONE_42"`)
- [ ] Invalid regex exits 2 at parse time
- [ ] `just validate` clean

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
