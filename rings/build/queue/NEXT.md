## Batch: Resilience, Discovery & Safety Feedback — 2026-03-16

**Features:** F-081 (--dry-run), F-056 (Stale Lock Detection), F-070 (rings list), F-050 (Workflow File Change Detection), F-029 (Unknown Variable Warnings), F-149 (Cost Spike Detection), F-049 (Resume State Recovery), F-053 (Double Ctrl+C)

---

### Task 1: Engine Refactor + Critical Bug Fixes (Prerequisite)

**Files:** `src/engine.rs`, `src/executor.rs`, `src/lock.rs`, `src/state.rs`, `src/audit.rs`

Extract mutable state from the `run_workflow` monolith and fix two confirmed bugs before any feature work begins.

**Engine refactor:**
- Define `BudgetTracker` struct (in `src/engine.rs` or `src/budget.rs`) owning: `phase_costs: HashMap<String, f64>`, `cumulative_cost: f64`, budget warning flags (`HashMap<String, bool>`), and per-phase rolling windows (`rolling_windows: HashMap<String, VecDeque<f64>>`, cap 5 per phase — see Open Decision OD-1).
- Define `RunContext` struct owning all remaining mutable loop state: `total_runs`, `last_cycle`, `last_successful_run`, `current_display_cycle`, `parse_warnings: Vec<ParseWarning>`, and a `BudgetTracker`.
- Create `make_state_snapshot(ctx: &RunContext, spec: &RunSpec, reason: ExitReason) -> StateFile` to replace the six near-identical `StateFile { ... }` construction sites in `engine.rs`. The cancellation path and success path currently diverge on `last_completed_run`; the helper must unify them.
- Create `save_state(ctx: &RunContext, spec: &RunSpec, reason: ExitReason) -> Result<()>` that calls `make_state_snapshot` then `write_atomic`.

**F-053 bug fix (blocker):**
- The grace-period inner loop (`engine.rs` lines ~372–389) polls `handle.try_wait()` every 100ms but never re-checks `cancel_state.is_force_kill()`. Add `if cancel_state.is_force_kill() { let _ = handle.send_sigkill(); break; }` as the first statement inside both grace-period inner loops (cancellation path and timeout path). Without this fix the entire F-053 feature is non-functional.

**`lock.rs` `unwrap()` fix (blocker per CLAUDE.md):**
- `src/lock.rs` ~line 75: `serde_json::to_string(&lock_data).unwrap()` must become `serde_json::to_string(&lock_data).map_err(|e| LockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?`

**`write_atomic` temp-file collision fix (blocker):**
- `StateFile::write_atomic` uses `path.with_extension("tmp")`, producing a fixed `state.tmp`. Two concurrent callers (e.g., cancellation + budget-cap exit racing) will silently overwrite each other. Change the temp path to include PID and a monotonic counter: `path.with_extension(format!("{}.{}.tmp", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)))`. Delete the temp file on rename failure (do not leave orphan `.tmp` files).

**`SlowMockRunHandle` (required for F-053 tests):**
- `MockRunHandle::try_wait` always returns `Some(output)` immediately. Add `try_wait_returns_none_count: Arc<AtomicU32>`. When the counter is > 0, `try_wait` decrements it and returns `Ok(None)`. When it reaches 0, returns `Ok(Some(output))`.

**Shared `stream_cost_entries` helper:**
- Add `pub fn stream_cost_entries(path: &Path) -> impl Iterator<Item = Result<CostEntry>>` to `src/audit.rs` using `BufReader` + `lines()`. All three future consumers (`BudgetTracker::reconstruct_from_costs`, `StateFile::load_or_recover`, and the existing resume cost-reconstruction) must use this single iterator — not `read_to_string` slurp.

**Tests:**
- [ ] `SlowMockRunHandle::try_wait` returns `Ok(None)` for N calls then `Ok(Some(output))`
- [ ] `stream_cost_entries` on a 1,000-line `costs.jsonl` returns correct entries without slurping
- [ ] All existing tests in `engine_integration.rs` and `engine_timeout_cancel.rs` pass after refactor

**Steps:**
- [x] Define `BudgetTracker` struct with per-phase `VecDeque<f64>` windows (cap 5)
- [x] Define `RunContext` struct and migrate local variables from `run_workflow`
- [x] Create `make_state_snapshot` + `save_state` helpers; replace all 6 construction sites
- [x] Add `is_force_kill()` check inside both grace-period inner loops
- [x] Fix `lock.rs` `serde_json::to_string` unwrap
- [x] Fix `write_atomic` temp-file naming to include PID+counter; delete on rename failure
- [x] Add `SlowMockRunHandle` with configurable `try_wait` behavior
- [x] Extract `stream_cost_entries` into `audit.rs`

---

### Task 2: F-053 — Double Ctrl+C Grace-Period Fix

**Files:** `src/engine.rs`, `tests/engine_timeout_cancel.rs`

The engine loop bug fix is done in Task 1. This task adds meaningful test coverage proving the fix works.

**Implementation:**
- Using `SlowMockRunHandle` (from Task 1), write an integration test that: (1) starts the engine on a background thread with a mock configured to stay alive for 50 polls, (2) sends the first cancellation signal, (3) waits for the grace-period loop to begin (SIGTERM recorded), (4) sends the second signal (force-kill), (5) asserts `sigkill_called` is `true` and total elapsed time is < 500ms (well within the 5-second grace period).

**Tests:**
- [x] SIGKILL sent before 500ms after second signal during grace period (use `Instant` in test)
- [x] Mock ignoring SIGTERM receives SIGKILL on second Ctrl+C (not after 5s expiry)
- [x] Normal single-Ctrl+C: SIGTERM sent, waits up to 5s (no regression)

**Steps:**
- [x] Add `SlowMockRunHandle`-based double-signal integration test to `engine_timeout_cancel.rs`
- [x] Assert SIGKILL latency < 500ms using `std::time::Instant`
- [x] Verify existing single-signal test still covers its path

---

### Task 3: F-056 — Stale Lock Detection Warning

**Files:** `src/lock.rs`, `src/main.rs`

**Implementation:**
- Add `pub struct StaleLockInfo { pub run_id: String, pub pid: u32 }` to `lock.rs`.
- Add `pub struct LockAcquireResult { pub lock: ContextLock, pub stale_removed: Option<StaleLockInfo> }`.
- Change `ContextLock::acquire()` to return `Result<LockAcquireResult, LockError>`. When a stale lock is removed, populate `stale_removed`. Do NOT `eprintln!` from inside `lock.rs` — return the info for the caller to emit.
- In `main.rs`, match on `LockAcquireResult`. When `stale_removed` is `Some(info)`, emit to stderr: `Warning: Removed stale lock file from previous run {} (PID={} no longer running).` (exact spec wording). Stale lock warning goes to stderr unconditionally (not a JSONL event) because it occurs before `--output-format` is established — record in `REVIEW.md`.
- Update both `acquire()` call sites in `main.rs`.

**Tests:**
- [ ] `stale_removed` is `Some(StaleLockInfo { run_id, pid })` when stale lock removed
- [ ] `stale_removed` is `None` when no stale lock existed
- [ ] Captured stderr contains run_id and PID when stale lock emits warning
- [ ] Active PID lock still returns `LockError::ActiveProcess` (no regression)
- [ ] `--force-lock` still bypasses lock check (no regression)

**Steps:**
- [x] Add `StaleLockInfo` and `LockAcquireResult` to `lock.rs`
- [x] Change `acquire()` return type; populate `stale_removed` on stale removal
- [x] Update both call sites in `main.rs`; emit warning string to stderr

---

### Task 4: F-029 — Unknown Variable Warnings

**Files:** `src/template.rs`, `src/engine.rs`, `tests/unknown_vars.rs`

**Implementation:**
- Define `KNOWN_VARS: &[&str]` = the 7 spec variables (`phase_name`, `cycle`, `max_cycles`, `iteration`, `runs_per_cycle`, `run`, `cost_so_far_usd`) PLUS `workflow_name` and `context_dir` (currently substituted but undocumented). Including them prevents spurious warnings for existing users. Record this under Conflicts in `REVIEW.md` with a note to update the spec.
- Add `pub fn find_unknown_variables(template: &str, known: &[&str]) -> Vec<String>` as a pure function. Implementation: (1) replace `{{{{` with a placeholder (e.g., `\x00ESCAPE\x00`), (2) scan for `{{([^{}]+)}}` patterns not in `known` using a `lazy_static!` compiled regex, (3) return deduplicated unknown variable names. Do NOT scan the post-substitution output.
- Handle `{{{{` escape in `render_prompt` via the same sentinel pre-pass ordering: replace `{{{{` with `\x00ESCAPE\x00` before variable substitutions, replace `\x00ESCAPE\x00` with `{{` at the end.
- At workflow startup (once per phase, before any runs), call `find_unknown_variables` on the raw template string and emit one `advisory_warning` JSONL event (or stderr in human mode) per unknown variable. Keep `render_prompt` returning `String` — do not change its signature.

**Tests:**
- [ ] All 9 known variables (`phase_name`, `cycle`, `max_cycles`, `iteration`, `runs_per_cycle`, `run`, `cost_so_far_usd`, `workflow_name`, `context_dir`) produce no warning
- [ ] `{{typo}}` in `unknown_vars`; remains as literal `{{typo}}` in rendered output
- [ ] `{{{{` → `{{` in rendered output; `unknown_vars` is empty
- [ ] `{{{{phase_name}}}}` → `{{phase_name}}` in rendered output; NOT flagged as unknown
- [ ] Two different unknown variables in same template: both reported
- [ ] Same unknown variable appearing twice: reported only once (deduplication)
- [ ] Unknown variables across multiple phases: all reported at startup
- [ ] Integration: workflow with unknown variable emits advisory_warning but execution proceeds

**Steps:**
- [ ] Define `KNOWN_VARS` constant
- [ ] Implement sentinel pre-pass for `{{{{` escape in `render_prompt`
- [ ] Implement `find_unknown_variables` with `lazy_static!` regex
- [ ] Add startup scan in `engine.rs`, emit advisory_warning per unknown variable per phase
- [ ] Write `tests/unknown_vars.rs`

---

### Task 5: F-081 — `--dry-run`

**Files:** `src/dry_run.rs` (new), `src/cli.rs`, `src/main.rs`, `tests/dry_run.rs`

**Depends on:** Task 4 (F-029 unknown variable scan)

**Implementation:**
- Add `#[arg(long)] dry_run: bool` to `RunArgs` in `cli.rs`.
- Create `src/dry_run.rs` with types:
  - `DryRunPhase { name: String, prompt_source: String, runs_per_cycle: u32, signal_check: SignalCheck, unknown_vars: Vec<String> }`
  - `SignalCheck { found: bool, line_number: Option<u32> }` — for inline `prompt_text`, `line_number` is the offset within the TOML value; `found` checks for the signal string as a substring
  - `DryRunPlan { phases: Vec<DryRunPhase>, runs_per_cycle_total: u32, max_cycles: Option<u32>, max_total_runs: Option<u32> }`
  - JSONL event wrapper: `DryRunPlanEvent { event: String, plan: DryRunPlan, timestamp: String }` with `#[serde(rename_all = "snake_case")]`
- Signal check for `completion_signal_mode = "regex"`: search for the literal pattern string as a substring of the prompt source (not run the regex against the prompt). Record in `REVIEW.md`.
- In dry-run mode: load workflow, scan unknown variables (Task 4), check signal per phase, build `DryRunPlan`, emit output (human: table with `✓`/`✗`; JSONL: `dry_run_plan` event), exit 0. No executor subprocess spawned. If TOML is malformed, exit 2.
- Validate `completion_signal_mode = "regex"` patterns via `Regex::new()` at workflow load time; fail with exit 2 if invalid.

**Tests:**
- [ ] `DryRunPlan.total_runs_per_cycle` correct for multi-phase workflow
- [ ] `DryRunPlan.max_total_runs` = `total_runs_per_cycle × max_cycles` when both defined
- [ ] Signal found in file prompt with correct line number
- [ ] Signal not found: `SignalCheck { found: false, line_number: None }`
- [ ] JSONL mode: valid `dry_run_plan` event with fields `found`, `line_number`, `phase_name`
- [ ] Integration: `rings run --dry-run workflow.toml` exits 0; no executor subprocess spawned (use mock that panics on spawn)
- [ ] Integration: `rings run --dry-run invalid.toml` exits 2
- [ ] Integration: `--dry-run` also reports unknown template variables (F-029 interaction)
- [ ] Inline `prompt_text` phase: signal check against inline text

**Steps:**
- [ ] Add `dry_run: bool` to `RunArgs`
- [ ] Create `src/dry_run.rs` with all types
- [ ] Implement dry-run plan generation
- [ ] Implement human + JSONL output
- [ ] Branch on `--dry-run` in `main.rs` before engine invocation
- [ ] Write `tests/dry_run.rs`

---

### Task 6: F-149 — Cost Spike Detection

**Files:** `src/engine.rs` (or `src/budget.rs`), `src/audit.rs`, `tests/cost_spike.rs`

**Depends on:** Task 1 (engine refactor; `BudgetTracker` struct exists)

**Implementation:**
- `BudgetTracker.rolling_windows: HashMap<String, VecDeque<f64>>` — one deque per phase name, cap 5. This is per-phase to prevent false positives from cross-phase cost variation (see OD-1).
- Cap enforcement inline: `if window.len() == 5 { window.pop_front(); } window.push_back(cost);`
- `None`-cost runs are SKIPPED: do not push to window, do not count toward the 3-run minimum (see OD-6).
- Spike detection after each run: if the phase's window has ≥ 3 entries, compute average. If current cost > 5× average (strict `>`), emit `advisory_warning` JSONL event with multiplier. Zero-average guard: skip spike check if all entries are 0.0.
- `BudgetTracker::reconstruct_from_costs(path: &Path, ...) -> Result<BudgetTracker>` uses `stream_cost_entries` (Task 1) in ONE pass to simultaneously populate `cumulative_cost`, `phase_costs`, AND `rolling_windows`. This replaces the existing resume cost-reconstruction loop; there must be no second separate pass over `costs.jsonl`.

**Tests:**
- [ ] No warning with fewer than 3 entries in window
- [ ] No warning at exactly 5× average (boundary: strict `>`)
- [ ] Warning when cost > 5× rolling average; message includes correct multiplier
- [ ] Rolling window drops oldest at cap: push 7 entries, assert `len() == 5`
- [ ] Zero rolling average: no divide-by-zero, no warning
- [ ] `None`-cost entries skipped — do not count toward minimum, do not lower average
- [ ] Mix of `None` and `Some` entries: average over `Some` values only
- [ ] Integration: engine emits `advisory_warning` JSONL event on spike
- [ ] On resume, rolling window pre-populated from `costs.jsonl` in single pass

**Steps:**
- [ ] Add `rolling_windows` to `BudgetTracker` with per-phase `VecDeque<f64>` (cap 5)
- [ ] Implement `check_spike` method with `None`-skip logic and exact boundary check
- [ ] Implement `reconstruct_from_costs` single-pass method using `stream_cost_entries`
- [ ] Hook spike check into engine loop after each completed run
- [ ] Emit `advisory_warning` JSONL event on spike
- [ ] Write `tests/cost_spike.rs`

---

### Task 7: F-049 — Resume State Recovery

**Files:** `src/state.rs`, `src/audit.rs`, `src/main.rs`, `tests/state_recovery.rs`

**Implementation:**
- Define `pub enum StateLoadResult` in `src/state.rs`. Do NOT derive `Serialize`/`Deserialize` — control-flow type only.
  ```rust
  pub enum StateLoadResult {
      Ok(StateFile),
      Recovered { state: StateFile, warning: String },
      Unrecoverable { state_path: PathBuf, costs_path: PathBuf },
  }
  ```
- Implement `StateFile::load_or_recover(state_path: &Path, costs_path: &Path) -> StateLoadResult`. Call `StateFile::read(state_path)`; on failure, call `audit::recover_last_run_from_costs(costs_path)`. If recovery returns `Some(n)`, construct minimal `StateFile` with `last_completed_run = n`, synthesize cycle/phase/iteration via `RunSchedule::resume_from` (count-based). Record in `REVIEW.md` that cycle/phase/iteration cannot be recovered from `costs.jsonl` alone.
- If `costs.jsonl` absent OR has no parseable entries, return `Unrecoverable` (see OD-3).
- `Unrecoverable` exact error message: `"Cannot resume: state.json is corrupt and costs.jsonl could not reconstruct the run position.\n  state.json: {state_path}\n  costs.jsonl: {costs_path}\nPlease inspect these files manually."` Use `std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())` for path display.
- In `resume_inner` (`main.rs`), replace `StateFile::read(...)?` with a match on `StateLoadResult`:
  - `Ok(state)` → proceed
  - `Recovered { state, warning }` → emit warning to stderr, proceed
  - `Unrecoverable` → print error message, `return Ok(2)` (not `Err(...)` — preserves message format)
- `audit::recover_last_run_from_costs(path: &Path) -> Result<Option<u32>>`: use `stream_cost_entries` to find the maximum `run` field from parseable entries, skipping malformed lines.

**Tests:**
- [ ] Valid `state.json` → `StateLoadResult::Ok`
- [ ] Corrupt `state.json` + valid `costs.jsonl` with ≥1 entry → `Recovered` with run number in warning string
- [ ] Corrupt `state.json` + empty `costs.jsonl` → `Unrecoverable`
- [ ] Corrupt `state.json` + absent `costs.jsonl` → `Unrecoverable`
- [ ] Both files corrupt → `Unrecoverable`
- [ ] Integration: captured stderr contains both absolute file paths on `Unrecoverable`
- [ ] Malformed JSONL lines skipped; max run from valid lines only
- [ ] Ordering invariant: `costs.jsonl` has entry N, `state.json` records N-1 (crash simulated) → recovery returns N
- [ ] `costs.jsonl` with one `cost_usd = null` entry followed by valid entry → recovery picks valid entry's run number
- [ ] Integration: `rings resume` from corrupt state reconstructs position and continues

**Steps:**
- [ ] Define `StateLoadResult` enum in `src/state.rs`
- [ ] Implement `StateFile::load_or_recover` with fallback to `recover_last_run_from_costs`
- [ ] Implement `recover_last_run_from_costs` in `audit.rs` using `stream_cost_entries`
- [ ] Update `resume_inner` in `main.rs` to match on `StateLoadResult`
- [ ] Write `tests/state_recovery.rs`

---

### Task 8: F-050 — Workflow File Change Detection

**Files:** `src/workflow.rs`, `src/state.rs`, `src/main.rs`, `tests/workflow_change_detection.rs`

**Depends on:** Task 7 (F-049 — `StateFile::load_or_recover` must succeed before fingerprint comparison)

**Implementation:**
- `WorkflowFingerprint` is `Vec<String>` — phase names in order ONLY. `runs_per_cycle` is excluded because the spec classifies it as non-structural. Record under Decisions in `REVIEW.md`. (See also OD-5 for `runs_per_cycle` clamping on resume.)
- Store `phase_fingerprint: Option<Vec<String>>` in `RunMeta` (`run.toml`), NOT in `state.json`. The fingerprint is static; writing it to `state.json` (updated every cycle) would repeat an O(phases) blob each cycle unnecessarily (see OD-2). Add `#[serde(default)]` so old `run.toml` files without this field parse cleanly.
- `Workflow::structural_fingerprint(&self) -> Vec<String>` in `src/workflow.rs` returns phase names in declaration order.
- On `rings resume`, after loading state, read `run_meta.phase_fingerprint` and compare with `workflow.structural_fingerprint()`:
  - Absent fingerprint → skip check, emit advisory warning, proceed
  - Match → proceed
  - Added phase → exit code 2: `"Cannot resume: workflow has phases not present in the saved run."`
  - Removed phase → exit code 2: `"Cannot resume: saved run has phases removed from the current workflow."`
  - Reordered phases → exit code 2: `"Cannot resume: phase order has changed since this run was created."`
  - Non-structural change detected (`runs_per_cycle`, `max_cycles`, `completion_signal`, or prompt text differs) → emit warning to stderr and proceed: `"Workflow file has changed since this run was created. Non-structural changes will take effect from the resume point."`
- Record exit code 2 for structural changes in `REVIEW.md` under Decisions (spec does not assign a code).
- When `runs_per_cycle` changes (non-structural), clamp `last_completed_iteration` to the new `runs_per_cycle` to prevent position overflow (see OD-5).

**Tests:**
- [ ] Identical fingerprints → no error, no warning
- [ ] Added phase → exit code 2, error message contains "phases not present"
- [ ] Removed phase → exit code 2, error message contains "phases removed"
- [ ] Reordered phases → exit code 2, error message contains "phase order has changed"
- [ ] Changed `runs_per_cycle` only → exit 0 with exact non-structural warning text
- [ ] Changed `max_cycles` only → exit 0 with non-structural warning text
- [ ] Changed `completion_signal` → exit 0 with non-structural warning text
- [ ] Changed prompt text only → exit 0 with non-structural warning text
- [ ] Missing `phase_fingerprint` in old `run.toml` → skip check, advisory warning, proceed
- [ ] Integration: captured stderr contains exact non-structural change warning text
- [ ] Integration: `rings resume` with structurally changed workflow exits code 2

**Steps:**
- [ ] Implement `Workflow::structural_fingerprint` returning `Vec<String>`
- [ ] Add `phase_fingerprint: Option<Vec<String>>` to `RunMeta` with `#[serde(default)]`
- [ ] Write fingerprint to `run.toml` at run-start in `main.rs`
- [ ] Implement fingerprint comparison in `resume_inner`; emit appropriate error or warning
- [ ] Clamp `last_completed_iteration` to new `runs_per_cycle` when it changes
- [ ] Write `tests/workflow_change_detection.rs`

---

### Task 9: F-070 — `rings list`

**Files:** `src/list.rs` (new), `src/cli.rs`, `src/state.rs`, `src/main.rs`, `src/duration.rs`, `tests/list_runs.rs`

**Implementation:**

**`RunStatus` enum (do first within this task):**
- Add to `src/state.rs`: `pub enum RunStatus { Running, Completed, Canceled, Failed, Incomplete, Stopped }` with `#[serde(rename_all = "lowercase")]`, `Display`, `FromStr`. Include `Incomplete` and `Stopped` to match strings written by current `main.rs`. Do NOT add new on-disk strings. Record in `REVIEW.md` that spec's `--status` filter table omits `incomplete`/`stopped`.
- Change `RunMeta.status` from `String` to `RunStatus`.
- Test: round-trip each variant through `toml::to_string` / `toml::from_str`.
- Test: old `run.toml` with bare `status = "running"` deserializes without error.

**`--output-format` global flag (do early):**
- Move `output_format: OutputFormat` to the top-level `Cli` struct with `#[arg(long, global = true, alias = "format", default_value = "human")]` (see OD-4). This ensures all subcommands inherit it and shell completions/man pages show it correctly.

**`SinceSpec` type:**
- In `src/duration.rs`, add `d` suffix (86,400 seconds) to `parse_duration_secs`.
- Define `pub enum SinceSpec { AbsoluteDate(chrono::NaiveDate), Relative(chrono::Duration) }` with `FromStr`: try ISO 8601 date first, then relative duration. Reject `0d` (consistent with `0s` rejection).
- `SinceSpec::to_cutoff_datetime() -> DateTime<Utc>`: for `AbsoluteDate`, use `d.and_hms_opt(0, 0, 0).unwrap().and_utc()` (midnight UTC — see OD-4). Record in `REVIEW.md`.

**`rings list` command:**
- Add `List(ListArgs)` to `Commands` in `cli.rs`. `ListArgs`: `since: Option<SinceSpec>` (value_parser `SinceSpec::from_str`), `status: Option<RunStatus>` (value_parser `EnumValueParser::<RunStatus>::new()`), `workflow: Option<String>`, `limit: Option<usize>` (short `-n`, default 20).
- Note: `-n` is also used for `--max-cycles` on `rings run`. This is valid in clap (separate subcommands), but do NOT share an `Args` base struct between the two.
- Implement `list_runs(filters: &ListFilters, base_dir: &Path) -> Result<Vec<RunSummary>>` in `src/list.rs`. Strategy: sort directory names descending (names are `run_YYYYMMDD_...`, lexicographic == chronological descending), iterate reading `run.toml` per directory, apply `since`/`status`/`workflow` filters inline, break once `limit` matching entries are accumulated. This is O(limit) in the common case.
- If `base_dir` missing: return `Ok(vec![])`. Silently skip unreadable or partially-written run directories (missing/corrupt `run.toml`). Scan scope = same path as `resolve_output_dir` (see OD-10).
- `RunSummary { run_id: String, started_at: DateTime<Utc>, workflow: String, status: RunStatus, cycles_completed: u32, total_cost_usd: Option<f64> }`.
- For `RunStatus::Running` with `started_at` > 24h ago and no lock file in run directory: display status as `Running (stale?)` (heuristic, see OD-7).
- Add `cmd_list` in `main.rs` that dispatches to `list_runs`, maps errors to exit code 2.
- Output: human (tabular `RUN ID | DATE | WORKFLOW | STATUS | CYCLES | COST`), JSONL (one JSON object per run). Use "session" instead of "run" in help text where ambiguous (see spec gap 14).

**Tests:**
- [ ] `RunStatus` round-trip: each variant via `toml::to_string` / `toml::from_str`
- [ ] Old `run.toml` with bare `status = "running"` deserializes correctly
- [ ] `rings list --format jsonl` and `rings list --output-format jsonl` parse identically
- [ ] `rings list --status bogus` exits non-zero and lists valid choices
- [ ] `scan_runs` returns entries sorted by date descending
- [ ] `--status` filter excludes non-matching runs
- [ ] `--workflow` substring filter matches partial path
- [ ] `--since 7d` excludes runs older than 7 days
- [ ] `--since 2024-03-15` excludes runs before that date (midnight UTC)
- [ ] Run at `2024-03-15T23:30:00-05:00` (UTC `2024-03-16T04:30:00Z`) included with `--since 2024-03-16` (UTC-day test)
- [ ] `--since 2024-13-01` exits with clap error at parse time
- [ ] `--since 0d` exits with error
- [ ] `--limit N` truncates to N most recent entries
- [ ] `--limit 5` on 50-run directory: exactly 5 `run.toml` files opened (early termination)
- [ ] Empty base directory: returns empty list, no error
- [ ] Unreadable run directory: silently skipped
- [ ] Two runs with identical `started_at` second: sort is deterministic (stable by run_id tiebreaker)
- [ ] `rings list --status incomplete` filters correctly (record spec gap in REVIEW.md)
- [ ] Integration: JSONL mode emits one JSON object per run with correct fields

**Steps:**
- [ ] Add `d` suffix to `parse_duration_secs`; add `SinceSpec` enum with `FromStr` and `to_cutoff_datetime`
- [ ] Define `RunStatus` enum; update `RunMeta.status` from `String` to `RunStatus`
- [ ] Move `--output-format` to global `Cli` level
- [ ] Add `List(ListArgs)` subcommand with typed value parsers
- [ ] Implement `RunSummary` and `list_runs` in `src/list.rs` with early-termination
- [ ] Add `cmd_list` in `main.rs`
- [ ] Write `tests/list_runs.rs`

---

### Open Decisions

| ID | Decision | Recommendation |
|----|----------|----------------|
| OD-1 | Cost spike rolling window: per-phase `HashMap<String, VecDeque<f64>>` vs. single global `VecDeque<f64>` | **Per-phase.** Cross-phase cost variation (e.g., reviewer ~$0.01 vs. builder ~$0.40) causes false positives with a global window. Spec is ambiguous — record in `REVIEW.md`. |
| OD-2 | `phase_fingerprint` storage: `run.toml` (once) vs. `state.json` (every cycle) | **`run.toml`.** Fingerprint is static for the run lifetime; no reason to repeat it in `state.json` on every write. |
| OD-3 | `StateLoadResult::Unrecoverable` when `costs.jsonl` is absent | **Treat absent as empty → Unrecoverable.** Distinct user message: "No runs completed before interruption." Record in `REVIEW.md`. |
| OD-4 | `--output-format` scope: global `Cli` level vs. per-subcommand `Args` struct | **Global** on `Cli` with `#[arg(global = true)]`. Correct completions and man pages require it. |
| OD-5 | `runs_per_cycle` changes: structural error vs. non-structural warning | **Non-structural (warning).** Spec explicitly says non-structural. Exclude from fingerprint. Clamp `last_completed_iteration` to new value on resume. Record in `REVIEW.md`. |
| OD-6 | `None`-cost runs in `BudgetTracker` rolling window | **Skip entirely** — do not count toward 3-run minimum, do not add 0.0. Avoids suppressing legitimate spikes. |
| OD-7 | `rings list` stale `status = "running"` display | Show `Running (stale?)` if started > 24h ago with no lock file. Non-authoritative heuristic; document in help text. |
| OD-8 | `workflow_name` / `context_dir` in `KNOWN_VARS` | **Include them.** Currently substituted; excluding causes spurious warnings for existing users. File spec conflict in `REVIEW.md`. |
| OD-9 | `completion_signal_mode = "regex"` signal check in `--dry-run` | **Literal substring search** for the pattern string in prompt source — not running the regex against the prompt. Validate regex at workflow load time. |
| OD-10 | `rings list` scan scope | **Same as `resolve_output_dir`** (resume search path). Record as decision in `REVIEW.md`. |

---

### Spec Gaps

1. **`workflow_name` / `context_dir` undocumented** — substituted by `template.rs` but absent from `specs/execution/prompt-templating.md`. Include in `KNOWN_VARS`; update spec. Record under Conflicts in `REVIEW.md`.

2. **`WorkflowFingerprint` is phase names only** — draft plan incorrectly included `runs_per_cycle`. Spec says `runs_per_cycle` is non-structural. Record under Decisions in `REVIEW.md`.

3. **Rolling window per-phase vs. global** — `specs/execution/engine.md` says "last 5 runs" without defining scope. Record under Open Questions in `REVIEW.md`.

4. **`rings list --status` missing `incomplete` / `stopped`** — `specs/cli/commands-and-flags.md` lists only 4 values; engine writes both. Record under Open Questions in `REVIEW.md`.

5. **F-053 "immediately" latency** — spec says "immediately"; acceptable = within one 100ms poll interval. Record under Decisions in `REVIEW.md`.

6. **`rings list` scan scope** — spec does not specify which directories are scanned. Use `resolve_output_dir`. Record under Decisions in `REVIEW.md`.

7. **Unknown variable detection timing** — spec says "startup advisory warning"; implementation scans raw template once per phase before any runs. Record under Decisions in `REVIEW.md`.

8. **F-050 exit code for structural changes** — spec says "exits with an error" without a code. Use exit code 2 (consistent with other resume errors). Record under Decisions in `REVIEW.md`.

9. **`costs.jsonl` absent (not corrupt, not empty)** — treat as empty → `Unrecoverable`. Record under Decisions in `REVIEW.md`.

10. **`last_completed_cycle` semantics on cycle-1 cancellation** — engine sets it to the in-progress cycle, not the last completed one. Ambiguity affects F-049 recovery. Record under Open Questions in `REVIEW.md`.

11. **`state.json` inconsistency across specs** — `cancellation-resume.md` includes `workflow_file`; `audit-logs.md` omits it. Current code includes it. Record under Open Questions in `REVIEW.md`.

12. **F-050 fingerprint and prompt source type** — spec lists structural changes as name/order only; changing a phase from file-based `prompt` to inline `prompt_text` is not addressed. Record under Open Questions in `REVIEW.md`.

13. **Stale `status = "running"` display** — no spec-defined heuristic. Using 24h age + no lock file. Record under Decisions in `REVIEW.md`.

14. **"run" vocabulary collision** — spec glossary defines "run" as a single `claude` invocation, but `rings list` exposes `RUN_ID` meaning a full workflow session. Use "session" in `rings list` help text where ambiguous; do not rename other interfaces in this batch. Record under Open Questions in `REVIEW.md`.

---

