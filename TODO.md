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

## F-150: No-Files-Changed Streak Warning

**Spec:** `specs/execution/engine.md` (Advisory Checks table)

**Summary:** Warn after 3 consecutive runs where the executor produced no file changes in `context_dir`, suggesting the workflow may be stuck in a loop doing nothing productive.

### Task 1: Add no-change streak detection

**Files:** `src/engine.rs`

**Steps:**
- [x] Track a counter of consecutive runs with no file changes (use git status or manifest diff if available, else skip this check)
- [x] After each run completes: if no files were changed, increment counter; otherwise reset to 0
- [x] If counter reaches 3, print warning: `⚠  3 consecutive runs produced no file changes.\n   The workflow may be stuck. Consider reviewing the prompt or canceling.`
- [x] Only warn once per streak (don't re-warn at 4, 5, etc. — only at 3)
- [x] Only warn in human output mode
- [x] If file manifest is not enabled, skip this check entirely (no data to compare)

**Tests:**
- [x] 3 consecutive no-change runs triggers warning
- [x] 2 no-change runs followed by a change run: no warning
- [x] Warning fires only once at the 3rd run, not again at 4th
- [x] Manifest not enabled: check is skipped entirely
- [x] `just validate` clean

---

## F-118: File Diff Detection

**Spec:** `specs/observability/file-lineage.md`

**Summary:** Compare consecutive file manifests to determine which files were added, modified, or deleted by each run. The manifest infrastructure (F-117) already exists (`manifest_enabled`, SHA256 fingerprinting). This adds the diff computation and records changes in costs.jsonl and JSONL events.

### Task 1: Compute manifest diffs

**Files:** `src/manifest.rs`, `src/engine.rs`

**Steps:**
- [ ] Add a `compute_diff(before: &Manifest, after: &Manifest) -> ManifestDiff` function to `src/manifest.rs`
- [ ] `ManifestDiff` struct: `added: Vec<String>`, `modified: Vec<String>`, `deleted: Vec<String>`, `files_changed: u32`
- [ ] Compare by path and SHA256: same path + different hash = modified; path in after but not before = added; path in before but not after = deleted
- [ ] In the engine, after each run: if manifest is enabled, compute diff between before and after manifests
- [ ] Include diff data in `costs.jsonl` entry (`files_added`, `files_modified`, `files_deleted`, `files_changed`)
- [ ] Include diff data in JSONL `run_end` event

**Tests:**
- [ ] Added file detected correctly
- [ ] Modified file (same path, different hash) detected correctly
- [ ] Deleted file detected correctly
- [ ] Unchanged files not included in diff
- [ ] Diff data appears in costs.jsonl entry
- [ ] `just validate` clean

---

## F-119: File Manifest Ignore Patterns

**Spec:** `specs/observability/file-lineage.md` (Manifest Configuration section)

**Summary:** Allow users to specify directories/patterns to exclude from file manifest scanning (e.g., `.git/`, `target/`, `node_modules/`).

### Task 1: Add ignore patterns to manifest config

**Files:** `src/workflow.rs`, `src/manifest.rs`

**Steps:**
- [ ] The `manifest_ignore` field already exists in the workflow config — verify it's wired into manifest scanning
- [ ] When scanning `context_dir`, skip entries matching any ignore pattern (glob-style matching)
- [ ] Default ignore patterns should include `.git/` at minimum
- [ ] Apply ignore patterns to both the initial manifest and all subsequent manifests

**Tests:**
- [ ] `.git/` directory is excluded from manifest by default
- [ ] Custom ignore pattern `target/` excludes that directory
- [ ] Files outside ignore patterns are included normally
- [ ] `just validate` clean

---

## F-066: Default Executor Config

**Spec:** `specs/state/configuration.md`

**Summary:** Allow defining executor defaults in the workflow TOML `[executor]` section that apply to all phases unless overridden. The `[executor]` section already exists and works — this task is about ensuring the inheritance semantics are correct and documented when combined with per-phase overrides.

### Task 1: Verify and test executor config inheritance

**Files:** `src/workflow.rs`, `src/engine.rs`

**Steps:**
- [ ] Verify that when a phase has no `executor` block, it inherits the workflow-level `[executor]` config
- [ ] Verify that per-phase `executor.binary` overrides only that field, not the entire executor config
- [ ] Verify that per-phase `executor.args` replaces the workflow-level args (not appends — that's what `extra_args` is for)
- [ ] Add documentation comment in workflow.rs explaining the inheritance model
- [ ] If inheritance is already working correctly, mark as COMPLETE after verification

**Tests:**
- [ ] Phase without executor block uses workflow-level executor
- [ ] Phase with `executor.binary = "other"` but no `args` inherits workflow-level args
- [ ] Phase with `executor.args = [...]` replaces workflow-level args entirely
- [ ] `just validate` clean

---

## F-099: Inspect Files Changed View

**Spec:** `specs/cli/inspect-command.md` (--show files-changed section)

**Summary:** `rings inspect <RUN_ID> --show files-changed` shows which files were added/modified/deleted in each run, attributed by phase and cycle. Requires manifest data (F-117/F-118).

### Task 1: Implement `--show files-changed` view

**Files:** `src/inspect.rs`, `src/main.rs`

**Steps:**
- [ ] In `inspect_inner`, handle `InspectView::FilesChanged`:
  1. Read manifest diffs from costs.jsonl or manifest files
  2. Group changes by file path, showing which run/phase/cycle modified each file
  3. Display as a file-centric table: each file with the list of runs that touched it
- [ ] Support `--cycle N` and `--phase NAME` filters
- [ ] If no manifest data exists, print a helpful message: "No file change data available. Enable `manifest_enabled = true` in your workflow."
- [ ] In JSONL mode, emit structured file change data

**Tests:**
- [ ] View shows added/modified/deleted files attributed to correct runs
- [ ] `--cycle 1` filters to only cycle 1 changes
- [ ] Missing manifest data produces helpful message, not error
- [ ] JSONL mode emits structured output
- [ ] `just validate` clean

---
