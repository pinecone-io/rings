## Batch: Error Handling, File Lineage & Inspect Foundation — 2026-03-17

**Features:** F-037 (Error Classification), F-038 (Quota Error Detection), F-039 (Auth Error Detection), F-117 (File Manifest), F-118 (File Diff Detection), F-072 (`rings inspect`), F-097 (Summary View), F-044 (Quota Backoff), F-058 (Parent Run ID)

---

### Task 1: Engine Refactor — Extract RunContext, BudgetTracker, exit_workflow, interruptible_sleep

**Files:** `src/engine.rs`

Extract the ~750-line `run_workflow` monolith into reusable components before any new features touch the engine. The `RunContext` struct and `BudgetTracker` are already defined but unused in the actual loop — wire them in. The six inline `StateFile` construction sites must collapse into a single `make_state_snapshot` call (fix the `phase_index` bug first: on executor error, `last_completed_phase_index` must point to the *failing* run's position so it retries on resume, not the last successful position). Extract an `exit_workflow` helper that handles the duplicated "print final cycle cost, append cost entry, return EngineResult" sequences that appear at timeout, cancel, executor error, budget cap global, and budget cap per-phase paths. Extract `interruptible_sleep(duration: Duration, cancel: &CancelState, tick_callback: impl Fn(Duration)) -> SleepResult` where `SleepResult` is `Completed | Canceled`, polling at 100 ms intervals — this replaces the ad-hoc delay loop at engine.rs ~1244–1255 and will be reused by quota backoff.

**Tests:**
- [ ] Existing engine tests still pass after refactor (no behavior change)
- [ ] `interruptible_sleep` with `cancel_state` set before call returns `Canceled` immediately
- [ ] `interruptible_sleep` with `cancel_state` set mid-sleep returns `Canceled` within ~200 ms
- [ ] `make_state_snapshot(ExitReason::ExecutorError(...))` sets `last_completed_phase_index` to the failing run's position (not the last successful position)

**Steps:**
- [ ] Wire `RunContext` and `BudgetTracker` into the main loop (they exist but are unused)
- [ ] Fix `make_state_snapshot` phase_index invariant for `ExitReason::ExecutorError`
- [ ] Replace all six inline `StateFile { ... }` construction sites with `make_state_snapshot` calls
- [ ] Extract `exit_workflow` helper unifying all five exit-path sequences
- [x] Extract `interruptible_sleep` helper; replace existing inter-run delay loop with it

---

### Task 2: Schema Migration — FailureReason Enum, Ancestry Fields, CostEntry Extension

**Files:** `src/state.rs`, `src/audit.rs`, `Cargo.toml`

Three schema changes that must land together before any other feature writes to these files:

**FailureReason enum:** Replace `failure_reason: Option<String>` in `StateFile` with `failure_reason: Option<FailureReason>`. Add `#[serde(rename_all = "lowercase")]` on `FailureReason { Quota, Auth, Timeout, Unknown }`. The `Timeout` variant is already written by the existing timeout path — confirm it is consistent with the executor-integration spec and record in `REVIEW.md`. Use `#[serde(default)]` on the field for absent-key compat. Parameterize `ExitReason::ExecutorError(FailureReason)` so classification flows through `make_state_snapshot` without a separate local variable. Add `failure_reason: Option<FailureReason>` to `EngineResult` so `main.rs` can dispatch to the correct display function without re-reading state.

**Ancestry fields:** Add a nested `AncestryInfo { parent_run_id: Option<String>, continuation_of: Option<String>, ancestry_depth: u32 }` struct. In `state.json`, include as `#[serde(default)] pub ancestry: Option<AncestryInfo>`. In `run.toml` via `RunMeta`, add the three fields flat (matching the run.toml spec layout) with `#[serde(default)]`. Note: `toml::to_string_pretty` on `Option<String> = None` emits nothing (not `null`) — the spec example `parent_run_id = null` is misleading; record this in `REVIEW.md` under Decisions.

**CostEntry extension:** Add `#[serde(default)] pub files_added: u32`, `files_modified: u32`, `files_deleted: u32`, `files_changed: u32`, and `#[serde(default)] pub event: Option<String>` to `CostEntry` in `audit.rs`. The `event` field distinguishes `run_start` from `run_end` records — required by `rings inspect --show files-changed` and `--show costs`. Without `#[serde(default)]`, `stream_cost_entries` will fail to deserialize any existing `costs.jsonl` line lacking these fields, including the state-recovery path.

**Tests:**
- [ ] `FailureReason` round-trip: deserializing existing `state.json` with `"failure_reason": "quota"` (string) succeeds
- [ ] `FailureReason` round-trip: deserializing `state.json` with `"failure_reason": "timeout"` succeeds
- [ ] `FailureReason` round-trip: deserializing `state.json` without `failure_reason` field succeeds (None default)
- [ ] `RunMeta` deserializing old `run.toml` without ancestry fields succeeds with None/0 defaults (use a literal TOML string fixture matching current output format)
- [ ] `AncestryInfo` in `state.json` uses nested `"ancestry"` key, not flat fields
- [ ] `CostEntry` deserializing a `costs.jsonl` line without file-diff fields succeeds with 0/None defaults
- [ ] `CostEntry` deserializing a line without `event` field succeeds with None default
- [ ] `EngineResult` carries `failure_reason: Some(FailureReason::Quota)` after executor emits quota pattern

**Steps:**
- [ ] Define `FailureReason` enum with `#[serde(rename_all = "lowercase")]`
- [ ] Parameterize `ExitReason::ExecutorError(FailureReason)`; update `make_state_snapshot`
- [ ] Add `failure_reason: Option<FailureReason>` to `EngineResult`
- [ ] Add `AncestryInfo` struct; add `ancestry: Option<AncestryInfo>` to `StateFile`; add flat ancestry fields to `RunMeta`
- [ ] Add `event`, `files_added/modified/deleted/changed` to `CostEntry` with `#[serde(default)]`

---

### Task 3: Workflow Config Extension — ErrorProfile, New Fields, Workflow::validate() Promotion

**Files:** `src/workflow.rs`, `src/lib.rs`

Add `error_profile: Option<ErrorProfile>` to `ExecutorConfig`. Add `quota_backoff: bool`, `quota_backoff_delay: u64`, `quota_backoff_max_retries: u32`, `manifest_enabled: bool`, `manifest_ignore: Vec<String>`, `snapshot_cycles: bool`, `manifest_mtime_optimization: bool` to `WorkflowConfig`. Also add `delay_between_cycles: u64` (consistent with `delay_between_runs: u64`; record the duration-string inconsistency between these fields and `delay_between_runs` in `REVIEW.md` under Open Questions).

**Critical:** All new fields must be promoted through `Workflow::validate()` into the `Workflow` struct itself — the engine receives `Workflow`, not `WorkflowFile`. The plan's "add to `WorkflowConfig`" step is insufficient without also updating `validate()`. When `executor` is `None`, the compiled error profile defaults to `ClaudeCode`.

Build `CompiledErrorProfile` at `Workflow::validate()` time (pre-compiled regex sets), not per-run. Store on the validated `Workflow` struct. This prevents regex recompilation inside the engine loop.

`ErrorProfile` TOML deserialization requires `#[serde(untagged)]`:
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ErrorProfile {
    Named(ErrorProfileName),
    Custom { quota_patterns: Vec<String>, auth_patterns: Vec<String> },
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorProfileName { ClaudeCode, None }
```
`ErrorProfile` should only derive `Deserialize`, not `Serialize` (it is never serialized back to state files).

**Tests:**
- [ ] `ErrorProfile` TOML deser: bare string `"claude-code"` → `Named(ClaudeCode)`
- [ ] `ErrorProfile` TOML deser: bare string `"none"` → `Named(None)`
- [ ] `ErrorProfile` TOML deser: inline table `{ quota_patterns = [...], auth_patterns = [...] }` → `Custom`
- [ ] `ErrorProfile` does not derive `Serialize` (confirm at compile time via no `.serialize()` call)
- [ ] `Workflow::validate()` returns `CompiledErrorProfile` populated from `ClaudeCode` patterns when `executor` is None
- [ ] `Workflow::validate()` compiles custom patterns correctly

**Steps:**
- [ ] Add `ErrorProfile` and `ErrorProfileName` enums with `#[serde(untagged)]`
- [ ] Add all new fields to `ExecutorConfig` and `WorkflowConfig`
- [ ] Update `Workflow::validate()` to promote new fields into `Workflow`, compile `CompiledErrorProfile`

---

### Task 4: F-058 — Parent Run ID (Architectural Decision Required)

**Files:** `src/main.rs`, `src/state.rs`, `src/cli.rs`

**⚠️ Architectural decision must be resolved before implementation:** The spec (`specs/state/run-ancestry.md`) says `rings resume` "creates a new run" with `parent_run_id` set. The current implementation reuses the same `run_dir` and `run_id`. These are incompatible. Before implementing, record the resolution in `REVIEW.md` under Conflicts and Decisions:
- Option A: Change `resume_inner` to create a new run directory with a new `run_id`, writing the old `run_id` as `parent_run_id`. The old run directory's `costs.jsonl` and logs are not carried over — the new run starts fresh but links back. **This is the spec-compliant path.**
- Option B: Continue in same directory; `parent_run_id` is a no-op (self-reference). The spec says "creates a new run" which makes this non-compliant.
- **Recommended default:** Option A. The spec is explicit. Record in `REVIEW.md`.

Pending that decision: add `--parent-run <RUN_ID>` to `RunArgs` with a `value_parser` that validates `s.starts_with("run_")`. Add `continuation_of` field set from `--parent-run`. Set `ancestry_depth` by reading parent's depth + 1 (if parent directory exists; otherwise 0 + 1 = 1). Handle `rings resume` creating a new run directory with `parent_run_id` set.

Add `rings show` as `Command::Show(ShowArgs)` where `ShowArgs { run_id: String }`, dispatching to `rings inspect --show summary`. This is required by the spec and is a prerequisite for `rings inspect` to be spec-compliant.

**Tests:**
- [ ] Fresh run: `parent_run_id = None`, `ancestry_depth = 0` in `run.toml`
- [ ] `rings resume <id>`: new run has `parent_run_id = <id>`, `ancestry_depth = 1`
- [ ] `rings run --parent-run <id>`: sets `continuation_of = <id>`, `ancestry_depth = depth(parent) + 1`
- [ ] `--parent-run` with nonexistent run ID: produces clear error, not panic
- [ ] `--parent-run` with malformed value (no `run_` prefix): rejected at parse time by `value_parser`
- [ ] Old `run.toml` without ancestry fields deserializes with None/0 defaults (literal TOML fixture)
- [ ] `rings show <id>` dispatches to inspect summary view

**Steps:**
- [ ] Resolve new-run-vs-same-directory architectural question; record decision in `REVIEW.md`
- [ ] Add `Show(ShowArgs)` to `Command`; wire to inspect summary rendering
- [ ] Add `--parent-run` to `RunArgs` with `value_parser`
- [ ] Update `resume_inner` per architectural decision
- [ ] Write ancestry fields to `run.toml` and `state.json` at run start

---

### Task 5: F-037/F-038/F-039 — Error Classification

**Files:** `src/error_classify.rs` (new), `src/engine.rs`, `src/display.rs`, `src/lib.rs`

Create `src/error_classify.rs` with `ErrorClass { Quota, Auth, Unknown }` and `classify(output: &str, profile: &CompiledErrorProfile) -> ErrorClass`. `classify()` must receive `output.combined` (raw combined stdout+stderr from the executor), not `response_text` — quota and auth messages are typically in stderr and will not appear in the JSON `result` field. Classification runs only on non-zero exit codes.

Wire `classify()` into the engine's executor-error path: call after collecting `output.combined`, pass result into `ExitReason::ExecutorError(failure_reason)`, propagate through `make_state_snapshot` and `EngineResult`.

Add `print_quota_error(run_number: u32, cycle: u32, phase_name: &str, run_id: &str, cumulative_cost: f64)` and `print_auth_error(...)` to `display.rs` matching spec output templates from error-handling.md lines 62–76. Update `print_executor_error` to dispatch by `ErrorClass`. Define display function signatures before implementation — record the spec-required format in `REVIEW.md` under Decisions.

**Tests:**
- [ ] `classify()` returns `Quota` for each quota pattern (case-insensitive): `"usage limit reached"`, `"rate limit"`, `"quota exceeded"`, `"too many requests"`, `"429"`, `"claude.ai/settings"`
- [ ] `classify()` returns `Auth` for each auth pattern (case-insensitive): `"authentication"`, `"invalid api key"`, `"unauthorized"`, `"401"`, `"please log in"`, `"not logged in"`
- [ ] First-match-wins: both patterns present → `Quota` (quota patterns checked first)
- [ ] `classify()` returns `Unknown` when no patterns match
- [ ] `"none"` profile always returns `Unknown` regardless of output content
- [ ] Custom profile matches specified strings only
- [ ] `classify()` on a string where the quota pattern is only in the "stderr half" (after the `\n` separator) returns `Quota` — verifies `output.combined` is used, not `response_text`
- [ ] `"429"` appearing in non-error context (e.g., `"processing 429 records"`) on non-zero exit still classifies — confirming classification is gated on non-zero exit, not pattern location
- [ ] Integration: engine exits code 3 with `failure_reason = "quota"` (lowercase in `state.json`) when executor output contains quota pattern
- [ ] Integration: engine exits code 3 with `failure_reason = "auth"` for auth patterns
- [ ] Integration: engine exits code 3 with `failure_reason = "unknown"` for unmatched non-zero exit
- [ ] Security: new CLI flags (`--quota-backoff`, `--quota-backoff-delay`, `--quota-backoff-max-retries`, `--parent-run`) do not appear in executor command arguments

**Steps:**
- [ ] Create `src/error_classify.rs` with `classify()` pure function
- [ ] Wire `classify(output.combined, &compiled_profile)` into engine executor-error path
- [ ] Add `print_quota_error` / `print_auth_error` to `display.rs` per spec templates
- [ ] Update `main.rs` to dispatch display function based on `EngineResult.failure_reason`
- [ ] Expose `error_classify` in `src/lib.rs`

---

### Task 6: F-044 — Quota Backoff

**Files:** `src/backoff.rs` (new), `src/engine.rs`, `src/audit.rs`, `src/cli.rs`, `src/lib.rs`

Create `src/backoff.rs` with `QuotaBackoff { enabled: bool, delay_secs: u64, max_retries: u32, current_retries: u32 }`. Methods: `should_retry() -> bool` (returns false immediately when `enabled = false` or retries exhausted), `record_retry(&mut self)`. The `wait()` method must call `interruptible_sleep` (from Task 1) — **not** `std::thread::sleep` — so Ctrl+C during the backoff delay is honored within ~200 ms. Tests must pass `duration = 0` to avoid real-time delays.

The backoff retry loop in the engine must wrap the executor call without advancing `RunSchedule` — `total_runs` increments only on a final (non-retried) execution. Discard the old `RunHandle` entirely after a failed run; spawn a fresh handle for the retry. Do not re-signal an already-exited process.

If a retry exits non-zero with a non-quota classification (auth or unknown), exit immediately with that classification — do not consume a retry attempt. Record this design decision in `REVIEW.md`.

On backoff exhaustion, write `failure_reason = "quota"` to state. Record this in `REVIEW.md` under Decisions.

Extend `write_run_log` in `audit.rs` to accept `retry_count: Option<u32>`: when `Some(n)`, write to `{run:03}-retry-{n}.log`; when `None`, write to `{run:03}.log`. The original failed log (`007.log`) is retained; each retry gets its own file.

Add `--quota-backoff`, `--quota-backoff-delay <SECS>`, `--quota-backoff-max-retries <N>` to CLI with `#[arg(help = "...")]` doc comments including defaults. Add `requires = "quota_backoff"` on the delay and max-retries flags. TOML overrides apply; CLI takes precedence.

**Tests:**
- [ ] `QuotaBackoff::should_retry()` returns `false` immediately when `enabled = false`
- [ ] `QuotaBackoff` state machine: `should_retry()` returns true until max_retries exhausted, then false
- [ ] TOML deserialization of backoff config fields
- [ ] CLI `--quota-backoff-delay` and `--quota-backoff-max-retries` rejected when `--quota-backoff` absent
- [ ] Integration: quota error triggers retry; second attempt succeeds — run number not incremented, total_runs unchanged
- [ ] Integration: max retries exhausted — exits code 3 with `failure_reason = "quota"`, state saved
- [ ] Integration: Ctrl+C during backoff delay triggers cancellation within ~200 ms (delay set to 0 in test)
- [ ] Integration: retry log file sequence: `007.log` exists (original failed attempt), `007-retry-1.log` exists (first retry), `007-retry-2.log` exists (second retry)
- [ ] Integration: `quota_backoff = true` + `error_profile = "none"` → no retry (classify returns Unknown)
- [ ] Integration: auth error does NOT trigger quota backoff even when `quota_backoff = true`; exits immediately with `failure_reason = "auth"`
- [ ] Integration: retry exits with non-quota error → exits immediately without consuming retry attempt
- [ ] Integration: `quota_backoff = false` (default) exits immediately on first quota error

**Steps:**
- [ ] Create `src/backoff.rs` with `QuotaBackoff` state machine
- [ ] Extend `write_run_log` to accept `retry_count: Option<u32>`
- [ ] Add backoff flags to `RunArgs` in `src/cli.rs`
- [ ] Implement backoff retry loop in engine executor-error path
- [ ] Expose `backoff` in `src/lib.rs`

---

### Task 7: F-117/F-118 — File Manifest + Diff Detection

**Files:** `src/manifest.rs` (new), `src/engine.rs`, `src/audit.rs`, `src/lib.rs`, `Cargo.toml`

Create `src/manifest.rs` implementing `compute_manifest`, `write_manifest_gz`, `read_manifest_gz`, `diff_manifests`.

**Atomicity:** `write_manifest_gz` must write to a `.tmp` companion file in the same directory, then `rename` to final name — same pattern as `StateFile::write_atomic`. Never write directly to the final path.

**Directory creation:** `write_manifest_gz` must call `create_dir_all` on the `manifests/` parent before writing.

**Manifest path:** `output_dir/<run-id>/manifests/` per spec. The `000-before.json.gz` baseline is captured before the first executor spawn. On resume, if `000-before.json.gz` already exists, skip it (preserve original baseline). Record this in `REVIEW.md` under Decisions.

**Exclusions (hardcoded, cannot be overridden):**
```
**/.env, **/.env.*, **/*_rsa, **/*_ed25519, **/*.pem, **/*.key, **/.netrc, **/*.pfx, **/*.p12, **/.git/**
```
These are checked as a non-overridable second pass, after `manifest_ignore` glob processing. Additionally, `output_dir` itself is excluded as an absolute-path prefix check (separate from glob patterns) — this covers the case where `output_dir` is inside `context_dir`.

**Glob patterns:** Use `globset = "0.4"` crate (not `glob = "0.3"` — unmaintained, broken `**` semantics). Compile `GlobSet` at workflow-load time.

**`FileEntry`:** Use `chrono::DateTime<Utc>` for `modified`, formatted as RFC 3339 for JSON serialization. Keep `SystemTime` in the in-memory mtime cache only.

**mtime cache:** Load from previous manifest file on disk at the start of each `compute_manifest` call; build a `HashMap<PathBuf, (SystemTime, [u8;32])>` in memory. Re-reading from disk is simpler and correct for resume cases.

**`FileDiff`:** Internal struct `{ added, modified, deleted }`. When appending to `CostEntry`, use `files_added: u32`, `files_modified: u32`, `files_deleted: u32`, `files_changed: u32` (sum). Use `#[serde(rename)]` to ensure wire format matches spec field names.

**Manifest write ordering:** After state and costs persistence (manifest is observability, not correctness-critical). Manifest write errors are logged as advisory warnings; they do not fail the run.

**Symlinks:** Follow them; skip broken symlinks with a warning. Record this in `REVIEW.md` under Decisions.

**Large-file-count warning:** Emit advisory warning when `compute_manifest` finds >10,000 files.

**New dependencies:**
- `flate2 = { version = "1", default-features = false, features = ["rust_backend"] }`
- `sha2 = "0.10"`
- `globset = "0.4"`

**Tests:**
- [ ] `compute_manifest` produces correct SHA-256 for known file content
- [ ] mtime optimization reuses SHA-256 when mtime unchanged from previous manifest
- [ ] All 9 credential patterns are excluded (test each individually): `.env`, `.env.*`, `*_rsa`, `*_ed25519`, `*.pem`, `*.key`, `.netrc`, `*.pfx`, `*.p12`
- [ ] `.git/` directory always excluded
- [ ] Credential patterns excluded even when `manifest_ignore = []` (no user-specified patterns)
- [ ] `output_dir` excluded when it is inside `context_dir` (e.g., `context_dir = "."`, `output_dir = "./rings-output"`)
- [ ] Custom `manifest_ignore` patterns work correctly
- [ ] `write_manifest_gz` / `read_manifest_gz` gzip roundtrip preserves content
- [ ] Manifest gzip roundtrip with non-ASCII / unicode filenames
- [ ] Non-UTF-8 path produces clear error or is skipped (not silent corruption)
- [ ] Atomic write: truncated `.tmp` file does not leave a corrupt final manifest
- [ ] `diff_manifests` detects added, modified, and deleted files correctly
- [ ] Unchanged files do not appear in any diff category
- [ ] Integration: `000-before.json.gz` created before first run
- [ ] Integration: manifests written to correct paths after each run
- [ ] Integration: diff data appended to `costs.jsonl` `run_end` records
- [ ] Integration: manifest not written when executor exits non-zero (no `NNN-after.json.gz` for failed runs)
- [ ] Integration: large-file-count warning emitted when >10,000 files

**Steps:**
- [ ] Add `globset` to `Cargo.toml`; remove `glob` (do not add `glob = "0.3"`)
- [ ] Add `flate2`, `sha2` to `Cargo.toml`
- [ ] Create `src/manifest.rs`
- [ ] Integrate `compute_manifest` hooks into engine (before first run, after each successful run)
- [ ] Expose `manifest` in `src/lib.rs`

---

### Task 8: F-072/F-097 — `rings inspect` + Summary View

**Files:** `src/inspect.rs` (new), `src/cli.rs`, `src/main.rs`, `src/lib.rs`

Create `src/inspect.rs` with `build_summary(run_dir: &Path) -> Result<RunSummary>` aggregating `run.toml`, `state.json`, `costs.jsonl`. `RunSummary` includes status, cycles, cost, phase breakdown (`Vec<PhaseSummary>`), files changed (sum across all runs), and ancestry.

`InspectView` with `#[derive(ValueEnum)]`: use `#[value(name = "files-changed")]` for kebab-case names. `DataFlow` view should degrade gracefully with a "phase contracts not yet available" message — phase contract spec does not exist yet; record in `REVIEW.md` under Open Questions.

`InspectArgs`: `run_id: String`, `show: Vec<InspectView>` with `#[arg(long, action = clap::ArgAction::Append)]`, `cycle: Option<u32>`, `phase: Option<String>`. **Do not add a local `--output-format` / `--format` field** — the global `--output-format` flag on `Cli` is inherited automatically; a local copy would produce duplicate-flag errors. Record this decision in `REVIEW.md`.

Default view when `show.is_empty()`: use `if show.is_empty() { show = vec![InspectView::Summary] }` in dispatch, not a clap `default_value` (which would always include `summary` even when `--show cycles` is given).

`rings lineage` traversal: follow `parent_run_id` chain across run directories. Guard against cycles (cap at 1000 hops) and missing run directories (stop with warning when linked run directory not found).

Handle partial runs gracefully: missing `state.json`, empty `costs.jsonl`, runs still in `"running"` status. Exit code 2 for run ID not found or data unreadable.

`rings completions <SHELL>` hidden subcommand: add `Completions(CompletionsArgs)` variant to `Command` with `#[command(hide = true)]`, wire to `clap_complete::generate()`. Required for shell completions to include new `inspect`, `lineage`, `show` subcommands.

**Tests:**
- [ ] `build_summary` correctly parses a well-formed run directory with `run.toml`, `state.json`, `costs.jsonl`
- [ ] Phase breakdown aggregation from `costs.jsonl`
- [ ] Human rendering format matches spec layout
- [ ] JSONL rendering produces valid JSON
- [ ] `--cycle` and `--phase` filters work correctly
- [ ] Summary gracefully degrades when manifests absent (`manifest_enabled = false`)
- [ ] Summary gracefully degrades when `costs.jsonl` is absent or empty
- [ ] `InspectView::from_str("files-changed")` succeeds (kebab-case validation)
- [ ] `--show summary --show costs` (two `--show` flags) renders both views without duplication — verify `args.show.len() == 2`
- [ ] `rings inspect <unknown_run_id>` exits code 2, writes error to stderr, nothing to stdout
- [ ] `rings inspect <id>` on a run still in `"running"` status handles partial data gracefully
- [ ] `rings lineage` traversal stops with warning when linked run directory not found
- [ ] `rings lineage` traversal caps at 1000 hops (cycle detection)
- [ ] Summary shows ancestry info when `parent_run_id` present
- [ ] Integration: `rings inspect <run_id>` after a completed run shows correct summary

**Steps:**
- [ ] Add `Show(ShowArgs)`, `Inspect(InspectArgs)`, `Lineage(LineageArgs)`, `Completions(CompletionsArgs)` to `Command` in `src/cli.rs`
- [ ] Ensure all new flags have `#[arg(help = "...")]` doc comments with defaults
- [ ] Create `src/inspect.rs` with `build_summary`, view rendering, lineage traversal
- [ ] Wire `Command::Inspect`, `Command::Show`, `Command::Lineage`, `Command::Completions` in `src/main.rs`
- [ ] Expose `inspect` in `src/lib.rs`

---

### Open Decisions

| ID | Decision | Recommendation |
|----|----------|----------------|
| D-1 | `rings resume` creates new run directory vs continues in same directory | **Create new run directory** per spec ("creates a new run"); record run_id of old dir as `parent_run_id`. Record architectural decision in REVIEW.md before starting Task 4. |
| D-2 | `FailureReason::Timeout` present in plan but not in error-handling spec | Retain it — existing code already writes `"timeout"`. Confirm against executor-integration spec. Record in REVIEW.md under Open Questions if spec is ambiguous. |
| D-3 | On quota backoff exhaustion, what `failure_reason` is written? | Write `failure_reason = "quota"` — the triggering cause was quota. Record in REVIEW.md under Decisions before F-044. |
| D-4 | Retry exits non-zero with non-quota error | Exit immediately without consuming a retry attempt. Record in REVIEW.md. |
| D-5 | Manifest ordering relative to state persistence | After state and costs; manifest errors are advisory warnings, not run failures. |
| D-6 | On resume, should `000-before.json.gz` be re-captured? | Preserve original baseline; skip if already exists. Record in REVIEW.md. |
| D-7 | `ancestry_depth = 0` on root runs: omit or include? | Serialize all three ancestry fields unconditionally (format is self-describing). |
| D-8 | `--output-format` global vs local on `InspectArgs` | Use global flag only; no local copy. |
| D-9 | `glob = "0.3"` vs `globset = "0.4"` | Use `globset = "0.4"` — maintained, correct `**` semantics. |
| D-10 | fsync before rename for manifests | Accept theoretical power-loss window; document in REVIEW.md. |
| D-11 | `delay_between_cycles` type (`u64` vs `DurationField`) | Use `u64` (consistent with `delay_between_runs`); record inconsistency with duration-string fields in REVIEW.md under Open Questions. |
| D-12 | Manifest symlinks | Follow symlinks; skip broken symlinks with advisory warning. |

### Spec Gaps

- **`continuation_of` vs `parent_run_id` ambiguity:** `run-ancestry.md` does not define behavior when both `rings resume <id>` and `--parent-run <other-id>` are specified simultaneously. Record in REVIEW.md under Open Questions before Task 4.
- **`manifest_mtime_optimization` not in spec config block:** `file-lineage.md` mentions it only in a security note, not in the config block. Record in REVIEW.md under Open Questions; do not edit the spec.
- **`FailureReason::Timeout` not explicitly listed in error-handling spec:** Verify against executor-integration spec. Record if unresolved.
- **`snapshot_cycles` startup warning behavior:** spec shows a TTY-gated `[y/N]` prompt with no defined non-interactive fallback. Record as Open Question; default behavior for non-TTY must be decided during implementation (recommend: skip warning, proceed).
- **`rings show` existing vs new:** spec (`inspect-command.md` line 188) refers to it as an "existing command, updated" but no `Show` variant exists in `src/cli.rs`. Treat as new. Record in REVIEW.md.
- **`rings lineage` not in `completion-and-manpage.md`:** man page spec omits `rings-lineage(1)` detail. Record as Open Question; implement `--output-format` via global flag.
- **`--show data-flow` requires phase contracts:** No phase contract schema exists in `PhaseConfig` or any spec. `DataFlow` view should degrade gracefully. Record as Open Question.
- **Limit detection in executor output** (error-handling.md lines 109–118, "N requests remaining"): not in this batch's feature list. Record as explicit deferral in REVIEW.md.
- **`000-before.json.gz` timing on first-ever run:** manifests must go into `output_dir/<run-id>/manifests/` but the run directory may not exist yet when the before-manifest is captured. Engine must `create_dir_all` for the manifests directory before spawning the first executor.
- **`quota_backoff_delay` integer-only vs duration string:** inconsistent with `delay_between_runs`. Record in REVIEW.md under Open Questions.
- **`build.rs` for man page generation:** `completion-and-manpage.md` specifies man pages generated at build time via `clap_mangen`. No `build.rs` exists. Defer to a separate task; file in `TECH_DEBT.md`.

---
