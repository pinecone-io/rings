# Plan Drafts Queue

Initial technical plans produced by plan-create, awaiting review by plan-review.
Each entry begins with `## [DRAFT]` or `## [REVIEWED]`.

## [REVIEWED] Batch: Resilience, Discovery & Safety Feedback — 2026-03-16

### Selected Features

| F-NNN | Feature | Spec file |
|-------|---------|-----------|
| F-081 | --dry-run | specs/cli/commands-and-flags.md |
| F-056 | Stale Lock Detection | specs/state/cancellation-resume.md |
| F-070 | rings list | specs/cli/commands-and-flags.md |
| F-050 | Workflow File Change Detection | specs/state/cancellation-resume.md |
| F-029 | Unknown Variable Warnings | specs/execution/prompt-templating.md |
| F-149 | Cost Spike Detection | specs/execution/engine.md |
| F-049 | Resume State Recovery | specs/state/cancellation-resume.md |
| F-053 | Double Ctrl+C | specs/state/cancellation-resume.md |

### Source Files

Files to create or modify, deduplicated across all three reviewers:

**Create (new files):**
- `src/dry_run.rs` — dry-run plan generation (F-081)
- `src/list.rs` — run directory scanning and filtering for `rings list` (F-070)
- `src/run_index.rs` — (alternate name considered) `RunSummary` aggregation from run metadata
- `src/commands/mod.rs`, `src/commands/run.rs`, `src/commands/resume.rs`, `src/commands/list.rs` — command handler extraction from `main.rs` (architectural prerequisite)
- `src/budget.rs` — `BudgetTracker` with rolling cost window (F-149, engine refactor)
- `src/workflow_diff.rs` — structural fingerprint comparison (F-050)
- `tests/dry_run.rs` — integration tests for F-081
- `tests/list_runs.rs` — integration tests for F-070
- `tests/unknown_vars.rs` — tests for F-029
- `tests/cost_spike.rs` — tests for F-149
- `tests/state_recovery.rs` — tests for F-049
- `tests/workflow_change_detection.rs` — tests for F-050

**Modify (existing files):**
- `src/engine.rs` — extract `RunContext`/`BudgetTracker`/`save_state` helpers; add cost spike detection (F-149); fix double Ctrl+C grace-period SIGKILL check (F-053); add rolling cost window on resume
- `src/template.rs` — add `scan_unknown_vars`, `KNOWN_VARS` constant, `find_unknown_variables` function, `{{{{` escape handling (F-029)
- `src/lock.rs` — emit stale-lock warning with run_id and PID; return `LockAcquireResult` with `stale_removed` field (F-056)
- `src/state.rs` — add `phase_signatures`/`phase_fingerprint` field (F-050); add `StateFile::recover_from_costs` (F-049); promote `status` to `RunStatus` enum (F-070)
- `src/cli.rs` — add `--dry-run` flag to `RunArgs`; add `List(ListArgs)` subcommand variant (F-070, F-081)
- `src/main.rs` — dispatch `Command::List`; branch on `--dry-run` before engine invocation; extract command handlers to `src/commands/`
- `src/duration.rs` — add `d` (days) suffix support; add `parse_since_duration() -> Result<chrono::Duration>` (F-070)
- `src/audit.rs` — add `recover_last_run_from_costs(costs_path: &Path) -> Result<Option<u32>>` (F-049)
- `src/workflow.rs` — add `Workflow::structural_fingerprint(&self) -> Vec<(String, u32)>` (F-050)
- `src/lib.rs` — expose new modules (`dry_run`, `list`, `budget`, `workflow_diff`)
- `tests/engine_timeout_cancel.rs` — add/verify double Ctrl+C coverage (F-053)

### Key Types, Traits, and Structs

| Type / Trait | Purpose | Feature |
|---|---|---|
| `DryRunPlan` | Serializable execution plan (phases, totals, prompt checks) emitted by `--dry-run` | F-081 |
| `DryRunPhase` | Per-phase dry-run summary (name, runs_per_cycle) | F-081 |
| `PromptCheckResult` / `SignalCheck` | Reports whether completion signal is found in a prompt file, with line number | F-081 |
| `RunSummary` | Aggregated per-run data for `rings list` (id, date, workflow, status, cycles, cost) | F-070 |
| `ListFilters` | Filter parameters for `list_runs` (status, workflow substring, since, limit) | F-070 |
| `RunStatus` | Enum replacing the bare `String` status field: `Running`, `Completed`, `Canceled`, `Failed`, `Incomplete` | F-070 |
| `LockAcquireResult` | Wraps `ContextLock` with optional `StaleLockInfo { run_id, pid }` for warning emission | F-056 |
| `WorkflowFingerprint` / `PhaseSignature` | Ordered `Vec<(String, u32)>` of `(phase_name, runs_per_cycle)` for structural comparison | F-050 |
| `StateLoadResult` | Enum: `Ok(StateFile)` / `Recovered { state, warning }` / `Unrecoverable { state_path, costs_path }` | F-049 |
| `BudgetTracker` | Owns `phase_costs`, `cumulative_cost`, budget warning flags, and rolling cost `VecDeque<f64>` | F-149 + engine refactor |
| `CostHistory` / `SpikeWarning` | Rolling window (cap 5) and spike detection result | F-149 |
| `RenderResult` | Replaces bare `String` from `render_prompt`: adds `unknown_vars: Vec<String>` | F-029 |
| `RunContext` | Mutable accumulated state extracted from `run_workflow` monolith (engine refactor prerequisite) | engine refactor |

### Test Cases Required

**F-081 (--dry-run):**
- Unit: `DryRunPlan` correctly computes `total_runs_per_cycle` and `max_total_runs` for multi-phase workflows
- Unit: Signal check finds signal in prompt file with correct line number
- Unit: Signal check reports "not found" when signal absent from prompt file
- Unit: JSONL mode produces valid `dry_run_plan` event with structured JSON
- Integration: `rings run --dry-run workflow.toml` exits 0 without spawning any executor subprocess
- Integration: `--dry-run` also reports unknown template variables (F-029 interaction)

**F-056 (Stale Lock Detection):**
- Unit: Stale lock removal emits warning containing old run_id and PID
- Unit: Active PID lock returns exit code 2 with error message
- Unit: `--force-lock` bypasses lock check entirely
- Unit: `LockAcquireResult.stale_removed` is `Some` when stale lock was removed
- (Note: PID liveness check and basic lock creation already tested; add warning message assertion)

**F-070 (rings list):**
- Unit: `scan_runs` / `list_runs` returns entries sorted by date descending
- Unit: `--status` filter excludes non-matching runs
- Unit: `--workflow` substring filter matches partial path
- Unit: `--since 7d` relative duration filter excludes runs older than 7 days
- Unit: `--since 2024-03-15` ISO 8601 filter excludes runs before that date
- Unit: `--limit N` truncates to N most recent entries
- Integration: End-to-end list with pre-seeded run directories
- Integration: JSONL mode emits one JSON object per run with correct fields

**F-050 (Workflow File Change Detection):**
- Unit: Identical phase fingerprints pass without error or warning
- Unit: Added phase detected as structural change → error
- Unit: Removed phase detected as structural change → error
- Unit: Reordered phases detected as structural change → error
- Unit: Changed `prompt_text` only (non-structural) → passes with warning
- Unit: Changed `max_cycles` only (non-structural) → passes with warning
- Unit: Changed `completion_signal` → passes with specific warning
- Unit: Missing `phase_fingerprint` in old state files → check skipped with warning
- Integration: `rings resume` with structurally changed workflow exits with error

**F-029 (Unknown Variable Warnings):**
- Unit: All known variables (`phase_name`, `cycle`, `max_cycles`, `iteration`, `runs_per_cycle`, `run`, `cost_so_far_usd`) are substituted without warning
- Unit: `{{typo}}` passes through as literal text and is reported in `unknown_vars`
- Unit: `{{{{` escape produces literal `{{` and is not flagged as unknown
- Unit: Multiple unknown variables across phases all reported independently
- Unit: Startup emits one advisory warning per unknown variable per phase
- Integration: Workflow with unknown variable in prompt emits warning at startup but execution proceeds

**F-149 (Cost Spike Detection):**
- Unit: No warning when history has fewer than 3 runs
- Unit: No warning when cost is within 5× rolling average
- Unit: Warning emitted when cost exceeds 5× rolling average, with correct multiplier in message
- Unit: Rolling window drops oldest entry after holding 5 entries
- Unit: Zero rolling average does not produce divide-by-zero
- Integration: Engine emits `advisory_warning` JSONL event on spike
- Integration: On resume, rolling window is pre-populated from `costs.jsonl`

**F-049 (Resume State Recovery):**
- Unit: Valid `state.json` → `StateLoadResult::Ok`
- Unit: Corrupt `state.json` + valid `costs.jsonl` with ≥1 entry → `StateLoadResult::Recovered` with correct last run number
- Unit: Corrupt `state.json` + empty `costs.jsonl` → `StateLoadResult::Unrecoverable`
- Unit: Both files corrupt → `StateLoadResult::Unrecoverable` with absolute paths in message
- Unit: Malformed JSONL lines are skipped; max run is taken from valid lines only
- Integration: `rings resume` from corrupt state reconstructs position and continues execution

**F-053 (Double Ctrl+C):**
- Unit: `CancelState` transitions: `NotCanceled → Canceling → ForceKill` (existing, verify coverage)
- Integration: Force-kill check inside grace-period wait loop sends SIGKILL immediately when `is_force_kill()` is true; does not wait full 5 seconds
- Integration: Mock executor ignoring SIGTERM receives SIGKILL on second Ctrl+C

### Cross-Feature Dependencies

- **F-081 (--dry-run) → F-029 (Unknown Variable Warnings)**: Dry-run should also report unknown template variables as part of its prompt check, giving users full pre-flight validation without spending money. Implement F-029 first.
- **F-049 (State Recovery) is prerequisite for F-050 (Workflow Change Detection)**: Resume must successfully load (or recover) state before comparing it to the current workflow structure. Implement F-049 first.
- **F-149 (Cost Spike Detection) → engine refactor prerequisite**: Cost spike detection naturally belongs in a `BudgetTracker` struct extracted from `run_workflow`. Extracting `RunContext`/`BudgetTracker` first prevents the engine from growing further. Implement the engine refactor before F-149.
- **F-053 (Double Ctrl+C)**: Appears largely implemented; requires only a grace-period loop fix in `engine.rs`. Independent of all other features.
- **F-056 (Stale Lock Detection)**: Largely implemented; requires only the stale-lock warning emission. Independent of all other features.
- **F-070 (rings list)**: Fully independent of the engine; can be implemented in parallel with engine-touching features. Depends only on `RunMeta`/`StateFile` read paths being stable.
- **F-029 (Unknown Variable Warnings)**: Self-contained in `template.rs`. No dependencies. Implement early as a prerequisite for F-081.

**Recommended implementation order:**
1. Engine refactor (extract `RunContext`, `BudgetTracker`, `save_state` helper) — prerequisite
2. F-053 (Double Ctrl+C grace-period fix) — small, validates refactor
3. F-056 (Stale lock warning emission) — small, nearly complete
4. F-029 (Unknown variable warnings) — self-contained, prerequisite for F-081
5. F-081 (--dry-run) — new module, depends on F-029
6. F-149 (Cost spike detection) — needs BudgetTracker from step 1
7. F-049 (Resume state recovery) — new `StateLoadResult` type
8. F-050 (Workflow file change detection) — depends on F-049 for recovery path
9. F-070 (rings list) — independent, can be done in parallel after step 1

<!-- No pending drafts. -->
