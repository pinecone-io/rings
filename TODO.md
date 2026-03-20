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

## F-184: Phase Cost Bar Chart

**Spec:** `specs/observability/runtime-output.md`

**Summary:** Completion and cancellation summaries show a proportional bar chart of cost distribution across phases. The `render_bar_chart` function already exists in `display.rs` — verify it's used in all summary paths.

### Task 1: Verify bar chart in all summary paths

**Files:** `src/display.rs`

**Steps:**
- [x] Verify `render_bar_chart` is called in `print_completion`
- [x] Verify `render_bar_chart` is called in `print_cancellation`
- [x] Verify the bar chart renders correctly with 1 phase, 2 phases, and 5+ phases
- [x] If already working in all paths, mark as COMPLETE

**Tests:**
- [x] Completion summary includes phase bar chart
- [x] Cancellation summary includes phase bar chart
- [x] Single-phase workflow shows full bar
- [x] `just validate` clean

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

## F-191: Model Name Display

**Spec:** `specs/observability/runtime-output.md`

**Summary:** The startup header shows the detected model name or "(default)" so the user always knows which model is being used. The `RunHeaderParams.model` field already exists — verify it's populated correctly.

### Task 1: Verify model name detection and display

**Files:** `src/display.rs`, `src/main.rs`, `src/workflow.rs`

**Steps:**
- [ ] Verify `Workflow::detect_model_name()` extracts the model from executor args (scans for `--model` flag)
- [ ] Verify the startup header displays the model name when detected
- [ ] Verify "(default)" is shown when no model flag is found
- [ ] If already working, mark as COMPLETE

**Tests:**
- [ ] Workflow with `args = ["--model", "claude-sonnet-4-6"]` shows "sonnet" or full model name in header
- [ ] Workflow with no `--model` flag shows "(default)"
- [ ] `just validate` clean

---

## F-186: Styled Startup Header

**Spec:** `specs/observability/runtime-output.md`

**Summary:** The startup header shows workflow details in a clean, labeled layout with semantic coloring. The `print_run_header` function already exists — verify it uses the color system consistently.

### Task 1: Verify styled header

**Files:** `src/display.rs`

**Steps:**
- [ ] Verify the startup header uses `style::dim` for labels, `style::bold` for the version line, `style::accent` for budget
- [ ] Verify the header includes: Workflow, Context, Phases, Model, Max, Budget (if set), Output
- [ ] Verify `--no-color` and `NO_COLOR` disable all styling
- [ ] If already working, mark as COMPLETE

**Tests:**
- [ ] Header contains all expected labels and values
- [ ] `NO_COLOR=1` produces plain text header with no ANSI
- [ ] `just validate` clean

---

## F-188: Styled List Table

**Spec:** `specs/observability/runtime-output.md`

**Summary:** `rings list` output uses color-coded status, bold headers, and accent cost figures.

### Task 1: Verify styled list output

**Files:** `src/main.rs` (list display section)

**Steps:**
- [ ] Verify `rings list` headers use `style::bold`
- [ ] Verify status values are color-coded: green for completed, red for failed, yellow for canceled/running
- [ ] Verify cost figures use `style::accent`
- [ ] If already working, mark as COMPLETE

**Tests:**
- [ ] List output with color enabled shows styled headers and status
- [ ] `NO_COLOR=1` produces plain text list
- [ ] `just validate` clean

---

## F-193/F-194/F-195: Context Dir Tracking and List Filtering

**Spec:** `specs/cli/commands-and-flags.md` (rings list section)

**Summary:** Store `context_dir` in `run.toml` metadata, add DIR column to `rings list`, and add `--dir` filter flag. Specs and inventory already updated — check if implementation was completed.

### Task 1: Verify context_dir in list display

**Files:** `src/state.rs`, `src/list.rs`, `src/main.rs`

**Steps:**
- [ ] Verify `RunMeta.context_dir` field exists and is populated on run start
- [ ] Verify `rings list` displays a DIR column
- [ ] Verify `--dir` filter works as substring match on context_dir
- [ ] If already working, mark as COMPLETE

**Tests:**
- [ ] `rings list` output includes DIR column
- [ ] `--dir /my/project` filters correctly
- [ ] JSONL output includes `context_dir` field
- [ ] `just validate` clean

---
