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

## Bug: Config tests race on process-global state (`cwd` and `XDG_CONFIG_HOME`)

**Ref:** CLAUDE.md testing requirements

**Summary:** The config tests in `src/config.rs` (lines 91-202) use `std::env::set_current_dir` and `std::env::set_var("XDG_CONFIG_HOME")` which are process-global mutations. Rust runs unit tests in parallel within the same process, so concurrent tests interfere with each other. The test `test_load_user_config_when_no_project_config` fails intermittently with "No such file or directory" when another test's `set_current_dir` runs between the save and restore of `cwd`. The `set_var`/`remove_var` calls race similarly.

### Task 1: Make config tests serialization-safe

**Files:** `src/config.rs`

**Steps:**
- [x] Replace the `with_cwd` + `set_current_dir` pattern with a testable API that accepts an explicit base directory parameter instead of relying on `cwd`
- [x] Add a `RingsConfig::load_from(project_dir: &Path, xdg_config_home: Option<&Path>) -> Result<Self>` method that takes explicit paths instead of reading from `cwd` and `XDG_CONFIG_HOME`
- [x] Have the public `RingsConfig::load()` call `load_from(std::env::current_dir()?, ...)` as the production entry point
- [x] Rewrite tests to call `load_from(temp_dir, Some(xdg_dir))` directly — no `set_current_dir` or `set_var` needed
- [x] Remove the `with_cwd` helper entirely

**Tests:**
- [x] All config tests pass reliably in parallel (run `cargo test` 10 times to verify no flakes)
- [x] `RingsConfig::load()` still works in production (delegates to `load_from`)
- [x] `just validate` clean

---

## F-150: No-Files-Changed Streak Warning

**Spec:** `specs/execution/engine.md` (Advisory Checks table)

**Summary:** Warn after 3 consecutive runs where the executor produced no file changes in `context_dir`, suggesting the workflow may be stuck in a loop doing nothing productive.

### Task 1: Add no-change streak detection

**Files:** `src/engine.rs`

**Steps:**
- [ ] Track a counter of consecutive runs with no file changes (use git status or manifest diff if available, else skip this check)
- [ ] After each run completes: if no files were changed, increment counter; otherwise reset to 0
- [ ] If counter reaches 3, print warning: `⚠  3 consecutive runs produced no file changes.\n   The workflow may be stuck. Consider reviewing the prompt or canceling.`
- [ ] Only warn once per streak (don't re-warn at 4, 5, etc. — only at 3)
- [ ] Only warn in human output mode
- [ ] If file manifest is not enabled, skip this check entirely (no data to compare)

**Tests:**
- [ ] 3 consecutive no-change runs triggers warning
- [ ] 2 no-change runs followed by a change run: no warning
- [ ] Warning fires only once at the 3rd run, not again at 4th
- [ ] Manifest not enabled: check is skipped entirely
- [ ] `just validate` clean

---
