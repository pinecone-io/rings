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

## F-082/F-083: Step-Through Mode

**Spec:** `specs/cli/commands-and-flags.md` lines 43–48, `specs/observability/runtime-output.md` (F-136/137/138)

**Summary:** `--step` pauses after every individual run, showing a summary and waiting for confirmation. `--step-cycles` pauses only at cycle boundaries. The CLI flags already exist but the pause logic is not wired in the engine.

### Task 1: Add step-through pause logic to engine

**Files:** `src/engine.rs`, `src/display.rs`

**Steps:**
- [ ] Add `step: bool` and `step_cycles: bool` fields to `EngineConfig`
- [ ] Pass `args.step` and `args.step_cycles` through from `run_inner` in `main.rs`
- [ ] After each completed run (after cost parsing, before next run): if `step` is true and stderr is a TTY:
  1. Print step summary: cost of this run, cumulative cost, whether completion signal was detected
  2. Prompt: `[c]ontinue, [s]kip cycle, [q]uit > `
  3. Read a single character from stdin
  4. `c` or Enter: continue to next run
  5. `s`: skip remaining runs in this cycle, advance to next cycle
  6. `q`: trigger normal cancellation flow (save state, print resume command)
- [ ] For `step_cycles`: same logic but only prompt at cycle boundaries (after all phases in a cycle complete), not after every run
- [ ] Non-TTY: `--step` and `--step-cycles` are silently ignored (no pausing)
- [ ] Already have: `--step` + `--output-format jsonl` conflict check (exits 2)

**Tests:**
- [ ] `--step` with mock stdin `c\nc\nq\n`: runs 2 runs then quits with cancellation
- [ ] `--step` with mock stdin `s\n`: skips remaining runs in cycle
- [ ] `--step-cycles` only pauses at cycle boundaries, not between runs within a cycle
- [ ] Non-TTY mode: `--step` runs without pausing
- [ ] Step summary shows cost and completion signal status
- [ ] `just validate` clean

---
