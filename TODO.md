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

## F-097/F-098/F-101: Inspect Views — Summary, Cycles, Costs

**Spec:** `specs/cli/inspect-command.md`

**Summary:** Implement the three most useful inspect views. `--show summary` is already done (F-071). Add `--show cycles` (per-cycle breakdown) and `--show costs` (detailed per-run cost/token table).

### Task 2: Implement `--show costs` view

**Files:** `src/inspect.rs`

**Steps:**
- [x] In `inspect_inner`, handle `InspectView::Costs`:
  1. Read `costs.jsonl` → display a table of all runs
  2. Columns: Run #, Cycle, Phase, Cost USD, Input Tokens, Output Tokens, Confidence, Duration
  3. Show totals row at bottom
- [x] Support `--phase <NAME>` filter to show only a specific phase's costs
- [x] In JSONL mode, emit one JSON object per run

**Tests:**
- [x] `rings inspect <id> --show costs` displays per-run cost table
- [x] `--phase builder` filters to only builder phase runs
- [x] Totals row sums cost and tokens correctly
- [x] JSONL mode emits structured cost data per run
- [x] `just validate` clean

---

## F-102: Inspect Claude Output View

**Spec:** `specs/cli/inspect-command.md` (--show claude-output section)

**Summary:** `rings inspect <RUN_ID> --show claude-output` prints the captured stdout/stderr from each executor invocation, with run headers. Supports `--cycle N` and `--phase NAME` filters.

### Task 1: Implement `--show claude-output` view

**Files:** `src/inspect.rs`

**Steps:**
- [ ] In `inspect_inner`, handle `InspectView::ClaudeOutput`:
  1. Scan the `runs/` subdirectory for log files (named like `001.log`, `002.log`, etc.)
  2. For each log file, print a header with run number, then the file contents
  3. Support `--cycle N` and `--phase NAME` filters: need to cross-reference with `costs.jsonl` to map run numbers to cycles/phases
- [ ] In JSONL mode, emit one JSON object per run with the log content as a string field
- [ ] Handle missing log files gracefully (print "log not found" for that run)

**Tests:**
- [ ] `rings inspect <id> --show claude-output` displays log contents with run headers
- [ ] `--cycle 1` filters to only cycle 1 runs
- [ ] `--phase builder` filters to only builder phase runs
- [ ] Missing log file produces a graceful message, not an error
- [ ] JSONL mode emits structured output per run
- [ ] `just validate` clean

---

## F-073: `rings lineage` — Ancestry Chain Display

**Spec:** `specs/cli/inspect-command.md` (rings lineage section)

**Summary:** `rings lineage <RUN_ID>` traverses the ancestry chain (parent_run_id links) and displays the full history of related runs with aggregate totals. Currently a stub.

### Task 1: Implement lineage traversal and display

**Files:** `src/main.rs`, `src/list.rs` (or new `src/lineage.rs`)

**Steps:**
- [ ] In `cmd_lineage`, replace the stub with real implementation:
  1. Load `run.toml` for the given run ID
  2. Walk backwards via `parent_run_id` / `continuation_of` to find the root run
  3. Walk forwards from root: scan all run directories for runs whose `parent_run_id` or `continuation_of` matches each chain member
  4. For each run in the chain, load status, cycles, cost from `run.toml` and `state.json`
- [ ] Display the chain as a numbered table (see spec for format): `#, RUN_ID, DATE, STATUS, CYCLES, COST` with relationship indicators
- [ ] Show chain totals at bottom: total wall time, total cycles, total runs, total cost
- [ ] In JSONL mode: emit one JSON object per run, then a `chain_summary` object
- [ ] Handle broken chains gracefully (missing parent run directory → show "parent not found" and stop traversal)

**Tests:**
- [ ] Single run with no parent shows just itself
- [ ] Chain of 3 runs (root → resumed → resumed) displays all 3 with correct relationships
- [ ] Chain totals sum correctly across all runs
- [ ] Broken chain (missing parent dir) shows partial chain with warning
- [ ] JSONL mode emits correct structured output
- [ ] `just validate` clean

---

## F-061/F-062: User and Project Config Files

**Spec:** `specs/state/configuration.md`

**Summary:** Load user-level defaults from `~/.config/rings/config.toml` and project-level defaults from `.rings-config.toml` in the current directory. These provide defaults that CLI flags and workflow TOML override.

### Task 1: Config file loading

**Files:** `src/config.rs` (new), `src/main.rs`, `src/lib.rs`

**Steps:**
- [ ] Create `src/config.rs` with a `RingsConfig` struct containing optional fields for all configurable defaults:
  - `default_output_dir: Option<String>`
  - `color: Option<bool>`
  - Additional fields can be added later
- [ ] Implement `RingsConfig::load() -> Result<RingsConfig>` that:
  1. Checks for `.rings-config.toml` in the current directory
  2. Checks for `~/.config/rings/config.toml` (or `$XDG_CONFIG_HOME/rings/config.toml`)
  3. First found wins (project config takes precedence over user config)
  4. If neither exists, return empty defaults
- [ ] Register `pub mod config;` in `src/lib.rs`
- [ ] In `main.rs`, load config early and apply defaults before CLI flag processing

**Tests:**
- [ ] `.rings-config.toml` in current dir is loaded
- [ ] `~/.config/rings/config.toml` is loaded when no project config exists
- [ ] Project config takes precedence over user config
- [ ] Missing both config files returns empty defaults (no error)
- [ ] Invalid TOML in config file produces clear error
- [ ] `just validate` clean

---
