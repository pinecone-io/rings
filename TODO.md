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

## F-187: Styled Cycle Transitions

**Spec:** `specs/observability/runtime-output.md`

**Summary:** Cycle boundaries show a horizontal rule with the cycle number and previous cycle cost embedded. The `format_cycle_boundary` function already exists — verify it uses the style system.

### Task 1: Verify styled cycle boundaries

**Files:** `src/display.rs`

**Steps:**
- [x] Verify `format_cycle_boundary` uses `style::dim` for dashes, `style::bold` for cycle number, `style::accent` for cost
- [x] Verify the output matches the spec format: `── Cycle N ──── $X.XX prev ──`
- [x] If already working, mark as COMPLETE

**Tests:**
- [x] Cycle boundary line contains styled cycle number and cost
- [x] First cycle has no cost suffix
- [x] `NO_COLOR=1` produces plain text
- [x] `just validate` clean

---

## F-189: Styled Dry Run Output

**Spec:** `specs/observability/runtime-output.md`

**Summary:** `rings run --dry-run` uses the same color system as live runs for visual consistency.

### Task 1: Verify dry-run styling

**Files:** `src/main.rs` (dry-run output section)

**Steps:**
- [ ] Verify dry-run output uses semantic colors: `style::bold` for headers, `style::accent` for cost estimates, `style::dim` for structural elements
- [ ] Verify completion signal check results use `style::success` (✓) and `style::warn` (✗)
- [ ] Verify `--no-color` disables styling in dry-run output
- [ ] If already working, mark as COMPLETE

**Tests:**
- [ ] Dry-run output is styled with colors on TTY
- [ ] `NO_COLOR=1` produces plain dry-run output
- [ ] `just validate` clean

---

## F-018: Data Flow Documentation in Inspect

**Spec:** `specs/workflow/phase-contracts.md`, `specs/cli/inspect-command.md`

**Summary:** `rings inspect <RUN_ID> --show data-flow` shows declared vs. actual data flow for each phase. The `InspectView::DataFlow` variant already exists and has partial implementation.

### Task 1: Complete data-flow view

**Files:** `src/inspect.rs`, `src/main.rs`

**Steps:**
- [ ] Verify `InspectView::DataFlow` handler in `inspect_inner` loads declared flow from workflow consumes/produces
- [ ] Add actual file attribution: which files were actually changed by each phase (from manifest diffs in costs.jsonl)
- [ ] Display format per spec: declared flow diagram, then actual file attribution table
- [ ] Support `--cycle N` and `--phase NAME` filters
- [ ] In JSONL mode, emit structured data flow information

**Tests:**
- [ ] Data flow view shows declared consumes/produces for each phase
- [ ] Actual file changes are attributed to correct phases
- [ ] Missing contract declarations show "no contracts declared"
- [ ] JSONL mode emits structured output
- [ ] `just validate` clean

---

## F-057: Cross-Machine Resume Documentation

**Spec:** `specs/state/cancellation-resume.md`

**Summary:** Document that resume requires the workflow file at the same absolute path. When paths don't match, print a clear error suggesting `--parent-run` for cross-machine linking.

### Task 1: Add path mismatch check on resume

**Files:** `src/main.rs` (in `resume_inner`)

**Steps:**
- [ ] On resume, compare the current workflow file's absolute path against the path stored in `run.toml`
- [ ] If paths differ, print a warning (not error): `⚠  Workflow file path has changed:\n   Saved: {old_path}\n   Current: {new_path}\n   This may cause issues if the workflow structure has also changed.`
- [ ] The phase fingerprint check (F-050) already catches structural changes — this is for path-only changes (e.g., moved repo)
- [ ] If the path is different but fingerprint matches, proceed with warning only

**Tests:**
- [ ] Resume with same path: no warning
- [ ] Resume with different path but same fingerprint: warning but proceeds
- [ ] `just validate` clean

---

## F-100: Inspect Data Flow View

**Spec:** `specs/cli/inspect-command.md` (--show data-flow section)

**Summary:** `rings inspect <RUN_ID> --show data-flow` shows declared vs. actual file inputs and outputs for each phase. Requires phase contracts (F-014/015) and file manifest (F-117).

### Task 1: Implement data-flow view display

**Files:** `src/inspect.rs`

**Steps:**
- [ ] The `InspectView::DataFlow` handler already has partial implementation loading declared flow
- [ ] Add rendering of the actual file attribution from manifest diffs
- [ ] Show which files each phase actually touched vs. what it declared it would touch
- [ ] Highlight mismatches: files produced but not declared, files declared but not produced
- [ ] Support `--phase NAME` filter

**Tests:**
- [ ] View shows both declared and actual data flow
- [ ] Mismatches are highlighted
- [ ] Phase filter works correctly
- [ ] `just validate` clean

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
