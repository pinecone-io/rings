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

## F-029: Unknown Template Variable Startup Warning (User-Visible)

**Spec:** `specs/execution/prompt-templating.md` (Unknown Variables section)

**Summary:** The spec says unknown template variables should produce a **startup advisory warning** visible to the user before any Claude calls. The detection logic already exists (`template::find_unknown_variables`) and is used in two places: (1) `dry_run.rs` collects them for the dry-run plan, and (2) `engine.rs:835-852` logs them to `events.jsonl` at runtime. However, neither path prints a warning to stderr during startup in `run_inner()`, so users doing a normal `rings run` never see the warning — they'd have to inspect `events.jsonl` or use `--dry-run` to discover it.

### Task 1: Add startup unknown variable warning in run_inner

**Files:** `src/main.rs`

**Steps:**
- [x] After loading all phase prompts (inline and file-based) but before entering the engine, scan each prompt for unknown variables using `template::find_unknown_variables(&prompt, template::KNOWN_VARS)`
- [x] Collect results as `(phase_name, prompt_source, Vec<String>)` tuples
- [x] If any unknowns found and `output_format == Human`, print warning to stderr:
  ```
  ⚠  Unknown template variable(s) in prompts:
     {{typo_var}} in phase "builder" (inline)
     {{custom}} in phase "reviewer" (prompts/review.md)
     Known variables: {{phase_name}}, {{cycle}}, {{max_cycles}}, {{iteration}}, {{run}}, {{cost_so_far_usd}}
  ```
- [x] This is advisory only — do not block execution
- [x] Reuse the existing `template::find_unknown_variables` function and `template::KNOWN_VARS` constant

**Tests:**
- [x] Prompt with `{{unknown_var}}` triggers visible warning on stderr
- [x] Prompt with only known variables produces no warning
- [x] JSONL mode suppresses the stderr warning
- [x] `just validate` clean

---
