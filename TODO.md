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

## F-035/F-036: Parse Warning Summary and Deduplication

**Spec:** `specs/execution/output-parsing.md`

**Summary:** At the end of a run, show a consolidated summary of any cost parsing failures with raw output snippets (F-035). Repeated parse failures for the same pattern are collapsed into a count instead of flooding the terminal (F-036).

### Task 2: Warning deduplication (F-036)

**Files:** `src/display.rs`

**Steps:**
- [x] In `print_parse_warnings`, group warnings by `(confidence, raw_match pattern)` before printing
- [x] If multiple warnings have the same pattern, print once with a count: `⚠  Low-confidence cost parse (×3): Run 5, 8, 11 (cycle 2, phase builder): $0.XX`
- [x] Keep the max 10 display limit but count deduplicated groups, not individual warnings

**Tests:**
- [x] 5 warnings with identical raw_match pattern produce 1 line with `(×5)`
- [x] Mixed patterns still show each unique pattern separately
- [x] `just validate` clean

---

## F-181: Per-Phase Model Selection via `executor.extra_args`

**Spec:** `specs/execution/executor-integration.md` (Per-Phase Model Selection section)

**Summary:** Allow phases to append extra executor args (e.g., `--model claude-haiku-4-5`) without re-specifying the full base args. The `extra_args` list is appended to the inherited `executor.args`.

### Task 1: Add `extra_args` to phase executor config

**Files:** `src/workflow.rs`, `src/engine.rs`, `src/executor.rs`

**Steps:**
- [ ] Add `extra_args: Vec<String>` field (with `#[serde(default)]`) to the phase-level executor config in `workflow.rs`
- [ ] Add `extra_args: Vec<String>` to the workflow-level `[executor]` config as well (phase inherits, can override)
- [ ] In the engine, when building the effective args for a phase's executor invocation: append `extra_args` after `args`
- [ ] At startup validation: if both `args` and `extra_args` contain `--model`, emit a configuration error (exit 2) — conflicting double model flag

**Tests:**
- [ ] Phase with `extra_args = ["--model", "claude-haiku-4-5"]` produces effective args with model appended
- [ ] Phase without `extra_args` inherits workflow-level `extra_args` if present
- [ ] Phase-level `extra_args` replaces (not appends to) workflow-level `extra_args`
- [ ] Duplicate `--model` in both `args` and `extra_args` exits 2
- [ ] `just validate` clean

---

## F-049: Resume State Recovery from costs.jsonl

**Spec:** `specs/state/cancellation-resume.md` (State Recovery section)

**Summary:** If `state.json` is corrupted or unreadable on resume, attempt to reconstruct the execution position from `costs.jsonl` before failing. This prevents losing all progress due to a single corrupted file.

### Task 1: Add state recovery fallback in resume path

**Files:** `src/main.rs` (in `resume_inner`), `src/state.rs`

**Steps:**
- [ ] In `resume_inner`, when `StateFile::read(&state_path)` fails:
  1. Attempt recovery: scan `costs.jsonl` for the highest completed run number
  2. If `costs.jsonl` is readable with at least one entry, reconstruct minimal state: `last_completed_run` = max run number, derive cycle/phase position from run count and workflow structure
  3. Print warning: `Warning: state.json was unreadable; state reconstructed from costs.jsonl.\n   Recovered to run N. If this is incorrect, start a new run with --parent-run to preserve ancestry.`
  4. Continue resume from the reconstructed position
- [ ] If `costs.jsonl` is also unreadable, exit with code 2 and print both file paths for manual inspection
- [ ] Add a `recover_state_from_costs(costs_path: &Path, workflow: &Workflow) -> Result<StateFile>` function to `src/state.rs`

**Tests:**
- [ ] Resume with corrupted state.json but valid costs.jsonl: recovers and prints warning
- [ ] Resume with both files corrupted: exits 2 with clear error listing both paths
- [ ] Recovered position matches expected cycle/phase/iteration based on costs.jsonl entries
- [ ] Resume with valid state.json: normal path, no recovery attempted
- [ ] `just validate` clean

---

## F-031: Custom Cost Parser

**Spec:** `specs/execution/output-parsing.md`

**Summary:** Allow users to provide a custom regex to extract cost from non-standard executor output formats. This enables cost tracking for executors that don't use Claude Code's JSON format.

### Task 1: Add custom cost_parser support

**Files:** `src/workflow.rs`, `src/cost.rs`

**Steps:**
- [ ] In the `[executor]` config, the `cost_parser` field already accepts `"claude-code"` and `"none"` — add support for a custom regex string
- [ ] When `cost_parser` is a string that is not `"claude-code"` or `"none"`, treat it as a regex pattern with a named capture group `cost` (e.g., `"Cost: \\$(?P<cost>[\\d.]+)"`)
- [ ] Compile the regex at workflow parse time; emit exit 2 if invalid
- [ ] In `parse_cost_from_output`, if a custom parser is configured, try it before the built-in patterns
- [ ] Custom parser match → `ParseConfidence::Full` if `cost` group captured, `ParseConfidence::Partial` otherwise

**Tests:**
- [ ] Custom regex `"Total: \\$(?P<cost>[\\d.]+)"` extracts cost from `"Total: $1.23"` output
- [ ] Invalid regex in `cost_parser` exits 2 at workflow parse time
- [ ] `cost_parser = "none"` still returns zero cost with no parsing
- [ ] `cost_parser = "claude-code"` (or absent) still uses built-in parser
- [ ] Custom parser with no match falls through to built-in patterns
- [ ] `just validate` clean

---

## F-097/F-098/F-101: Inspect Views — Summary, Cycles, Costs

**Spec:** `specs/cli/inspect-command.md`

**Summary:** Implement the three most useful inspect views. `--show summary` is already done (F-071). Add `--show cycles` (per-cycle breakdown) and `--show costs` (detailed per-run cost/token table).

### Task 1: Implement `--show cycles` view

**Files:** `src/inspect.rs`, `src/main.rs`

**Steps:**
- [ ] In `inspect_inner`, handle `InspectView::Cycles`:
  1. Read `costs.jsonl` → group entries by cycle number
  2. For each cycle, show: cycle number, runs in that cycle (phase name, cost, duration, completion signal status)
  3. Show cycle subtotal cost
- [ ] Support `--cycle <N>` filter to show only a specific cycle
- [ ] In JSONL mode, emit one JSON object per cycle

**Tests:**
- [ ] `rings inspect <id> --show cycles` displays per-cycle breakdown
- [ ] `--cycle 2` filters to only cycle 2
- [ ] Cycles with no cost data show "—" for cost
- [ ] JSONL mode emits structured cycle data
- [ ] `just validate` clean

---

### Task 2: Implement `--show costs` view

**Files:** `src/inspect.rs`

**Steps:**
- [ ] In `inspect_inner`, handle `InspectView::Costs`:
  1. Read `costs.jsonl` → display a table of all runs
  2. Columns: Run #, Cycle, Phase, Cost USD, Input Tokens, Output Tokens, Confidence, Duration
  3. Show totals row at bottom
- [ ] Support `--phase <NAME>` filter to show only a specific phase's costs
- [ ] In JSONL mode, emit one JSON object per run

**Tests:**
- [ ] `rings inspect <id> --show costs` displays per-run cost table
- [ ] `--phase builder` filters to only builder phase runs
- [ ] Totals row sums cost and tokens correctly
- [ ] JSONL mode emits structured cost data per run
- [ ] `just validate` clean

---
