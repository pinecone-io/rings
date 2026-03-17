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

## [REVIEWED] Batch: Error Handling, File Lineage & Inspect Foundation — 2026-03-17

### Selected Features

| F-NNN | Feature | Spec file |
|-------|---------|-----------|
| F-037 | Error Classification | specs/execution/error-handling.md |
| F-038 | Quota Error Detection | specs/execution/error-handling.md |
| F-039 | Auth Error Detection | specs/execution/error-handling.md |
| F-117 | File Manifest | specs/observability/file-lineage.md |
| F-118 | File Diff Detection | specs/observability/file-lineage.md |
| F-072 | `rings inspect` | specs/cli/inspect-command.md |
| F-097 | Summary View | specs/cli/inspect-command.md |
| F-044 | Quota Backoff | specs/execution/rate-limiting.md |
| F-058 | Parent Run ID | specs/state/run-ancestry.md |

### Source Files

**Create (new files):**
- `src/error_classify.rs` — `ErrorProfile` enum, compiled pattern matching, `classify()` function (F-037/038/039)
- `src/backoff.rs` — `QuotaBackoff` state machine encapsulating retry-with-delay logic (F-044)
- `src/manifest.rs` — manifest computation, SHA-256 hashing, mtime caching, gzip write/read, diff detection (F-117/118)
- `src/inspect.rs` — `rings inspect` rendering logic, `RunSummary` aggregation, view dispatching (F-072/097)
- `tests/error_classify.rs` — integration tests for error classification and error-specific exit behavior
- `tests/quota_backoff.rs` — integration tests for quota retry logic
- `tests/manifest.rs` — integration tests for manifest computation and diff detection
- `tests/inspect.rs` — integration tests for inspect command output

**Modify (existing files):**
- `src/workflow.rs` — add `error_profile` to `ExecutorConfig`; add `quota_backoff`, `quota_backoff_delay`, `quota_backoff_max_retries`, `delay_between_cycles` to `WorkflowConfig`; add `manifest_enabled`, `manifest_ignore`, `snapshot_cycles`, `manifest_mtime_optimization`
- `src/engine.rs` — call `classify()` on non-zero executor exits and populate `failure_reason`; add quota backoff retry loop around quota error path; invoke manifest computation before first run and after each run; use `RunContext`/`BudgetTracker` (prerequisite refactor)
- `src/state.rs` — add `parent_run_id`, `continuation_of`, `ancestry_depth` to `RunMeta` and `StateFile`; change `failure_reason` from `Option<String>` to `Option<FailureReason>` enum
- `src/cli.rs` — add `Inspect(InspectArgs)` and `Lineage(LineageArgs)` subcommand variants; add `--quota-backoff`, `--quota-backoff-delay`, `--quota-backoff-max-retries` flags; add `--parent-run` to `RunArgs`
- `src/display.rs` — add `print_quota_error()`, `print_auth_error()` functions matching spec output templates; add `print_quota_backoff_waiting()` and `print_quota_backoff_exhausted()`; update `print_executor_error()` to dispatch by `ErrorClass`
- `src/audit.rs` — extend `CostEntry` with `files_added`, `files_modified`, `files_deleted`, `files_changed` fields; add retry log file naming (`007-retry-1.log`)
- `src/main.rs` — dispatch `Command::Inspect` and `Command::Lineage`; set `parent_run_id` on resume; pass `--parent-run` on fresh run
- `src/lib.rs` — declare `error_classify`, `backoff`, `manifest`, `inspect` modules
- `Cargo.toml` — add `flate2`, `glob`, `sha2` dependencies

### Key Types, Traits, and Structs

| Type / Trait | Purpose | Feature |
|---|---|---|
| `ErrorClass` | Enum: `Quota`, `Auth`, `Unknown` — typed classification of executor failures | F-037 |
| `ErrorProfile` | Enum: `ClaudeCode`, `None`, `Custom { quota_patterns, auth_patterns }` — deserialized from TOML | F-037 |
| `CompiledErrorProfile` | Pre-compiled regex/pattern sets for quota and auth detection, created at workflow load time | F-037/038/039 |
| `FailureReason` | Enum: `Quota`, `Auth`, `Timeout`, `Unknown` with `#[serde(rename_all = "lowercase")]` — replaces stringly-typed `failure_reason` | F-037 |
| `QuotaBackoff` | State machine: `enabled`, `delay_secs`, `max_retries`, `current_retries`; methods `should_retry()`, `record_retry()` | F-044 |
| `FileEntry` | Struct: `path`, `sha256`, `size_bytes`, `modified` — one entry in a manifest | F-117 |
| `Manifest` | Struct: `timestamp`, `run`, `cycle`, `phase`, `iteration`, `root`, `files: Vec<FileEntry>` | F-117 |
| `FileDiff` | Struct: `added`, `modified`, `deleted` (each `Vec<String>`) — diff between two manifests | F-118 |
| `InspectView` | Enum (clap `ValueEnum`): `Summary`, `Cycles`, `FilesChanged`, `DataFlow`, `Costs`, `ClaudeOutput` | F-072 |
| `InspectArgs` | Struct: `run_id`, `show: Vec<InspectView>`, `cycle: Option<u32>`, `phase: Option<String>`, `format: OutputFormat` | F-072 |
| `RunSummary` | Aggregated per-run data for summary view: status, cycles, cost, phase breakdown, files changed, ancestry | F-097 |
| `PhaseSummary` | Struct: `name`, `runs`, `cost_usd` — used within `RunSummary.phase_breakdown` | F-097 |

### Test Cases Required

**F-037/038/039 (Error Classification):**
- Unit: `classify()` returns `Quota` for each quota pattern (case-insensitive): "usage limit reached", "rate limit", "quota exceeded", "too many requests", "429", "claude.ai/settings"
- Unit: `classify()` returns `Auth` for each auth pattern (case-insensitive): "authentication", "invalid api key", "unauthorized", "401", "please log in", "not logged in"
- Unit: first-match-wins when both quota and auth patterns present in output
- Unit: `classify()` returns `Unknown` when no patterns match
- Unit: `"none"` profile always returns `Unknown` regardless of output content
- Unit: custom profile with provided patterns matches specified strings only
- Unit: `ErrorProfile` deserializes from all three TOML shapes (string `"claude-code"`, string `"none"`, inline table)
- Integration: engine exits code 3 with `failure_reason = "quota"` when executor output contains quota patterns
- Integration: engine exits code 3 with `failure_reason = "auth"` for auth patterns
- Integration: engine exits code 3 with `failure_reason = "unknown"` for unmatched non-zero exit
- Integration: display functions emit spec-matching output for each error class

**F-044 (Quota Backoff):**
- Unit: `QuotaBackoff` state machine transitions: `should_retry()` returns true until max_retries exhausted
- Unit: TOML deserialization of backoff config fields, CLI override precedence
- Integration: quota error triggers retry, second attempt succeeds — run number not incremented
- Integration: max retries exhausted — exits with code 3, state saved
- Integration: Ctrl+C during backoff delay triggers cancellation
- Integration: retry log file naming (`007-retry-1.log`)
- Integration: `quota_backoff = false` (default) exits immediately on first quota error

**F-117 (File Manifest):**
- Unit: `compute_manifest` produces correct SHA256 for known file content
- Unit: mtime optimization reuses SHA256 when mtime unchanged
- Unit: credential patterns (`.env`, `*_rsa`, `*.pem`, `*.key`) always excluded regardless of `manifest_ignore`
- Unit: custom `manifest_ignore` patterns work correctly
- Unit: `write_manifest_gz` / `read_manifest_gz` gzip roundtrip preserves content
- Unit: manifest excludes rings output directory itself
- Integration: `000-before.json.gz` created before first run
- Integration: manifests written to correct paths after each run
- Integration: large-file-count warning emitted when >10,000 files

**F-118 (File Diff Detection):**
- Unit: `diff_manifests` correctly detects added files (in after but not before)
- Unit: `diff_manifests` correctly detects modified files (same path, different sha256)
- Unit: `diff_manifests` correctly detects deleted files (in before but not after)
- Unit: unchanged files do not appear in any diff category
- Integration: diff data appended to `costs.jsonl` `run_end` records

**F-072/097 (Inspect / Summary View):**
- Unit: `build_summary` correctly parses a well-formed run directory with `run.toml`, `state.json`, `costs.jsonl`
- Unit: phase breakdown aggregation from costs.jsonl
- Unit: human rendering format matches spec layout
- Unit: JSONL rendering produces valid JSON
- Unit: `--cycle` and `--phase` filters work correctly
- Unit: summary gracefully degrades when manifests absent (`manifest_enabled = false`)
- Unit: `InspectView::from_str("files-changed")` succeeds (kebab-case validation)
- Integration: `rings inspect <run_id>` after a completed run shows correct summary
- Integration: summary shows ancestry info when `parent_run_id` present

**F-058 (Parent Run ID):**
- Unit: fresh run has `parent_run_id = None`, `ancestry_depth = 0`
- Unit: resume sets `parent_run_id` to the resumed run's ID
- Unit: `--parent-run` sets both `parent_run_id` and `continuation_of`
- Unit: `ancestry_depth` increments correctly through a chain
- Unit: old `run.toml` without ancestry fields deserializes with `None` defaults (backward compatibility)
- Integration: `rings resume` creates a new run with `parent_run_id` set
- Integration: `rings run --parent-run <id>` sets ancestry correctly

### Cross-Feature Dependencies

- **F-038 and F-039 depend on F-037**: `ErrorProfile` and `classify()` must exist before specific quota and auth detection can be wired into the engine
- **F-044 depends on F-038**: quota backoff fires only when `classify()` returns `ErrorClass::Quota`
- **F-118 depends on F-117**: diff detection requires two consecutive manifests to compare
- **F-097 depends on F-072**: summary view is one of inspect's view modes
- **F-072/097 depend on F-058**: summary view displays ancestry info (`parent_run_id`)
- **F-072/097 depend on F-117/118**: `--show files-changed` requires manifest data on disk; summary shows files-changed count

**Prerequisite refactors (identified by all three reviewers):**
- **Engine refactor [blocker]**: `run_workflow` is ~750 lines with 15+ local mutable variables and 6+ duplicated `StateFile` construction sites. The `make_state_snapshot` helper exists but is `#[allow(dead_code)]`. `RunContext` and `BudgetTracker` are defined but unused in the actual loop. Before adding error classification, backoff loops, and manifest hooks, extract state into `RunContext`, use `make_state_snapshot`, and switch to `BudgetTracker`. This prevents all subsequent features from compounding the copy-paste debt.
- **`FailureReason` enum [concern]**: Replace stringly-typed `failure_reason: Option<String>` with a proper enum using `#[serde(rename_all = "lowercase")]`
- **`interruptible_sleep` helper [concern]**: Extract the polling sleep pattern (used for inter-run delay, will be needed for quota backoff delay) into a reusable helper

**New Cargo dependencies:**
- `flate2 = { version = "1", default-features = false, features = ["rust_backend"] }` — gzip compression for manifest storage (pure Rust, no C FFI)
- `glob = "0.3"` — glob pattern matching for manifest ignore patterns (no transitive deps)
- `sha2 = "0.10"` — SHA-256 file fingerprinting (pure Rust, RustCrypto org, minimal transitive surface)

**Recommended implementation order:**
1. Engine refactor (extract `RunContext`/`BudgetTracker`, use `make_state_snapshot`, extract `interruptible_sleep`)
2. F-058 (Parent Run ID) — state schema only, no engine changes, unblocks inspect ancestry display
3. F-037/F-038/F-039 (Error Classification) — new module + small engine integration
4. F-044 (Quota Backoff) — depends on error classification
5. F-117/F-118 (File Manifest + Diff) — independent of error features, parallel-safe after refactor
6. F-072/F-097 (Inspect + Summary View) — read-only, depends on all prior state/audit changes being in place
