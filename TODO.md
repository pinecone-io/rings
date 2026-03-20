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

## F-154: Large Context Directory Warning

**Spec:** `specs/observability/file-lineage.md`

**Summary:** Warn if context_dir has > 10,000 files because manifest scanning will be slow. Advisory only.

### Task 1: Add large directory warning

**Files:** `src/main.rs` (or `src/engine.rs`)

**Steps:**
- [x] After context_dir validation but before engine start, count files in context_dir (recursive)
- [x] If count > 10,000: print warning `⚠  context_dir contains {N} files. Manifest scanning may be slow.\n   Consider using manifest_ignore patterns to exclude large directories (e.g., node_modules/, target/).`
- [x] Only warn when `manifest_enabled = true` (no point warning if manifests are off)
- [x] Only warn in human output mode

**Tests:**
- [x] Directory with > 10,000 files triggers warning
- [x] Directory with < 10,000 files produces no warning
- [x] Warning suppressed when manifest_enabled is false
- [x] `just validate` clean

---

## F-183: ANSI Color System

**Spec:** `specs/observability/runtime-output.md` (Visual Enhancement section)

**Summary:** Use a semantic color palette (green success, red errors, cyan costs, dim chrome) gated behind NO_COLOR env var and TTY detection. The `style.rs` module already exists with color helpers — verify it's complete and consistent.

### Task 1: Verify and complete color system

**Files:** `src/style.rs`

**Steps:**
- [ ] Verify the semantic color functions exist: `success()`, `error()`, `warn()`, `accent()`, `dim()`, `muted()`, `bold()`
- [ ] Verify NO_COLOR environment variable disables all ANSI codes
- [ ] Verify non-TTY stderr disables colors
- [ ] Verify `--no-color` CLI flag disables colors
- [ ] If all above are working, mark as COMPLETE after verification

**Tests:**
- [ ] `NO_COLOR=1` environment variable disables all ANSI escapes
- [ ] Non-TTY output contains no ANSI escapes
- [ ] `--no-color` flag disables colors
- [ ] Colors are applied correctly in TTY mode
- [ ] `just validate` clean

---

## F-184: Phase Cost Bar Chart

**Spec:** `specs/observability/runtime-output.md`

**Summary:** Completion and cancellation summaries show a proportional bar chart of cost distribution across phases. The `render_bar_chart` function already exists in `display.rs` — verify it's used in all summary paths.

### Task 1: Verify bar chart in all summary paths

**Files:** `src/display.rs`

**Steps:**
- [ ] Verify `render_bar_chart` is called in `print_completion`
- [ ] Verify `render_bar_chart` is called in `print_cancellation`
- [ ] Verify the bar chart renders correctly with 1 phase, 2 phases, and 5+ phases
- [ ] If already working in all paths, mark as COMPLETE

**Tests:**
- [ ] Completion summary includes phase bar chart
- [ ] Cancellation summary includes phase bar chart
- [ ] Single-phase workflow shows full bar
- [ ] `just validate` clean

---

## F-185: Budget Gauge

**Spec:** `specs/observability/runtime-output.md`

**Summary:** When a budget cap is configured, summaries show a visual gauge of budget consumption with color-coded thresholds. The `render_budget_gauge` function already exists — verify it's used.

### Task 1: Verify budget gauge in summaries

**Files:** `src/display.rs`

**Steps:**
- [ ] Verify `render_budget_gauge` is called in `print_completion` when `budget_cap_usd` is set
- [ ] Verify it's called in `print_cancellation` when budget cap is set
- [ ] Verify color thresholds: green < 60%, yellow 60-85%, red > 85%
- [ ] If already working, mark as COMPLETE

**Tests:**
- [ ] Summary with budget cap shows gauge
- [ ] Summary without budget cap omits gauge
- [ ] Gauge colors change at threshold boundaries
- [ ] `just validate` clean

---

## F-190: Cumulative Token Display

**Spec:** `specs/observability/runtime-output.md`

**Summary:** The status line and summaries show cumulative input/output token counts that update after each completed run.

### Task 1: Verify token display

**Files:** `src/display.rs`, `src/engine.rs`

**Steps:**
- [ ] Verify the status line includes token counts when available (already done in `format_status_line`)
- [ ] Verify completion and cancellation summaries include token totals
- [ ] Verify tokens are accumulated correctly across runs in `BudgetTracker`
- [ ] If already working, mark as COMPLETE

**Tests:**
- [ ] Status line shows `18.2k in · 4.1k out` when tokens are non-zero
- [ ] Status line omits token segment when both are zero
- [ ] Completion summary includes token totals
- [ ] `just validate` clean

---
