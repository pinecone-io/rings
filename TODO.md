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

## F-147: Disk Space Check

**Spec:** `specs/execution/engine.md` (Advisory Checks table)

**Summary:** At startup, check available disk space in the output directory. Warn at < 100 MB free, abort with exit 2 at < 10 MB. Prevents silently losing run data mid-execution.

### Task 1: Add disk space check at startup

**Files:** `src/main.rs` (or `src/engine.rs`)

**Steps:**
- [x] After resolving the output directory, check available disk space using `fs2::available_space()` or `nix::sys::statvfs` (or a cross-platform alternative)
- [x] If < 10 MB: print error `Error: Less than 10 MB free in output directory ({path}). Aborting to prevent data loss.` and exit 2
- [x] If < 100 MB but >= 10 MB: print warning `⚠  Low disk space: only {N} MB free in output directory ({path}).`
- [x] Only check in human output mode for warnings; the fatal < 10 MB check applies in all modes
- [x] Use `#[cfg(unix)]` with `std::os::unix::fs::MetadataExt` or the `fs2` crate for portable disk space queries

**Tests:**
- [x] Mock/temp filesystem with limited space triggers warning at < 100 MB
- [x] Mock/temp filesystem with very low space triggers abort at < 10 MB
- [x] Adequate disk space produces no warning
- [x] `just validate` clean

---

## F-108: Auto-Generate summary.md

**Spec:** `specs/observability/audit-logs.md`

**Summary:** After a workflow completes (any exit path), generate a human-readable `summary.md` in the run's output directory. Contains the same info as the completion/cancellation display but in a persistent markdown file for later reference.

### Task 1: Generate summary.md on run completion

**Files:** `src/audit.rs` (or new function), `src/engine.rs`

**Steps:**
- [ ] Create a `generate_summary_md(run_dir: &Path, meta: &RunMeta, state: &StateFile, costs: &[CostEntry], phase_costs: &[(String, f64, u32)]) -> Result<()>` function
- [ ] Generate markdown content including:
  - Run ID, workflow file, status, started_at
  - Context dir, output dir
  - Cycles completed, total runs, total cost
  - Phase cost breakdown table
  - Token totals (if available)
  - If canceled: resume command
  - If completed: which run/cycle triggered completion
- [ ] Write to `{run_dir}/summary.md`
- [ ] Call this function from all engine exit paths: completion, max_cycles, cancellation, budget_cap, executor_error

**Tests:**
- [ ] Completed run produces `summary.md` with correct status and cost
- [ ] Canceled run produces `summary.md` with resume command
- [ ] `summary.md` contains phase cost breakdown
- [ ] `summary.md` is valid markdown (no broken formatting)
- [ ] `just validate` clean

---

## F-075: `rings completions` — Shell Completion Scripts

**Spec:** `specs/cli/completion-and-manpage.md`

**Summary:** `rings completions <SHELL>` generates shell completion scripts for bash, zsh, or fish. Uses clap's built-in completion generation.

### Task 1: Implement completions command

**Files:** `src/main.rs`, `src/cli.rs`

**Steps:**
- [ ] Replace the stub in `cmd_completions` with actual implementation using `clap_complete`:
  1. Add `clap_complete` to `Cargo.toml` dependencies
  2. Match on the shell argument (bash, zsh, fish)
  3. Call `clap_complete::generate()` with the CLI definition, writing to stdout
- [ ] The user pipes this to their shell config: `rings completions zsh > ~/.zfunc/_rings`

**Tests:**
- [ ] `rings completions bash` produces valid bash completion script (output contains expected patterns)
- [ ] `rings completions zsh` produces valid zsh completion script
- [ ] `rings completions fish` produces valid fish completion script
- [ ] Invalid shell name exits with error
- [ ] `just validate` clean

---

## Bug: Config tests race on process-global state (`cwd` and `XDG_CONFIG_HOME`)

**Ref:** CLAUDE.md testing requirements

**Summary:** The config tests in `src/config.rs` (lines 91-202) use `std::env::set_current_dir` and `std::env::set_var("XDG_CONFIG_HOME")` which are process-global mutations. Rust runs unit tests in parallel within the same process, so concurrent tests interfere with each other. The test `test_load_user_config_when_no_project_config` fails intermittently with "No such file or directory" when another test's `set_current_dir` runs between the save and restore of `cwd`. The `set_var`/`remove_var` calls race similarly.

### Task 1: Make config tests serialization-safe

**Files:** `src/config.rs`

**Steps:**
- [ ] Replace the `with_cwd` + `set_current_dir` pattern with a testable API that accepts an explicit base directory parameter instead of relying on `cwd`
- [ ] Add a `RingsConfig::load_from(project_dir: &Path, xdg_config_home: Option<&Path>) -> Result<Self>` method that takes explicit paths instead of reading from `cwd` and `XDG_CONFIG_HOME`
- [ ] Have the public `RingsConfig::load()` call `load_from(std::env::current_dir()?, ...)` as the production entry point
- [ ] Rewrite tests to call `load_from(temp_dir, Some(xdg_dir))` directly — no `set_current_dir` or `set_var` needed
- [ ] Remove the `with_cwd` helper entirely

**Tests:**
- [ ] All config tests pass reliably in parallel (run `cargo test` 10 times to verify no flakes)
- [ ] `RingsConfig::load()` still works in production (delegates to `load_from`)
- [ ] `just validate` clean

---
