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

## F-031: Custom Cost Parser

**Spec:** `specs/execution/output-parsing.md`

**Summary:** Allow users to provide a custom regex to extract cost from non-standard executor output formats. This enables cost tracking for executors that don't use Claude Code's JSON format.

### Task 1: Add custom cost_parser support

**Files:** `src/workflow.rs`, `src/cost.rs`

**Steps:**
- [x] In the `[executor]` config, the `cost_parser` field already accepts `"claude-code"` and `"none"` — add support for a custom regex string
- [x] When `cost_parser` is a string that is not `"claude-code"` or `"none"`, treat it as a regex pattern with a named capture group `cost` (e.g., `"Cost: \\$(?P<cost>[\\d.]+)"`)
- [x] Compile the regex at workflow parse time; emit exit 2 if invalid
- [x] In `parse_cost_from_output`, if a custom parser is configured, try it before the built-in patterns
- [x] Custom parser match → `ParseConfidence::Full` if `cost` group captured, `ParseConfidence::Partial` otherwise

**Tests:**
- [x] Custom regex `"Total: \\$(?P<cost>[\\d.]+)"` extracts cost from `"Total: $1.23"` output
- [x] Invalid regex in `cost_parser` exits 2 at workflow parse time
- [x] `cost_parser = "none"` still returns zero cost with no parsing
- [x] `cost_parser = "claude-code"` (or absent) still uses built-in parser
- [x] Custom parser with no match falls through to built-in patterns
- [x] `just validate` clean

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
