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

### Task 1: Consolidated parse warning summary

**Files:** `src/display.rs`, `src/engine.rs`

**Steps:**
- [x] The `print_parse_warnings` function in `display.rs` already exists and shows up to 10 warnings — verify it's called at the appropriate place in the engine (end of run, after all phases complete)
- [x] Ensure each warning includes: run number, cycle, phase, confidence level, and a raw output snippet (first 100 chars of the matched text or "no match")
- [x] If already fully implemented, mark this as COMPLETE after verification

**Tests:**
- [x] Multiple low-confidence runs produce a single consolidated summary at run end
- [x] Summary shows at most 10 individual warnings, then "... and N more"
- [x] `just validate` clean

---

### Task 2: Warning deduplication (F-036)

**Files:** `src/display.rs`

**Steps:**
- [ ] In `print_parse_warnings`, group warnings by `(confidence, raw_match pattern)` before printing
- [ ] If multiple warnings have the same pattern, print once with a count: `⚠  Low-confidence cost parse (×3): Run 5, 8, 11 (cycle 2, phase builder): $0.XX`
- [ ] Keep the max 10 display limit but count deduplicated groups, not individual warnings

**Tests:**
- [ ] 5 warnings with identical raw_match pattern produce 1 line with `(×5)`
- [ ] Mixed patterns still show each unique pattern separately
- [ ] `just validate` clean

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
