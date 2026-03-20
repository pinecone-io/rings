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

## Bug: Custom Cost Parser Bypasses Negative/NaN/Infinity Validation

**Ref:** `specs/execution/output-parsing.md`

**Summary:** The built-in cost parser validates parsed costs via `is_valid_cost()` (rejects negative, NaN, Infinity — added in commit f35a280). However, the custom cost parser path (`CompiledCostParser::Custom` in `cost.rs:233-269`) parses `cost_usd` with a raw `.parse::<f64>().ok()` at line 238, bypassing this validation. A custom regex like `(?P<cost_usd>[-\d.]+)` matching executor output `"Total: $-5.00"` would produce `cost_usd: Some(-5.0)`, which subtracts from cumulative cost and can bypass budget caps — the same class of bug that was fixed for the built-in parser.

### Task 1: Apply cost validation to custom parser path

**Files:** `src/cost.rs`

**Steps:**
- [ ] After parsing `cost_usd` at line 238, apply the same `is_valid_cost()` check used in the built-in paths
- [ ] If the parsed value fails validation (negative, NaN, Infinity): set `cost_usd = None` and `confidence = ParseConfidence::None`, same as the built-in parser behavior
- [ ] Reuse the existing `validated_cost()` helper or call `is_valid_cost()` directly

**Tests:**
- [ ] Custom parser matching `"-5.00"` returns `confidence: None`, `cost_usd: None`
- [ ] Custom parser matching `"NaN"` returns `confidence: None`, `cost_usd: None`
- [ ] Custom parser matching `"1.23"` (valid) still works normally
- [ ] `just validate` clean

---

## F-097/F-098/F-101: Inspect Views — Summary, Cycles, Costs

**Spec:** `specs/cli/inspect-command.md`

**Summary:** Implement the three most useful inspect views. `--show summary` is already done (F-071). Add `--show cycles` (per-cycle breakdown) and `--show costs` (detailed per-run cost/token table).

### Task 1: Implement `--show cycles` view

**Files:** `src/inspect.rs`, `src/main.rs`

**Steps:**
- [x] In `inspect_inner`, handle `InspectView::Cycles`:
  1. Read `costs.jsonl` → group entries by cycle number
  2. For each cycle, show: cycle number, runs in that cycle (phase name, cost, duration, completion signal status)
  3. Show cycle subtotal cost
- [x] Support `--cycle <N>` filter to show only a specific cycle
- [x] In JSONL mode, emit one JSON object per cycle

**Tests:**
- [x] `rings inspect <id> --show cycles` displays per-cycle breakdown
- [x] `--cycle 2` filters to only cycle 2
- [x] Cycles with no cost data show "—" for cost
- [x] JSONL mode emits structured cycle data
- [x] `just validate` clean

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
