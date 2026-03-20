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

## F-144: Empty Context Directory Warning

**Spec:** `specs/execution/engine.md` (Advisory Checks table)

**Summary:** Warn at startup if `context_dir` contains zero files, since the executor will see nothing. This catches pointing at the wrong directory before spending money.

### Task 1: Add empty context_dir check

**Files:** `src/main.rs` (or `src/engine.rs`, wherever startup advisory checks run)

**Steps:**
- [x] After validating `context_dir` exists (F-143, already done), count entries via `std::fs::read_dir` — if 0 entries (excluding `.` and `..`), print warning
- [x] Warning text: `⚠  context_dir ("{path}") contains no files.\n   The executor will start with an empty working directory.\n   If this is intentional (the executor will create files from scratch), ignore this warning.`
- [x] Only warn in human output mode (suppress in JSONL mode — no advisory warnings on stdout)
- [x] Do not block execution — this is advisory only

**Tests:**
- [x] Empty context_dir triggers warning on stderr
- [x] Non-empty context_dir produces no warning
- [x] JSONL mode suppresses the warning
- [x] `just validate` clean

---

## F-145: Sensitive Files Warning

**Spec:** `specs/execution/engine.md` (Advisory Checks table)

**Summary:** Warn at startup if `context_dir` contains files matching credential patterns (`.env`, `*.key`, `*.pem`, etc.) that could be exposed to the model. Suppressible with `--no-sensitive-files-check` (F-096).

### Task 1: Add sensitive file detection

**Files:** `src/main.rs` (or `src/engine.rs`)

**Steps:**
- [ ] After context_dir validation, scan the top-level directory (non-recursive) for files matching credential patterns:
  - Exact names: `.env`, `.env.local`, `.env.production`, `.npmrc`, `.pypirc`
  - Extensions: `*.key`, `*.pem`, `*.p12`, `*.pfx`, `*.jks`, `*.keystore`
  - Patterns: `*credentials*`, `*secret*`, `*token*` (case-insensitive)
- [ ] If any matches found, print warning listing the matched filenames (max 10, then "... and N more")
- [ ] Warning text: `⚠  context_dir contains files that may contain credentials:\n   .env, server.key, credentials.json\n   These files will be visible to the executor. Use --no-sensitive-files-check to suppress.`
- [ ] Check the `--no-sensitive-files-check` flag — if set, skip this check entirely
- [ ] Only warn in human output mode

**Tests:**
- [ ] Directory with `.env` triggers warning
- [ ] Directory with `server.key` triggers warning
- [ ] Directory with no sensitive files produces no warning
- [ ] `--no-sensitive-files-check` suppresses the warning
- [ ] JSONL mode suppresses the warning
- [ ] `just validate` clean

---

### Task 2: Add `--no-sensitive-files-check` CLI flag (F-096)

**Files:** `src/cli.rs`, `src/main.rs`

**Steps:**
- [ ] Add `--no-sensitive-files-check` flag to `RunArgs`: `pub no_sensitive_files_check: bool`
- [ ] Pass it through to the advisory checks section and skip the sensitive files scan when set

**Tests:**
- [ ] `rings run --no-sensitive-files-check workflow.toml` parses correctly
- [ ] Flag suppresses the sensitive files warning
- [ ] `just validate` clean

---

## F-146: Output Directory Inside Repo Warning

**Spec:** `specs/execution/engine.md` (Advisory Checks table)

**Summary:** Warn if `output_dir` resolves to a path inside a git repository, since run logs and cost data could be accidentally committed.

### Task 1: Detect git repo above output_dir

**Files:** `src/main.rs` (or `src/engine.rs`)

**Steps:**
- [ ] After resolving the output directory path, walk up parent directories checking for a `.git` directory
- [ ] If found, print warning: `⚠  output_dir resolves to a path inside a git repository:\n   {output_dir} is under {repo_root}/ (which contains .git)\n   rings run logs and cost data will be written here and may be accidentally committed.\n   Consider adding {relative_output_dir}/ to .gitignore, or omit output_dir to use the default\n   off-repo location (~/.local/share/rings/runs/).`
- [ ] Only warn when the user explicitly set `output_dir` (via CLI `--output-dir` or TOML) — skip when using the default `~/.local/share/rings/runs/` path
- [ ] Only warn in human output mode

**Tests:**
- [ ] Output dir inside a git repo triggers warning
- [ ] Output dir outside any git repo produces no warning
- [ ] Default output dir (no explicit setting) skips the check
- [ ] JSONL mode suppresses the warning
- [ ] `just validate` clean

---

## F-148: Delay Sanity Warning

**Spec:** `specs/execution/engine.md` (Advisory Checks table)

**Summary:** Warn if `delay_between_runs` exceeds 600 seconds, since that's likely a units mistake (user probably meant milliseconds or minutes).

### Task 1: Add delay sanity check

**Files:** `src/main.rs` (or `src/engine.rs`)

**Steps:**
- [ ] After resolving the final `delay_between_runs` value (CLI override or TOML), check if it exceeds 600
- [ ] If so, print warning: `⚠  delay_between_runs = {N} seconds ({human_readable}) between each run.\n   This is unusually long. If you meant {N} milliseconds, rings uses whole seconds.\n   Use --delay to override for this run without editing the workflow file.`
- [ ] Format the human-readable duration (e.g., "15 minutes", "1 hour 30 minutes")
- [ ] Only warn in human output mode

**Tests:**
- [ ] `delay_between_runs = 900` triggers warning mentioning "15 minutes"
- [ ] `delay_between_runs = 600` does NOT trigger warning (threshold is >600)
- [ ] `delay_between_runs = 30` does NOT trigger warning
- [ ] JSONL mode suppresses the warning
- [ ] `just validate` clean

---

## F-029: Unknown Template Variable Warnings

**Spec:** `specs/execution/prompt-templating.md`

**Summary:** Warn at startup if prompts reference `{{variables}}` that rings doesn't recognize. Catches typos before any Claude calls happen.

### Task 1: Scan prompts for unknown variables

**Files:** `src/template.rs`, `src/main.rs` (or `src/engine.rs`)

**Steps:**
- [ ] Define the set of known template variables: `phase_name`, `cycle`, `max_cycles`, `iteration`, `run`, `cost_so_far_usd`
- [ ] After loading all prompts (inline and file-based), scan each for `{{...}}` patterns using a regex like `\{\{(\w+)\}\}`
- [ ] For each match, check if the variable name is in the known set
- [ ] Collect all unknown variable names with their source (prompt file path or "inline in phase X")
- [ ] Print warning: `⚠  Unknown template variable(s) in prompts:\n   {{typo_var}} in prompts/builder.md\n   {{unknown}} in phase "reviewer" (inline)\n   Known variables: {{phase_name}}, {{cycle}}, {{max_cycles}}, {{iteration}}, {{run}}, {{cost_so_far_usd}}`
- [ ] Only warn in human output mode
- [ ] This is advisory only — do not block execution (the unknown variables are left as literal text)

**Tests:**
- [ ] Prompt with `{{unknown_var}}` triggers warning listing the variable
- [ ] Prompt with only known variables produces no warning
- [ ] Multiple unknown variables across multiple prompts are all reported
- [ ] Warning includes the source file/phase for each unknown variable
- [ ] JSONL mode suppresses the warning
- [ ] `just validate` clean

---

## Bug: `--dry-run --output-format jsonl` Emits Human Output Instead of JSONL

**Ref:** `specs/cli/commands-and-flags.md` lines 38–40

**Summary:** The spec says `--dry-run` is "compatible with `--output-format jsonl`: emits a `dry_run_plan` event containing the plan as structured JSON, suitable for CI workflow validation." However, `run_inner()` in `main.rs` (lines 148–231) always emits human-formatted output regardless of `output_format`. The `DryRunPlan` struct in `dry_run.rs` already derives `Serialize`, so the data is ready — only the JSONL emission path is missing.

### Task 1: Add JSONL output path for dry-run

**Files:** `src/main.rs`

**Steps:**
- [ ] In the `if args.dry_run` block (line 148), check `output_format`:
  - If `Jsonl`: serialize the `DryRunPlan` as a JSON event with `"event": "dry_run_plan"` and print to stdout, then return `Ok(0)`
  - If `Human`: keep the existing human-formatted output (lines 152–228)
- [ ] The JSONL event should include all plan fields: phases, completion signal checks, unknown variables, max total runs
- [ ] Use `serde_json::to_string` on the existing `DryRunPlan` struct (already `Serialize`)

**Tests:**
- [ ] `--dry-run --output-format jsonl` emits a single JSON line with `"event": "dry_run_plan"`
- [ ] The emitted JSON contains `phases`, `max_cycles`, `completion_signal`, and `max_total_runs`
- [ ] `--dry-run` without `--output-format jsonl` still emits human-readable output (regression)
- [ ] `just validate` clean

---
