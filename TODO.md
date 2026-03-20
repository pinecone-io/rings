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

## F-061/F-062: User and Project Config Files

**Spec:** `specs/state/configuration.md`

**Summary:** Load user-level defaults from `~/.config/rings/config.toml` and project-level defaults from `.rings-config.toml` in the current directory. These provide defaults that CLI flags and workflow TOML override.

### Task 1: Config file loading

**Files:** `src/config.rs` (new), `src/main.rs`, `src/lib.rs`

**Steps:**
- [x] Create `src/config.rs` with a `RingsConfig` struct containing optional fields for all configurable defaults:
  - `default_output_dir: Option<String>`
  - `color: Option<bool>`
  - Additional fields can be added later
- [x] Implement `RingsConfig::load() -> Result<RingsConfig>` that:
  1. Checks for `.rings-config.toml` in the current directory
  2. Checks for `~/.config/rings/config.toml` (or `$XDG_CONFIG_HOME/rings/config.toml`)
  3. First found wins (project config takes precedence over user config)
  4. If neither exists, return empty defaults
- [x] Register `pub mod config;` in `src/lib.rs`
- [x] In `main.rs`, load config early and apply defaults before CLI flag processing

**Tests:**
- [x] `.rings-config.toml` in current dir is loaded
- [x] `~/.config/rings/config.toml` is loaded when no project config exists
- [x] Project config takes precedence over user config
- [x] Missing both config files returns empty defaults (no error)
- [x] Invalid TOML in config file produces clear error
- [x] `just validate` clean

---

## F-147: Disk Space Check

**Spec:** `specs/execution/engine.md` (Advisory Checks table)

**Summary:** At startup, check available disk space in the output directory. Warn at < 100 MB free, abort with exit 2 at < 10 MB. Prevents silently losing run data mid-execution.

### Task 1: Add disk space check at startup

**Files:** `src/main.rs` (or `src/engine.rs`)

**Steps:**
- [ ] After resolving the output directory, check available disk space using `fs2::available_space()` or `nix::sys::statvfs` (or a cross-platform alternative)
- [ ] If < 10 MB: print error `Error: Less than 10 MB free in output directory ({path}). Aborting to prevent data loss.` and exit 2
- [ ] If < 100 MB but >= 10 MB: print warning `⚠  Low disk space: only {N} MB free in output directory ({path}).`
- [ ] Only check in human output mode for warnings; the fatal < 10 MB check applies in all modes
- [ ] Use `#[cfg(unix)]` with `std::os::unix::fs::MetadataExt` or the `fs2` crate for portable disk space queries

**Tests:**
- [ ] Mock/temp filesystem with limited space triggers warning at < 100 MB
- [ ] Mock/temp filesystem with very low space triggers abort at < 10 MB
- [ ] Adequate disk space produces no warning
- [ ] `just validate` clean

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
