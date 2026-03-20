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

## Bug: `--dry-run --output-format jsonl` Emits Human Output Instead of JSONL

**Ref:** `specs/cli/commands-and-flags.md` lines 38–40

**Summary:** The spec says `--dry-run` is "compatible with `--output-format jsonl`: emits a `dry_run_plan` event containing the plan as structured JSON, suitable for CI workflow validation." However, `run_inner()` in `main.rs` (lines 148–231) always emits human-formatted output regardless of `output_format`. The `DryRunPlan` struct in `dry_run.rs` already derives `Serialize`, so the data is ready — only the JSONL emission path is missing.

### Task 1: Add JSONL output path for dry-run

**Files:** `src/main.rs`

**Steps:**
- [x] In the `if args.dry_run` block (line 148), check `output_format`:
  - If `Jsonl`: serialize the `DryRunPlan` as a JSON event with `"event": "dry_run_plan"` and print to stdout, then return `Ok(0)`
  - If `Human`: keep the existing human-formatted output (lines 152–228)
- [x] The JSONL event should include all plan fields: phases, completion signal checks, unknown variables, max total runs
- [x] Use `serde_json::to_string` on the existing `DryRunPlan` struct (already `Serialize`)

**Tests:**
- [x] `--dry-run --output-format jsonl` emits a single JSON line with `"event": "dry_run_plan"`
- [x] The emitted JSON contains `phases`, `max_cycles`, `completion_signal`, and `max_total_runs`
- [x] `--dry-run` without `--output-format jsonl` still emits human-readable output (regression)
- [x] `just validate` clean

---

## F-029: Unknown Template Variable Startup Warning (User-Visible)

**Spec:** `specs/execution/prompt-templating.md` (Unknown Variables section)

**Summary:** The spec says unknown template variables should produce a **startup advisory warning** visible to the user before any Claude calls. The detection logic already exists (`template::find_unknown_variables`) and is used in two places: (1) `dry_run.rs` collects them for the dry-run plan, and (2) `engine.rs:835-852` logs them to `events.jsonl` at runtime. However, neither path prints a warning to stderr during startup in `run_inner()`, so users doing a normal `rings run` never see the warning — they'd have to inspect `events.jsonl` or use `--dry-run` to discover it.

### Task 1: Add startup unknown variable warning in run_inner

**Files:** `src/main.rs`

**Steps:**
- [ ] After loading all phase prompts (inline and file-based) but before entering the engine, scan each prompt for unknown variables using `template::find_unknown_variables(&prompt, template::KNOWN_VARS)`
- [ ] Collect results as `(phase_name, prompt_source, Vec<String>)` tuples
- [ ] If any unknowns found and `output_format == Human`, print warning to stderr:
  ```
  ⚠  Unknown template variable(s) in prompts:
     {{typo_var}} in phase "builder" (inline)
     {{custom}} in phase "reviewer" (prompts/review.md)
     Known variables: {{phase_name}}, {{cycle}}, {{max_cycles}}, {{iteration}}, {{run}}, {{cost_so_far_usd}}
  ```
- [ ] This is advisory only — do not block execution
- [ ] Reuse the existing `template::find_unknown_variables` function and `template::KNOWN_VARS` constant

**Tests:**
- [ ] Prompt with `{{unknown_var}}` triggers visible warning on stderr
- [ ] Prompt with only known variables produces no warning
- [ ] JSONL mode suppresses the stderr warning
- [ ] `just validate` clean

---
