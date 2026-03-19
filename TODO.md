# TODO

Implementation tasks, ready to build. The `/build` command picks up the next task from here.

---

## F-182: `rings init` — Workflow Scaffolding

**Spec:** `specs/cli/commands-and-flags.md` lines 232–293

**Summary:** `rings init [NAME]` scaffolds a new, immediately runnable `.rings.toml` file so users don't have to write boilerplate by hand.

### Task 1: CLI subcommand and argument parsing

**Files:** `src/cli.rs`, `src/main.rs`

**Steps:**
- [x] Add `Init(InitArgs)` variant to `Command` enum in `src/cli.rs`
- [x] Define `InitArgs` struct:
  - `name: Option<String>` — positional, defaults to `"workflow"`
  - `--force` (`bool`) — overwrite existing file
  - (global `--output-format` already exists on `Cli`)
- [x] Add `Command::Init(args) => cmd_init(args, cli.output_format)` match arm in `main.rs`
- [x] Stub `cmd_init` that just prints the resolved path and returns `Ok(())`

**Tests:**
- [x] `rings init` parses with no args (name defaults to None)
- [x] `rings init my-task` parses name as `Some("my-task")`
- [x] `rings init --force` parses force flag

---

### Task 2: Path resolution and validation

**Files:** `src/main.rs` (or new `src/init.rs`)

**Steps:**
- [x] Implement `resolve_init_path(name: Option<&str>) -> Result<PathBuf>`:
  - Default name: `"workflow"` → `./workflow.rings.toml`
  - Custom name: `"my-task"` → `./my-task.rings.toml`
  - Relative path: `"workflows/my-task"` → `./workflows/my-task.rings.toml`
  - Appends `.rings.toml` suffix if not already present
  - Rejects paths containing `..` components (exit code 2)
- [x] Check if target file exists: if yes and `--force` not set, print error and exit 2
- [x] If path has directory components (e.g. `workflows/`), verify parent dir exists (don't create it — exit 2 if missing)

**Tests:**
- [x] Default name resolves to `workflow.rings.toml`
- [x] Custom name appends `.rings.toml`
- [x] Name already ending in `.rings.toml` is not double-suffixed
- [x] Path with `..` is rejected with exit code 2
- [x] Existing file without `--force` exits 2
- [x] Existing file with `--force` succeeds

---

### Task 3: Template content and atomic write

**Files:** `src/main.rs` (or `src/init.rs`)

**Steps:**
- [x] Define the scaffold template as a const string. Must include:
  - `[workflow]` with `completion_signal = "TASK_COMPLETE"`, `context_dir = "."`, `max_cycles = 10`, `completion_signal_mode = "line"`, `budget_cap_usd = 5.00`
  - One `[[phases]]` block named `"builder"` with `prompt_text` containing:
    - A useful starter prompt
    - The completion signal string embedded in the prompt text (so F-151 check passes)
    - A comment block listing all template variables: `{{phase_name}}`, `{{cycle}}`, `{{max_cycles}}`, `{{iteration}}`, `{{run}}`, `{{cost_so_far_usd}}`
- [x] Write atomically: write to `<path>.tmp`, then `std::fs::rename` to final path
- [x] On success, human mode: print `Created <path>` to stderr, then print a hint: `Run it with:  rings run <path>`
- [x] On success, JSONL mode: emit `{"event":"init_complete","path":"<absolute_path>"}` to stdout

**Tests:**
- [x] Scaffolded file parses as a valid `Workflow` via `Workflow::from_str`
- [x] Scaffolded file passes `rings run --dry-run` (completion signal found in prompt)
- [x] `budget_cap_usd` is present (F-116 no-cap warning won't fire)
- [x] Template variables comment is present in prompt_text
- [x] Atomic write: `.tmp` file does not remain after success
- [x] JSONL output is valid JSON with correct `event` and `path` fields

---

## F-192: `rings update` — Self-Update Command

**Spec:** `specs/cli/commands-and-flags.md`

**Summary:** `rings update` downloads and installs the latest nightly release from GitHub by shelling out to `install.sh`. Reuses existing platform detection, checksum verification, and install logic.

### Task 1: CLI subcommand

**Files:** `src/cli.rs`, `src/main.rs`

**Steps:**
- [x] Add `Update` variant to `Command` enum in `src/cli.rs` (no args struct needed — no flags)
- [x] Add `Command::Update => cmd_update()` match arm in `main.rs`
- [x] Implement `cmd_update()`:
  1. Check `curl` is on PATH (`which curl`); if not, print error and exit 1
  2. Check `bash` is on PATH (`which bash`); if not, print error and exit 1
  3. Get current binary path via `std::env::current_exe()?.canonicalize()?`
  4. Print `Updating rings...` to stderr
  5. Download `install.sh` to a temp file: `curl -fsSL https://raw.githubusercontent.com/pinecone-io/rings/main/install.sh -o <tmpfile>`
  6. Run `bash <tmpfile> <current_binary_path>`, inheriting stdout/stderr
  7. Clean up temp file
  8. If install succeeded (exit 0): exit 0
  9. If install failed: print error, exit 1

**Tests:**
- [x] `rings update` parses as the Update command (CLI parsing test)
- [x] `cmd_update` detects missing `curl` and returns error (mock PATH)
- [x] `cmd_update` detects missing `bash` and returns error (mock PATH)

---

## Batch: JSONL Output Mode — F-095, F-126, F-127, F-139, F-140

**Spec:** `specs/observability/runtime-output.md` (JSONL sections)

**Summary:** Enable `--output-format jsonl` for structured event output to stdout, suitable for scripting, CI, and dashboards. All human-mode display is suppressed in JSONL mode. Every event carries `event`, `run_id`, and `timestamp`.

### Task 1: Event types and emit helper

**Files:** `src/events.rs` (new), `src/lib.rs`

**Steps:**
- [x] Create `src/events.rs` with serializable event structs (all derive `Serialize`):
  - `StartEvent` — `event: "start"`, `run_id`, `workflow`, `rings_version`, `schema_version: 1`, `timestamp`
  - `RunStartEvent` — `event: "run_start"`, `run_id`, `run`, `cycle`, `phase`, `iteration`, `total_iterations`, `template_context: serde_json::Value`, `timestamp`
  - `RunEndEvent` — `event: "run_end"`, `run_id`, `run`, `cycle`, `phase`, `iteration`, `cost_usd: Option<f64>`, `input_tokens: Option<u64>`, `output_tokens: Option<u64>`, `exit_code: i32`, `produces_violations: Vec<String>`, `cost_confidence`, `total_iterations`, `timestamp`
  - `CompletionSignalEvent` — `event: "completion_signal"`, `run_id`, `run`, `cycle`, `phase`, `signal`, `timestamp`
  - `ExecutorErrorEvent` — `event: "executor_error"`, `run_id`, `run`, `cycle`, `phase`, `error_class` (quota/auth/unknown), `exit_code`, `message`, `timestamp`
  - `CanceledEvent` — `event: "canceled"`, `run_id`, `runs_completed`, `cost_usd`, `timestamp`
  - `BudgetCapJsonlEvent` — `event: "budget_cap"`, `run_id`, `cost_usd`, `budget_cap_usd`, `runs_completed`, `timestamp`
  - `MaxCyclesEvent` — `event: "max_cycles"`, `run_id`, `cycles`, `runs_completed`, `cost_usd`, `timestamp`
  - `DelayStartEvent` — `event: "delay_start"`, `run_id`, `run`, `cycle`, `phase`, `delay_secs`, `reason` (inter_run/inter_cycle/quota_backoff), `timestamp`
  - `DelayEndEvent` — `event: "delay_end"`, `run_id`, `run`, `timestamp`
  - `SummaryEvent` — `event: "summary"`, `run_id`, `status`, `cycles`, `runs`, `cost_usd`, `duration_secs`, `phases: Vec<PhaseSummary>`, `timestamp`
  - `FatalErrorEvent` — `event: "fatal_error"`, `run_id: Option<String>` (null if pre-run), `message`, `timestamp`
- [x] Add `emit_jsonl(event: &impl Serialize)` helper that serializes to JSON and prints to stdout with a newline
- [x] Add `now_iso8601() -> String` helper for consistent timestamp formatting
- [x] Register `pub mod events;` in `src/lib.rs`

**Tests:**
- [x] Each event type serializes to JSON with correct `event` field name
- [x] `run_id` and `timestamp` are always present in serialized output
- [x] `FatalErrorEvent` serializes `run_id` as `null` when None
- [x] `emit_jsonl` produces valid single-line JSON (no embedded newlines)

---

### Task 2: Wire output_format into engine

**Files:** `src/engine.rs`, `src/main.rs`

**Steps:**
- [x] Add `output_format: OutputFormat` field to `EngineConfig`
- [x] Pass `cli.output_format` from `run_inner` and `resume_inner` in `main.rs` into `EngineConfig`
- [x] In `run_workflow`, wrap all `display::print_*` calls with `if config.output_format == Human` guards
- [x] When `output_format == Jsonl`, suppress: `print_run_header`, `print_run_start`, `print_run_elapsed`, `print_run_result`, `print_cycle_boundary`, `print_cycle_cost`, and all other stderr display calls
- [x] In `main.rs`, similarly guard `print_completion`, `print_cancellation`, `print_executor_error`, etc. behind human-mode checks

**Tests:**
- [x] Engine with `output_format: Jsonl` produces no stderr output from display functions
- [x] Engine with `output_format: Human` still produces stderr output as before (regression check)

---

### Task 3: Emit lifecycle events (start, summary, fatal_error)

**Files:** `src/engine.rs`, `src/main.rs`

**Steps:**
- [x] At the top of `run_workflow`, if JSONL mode: emit `StartEvent` with run_id, workflow file path, rings version (`env!("CARGO_PKG_VERSION")`), schema_version 1
- [x] At the end of `run_workflow` (all exit paths), if JSONL mode: emit `SummaryEvent` with status (completed/canceled/max_cycles/budget_cap/executor_error), cycles completed, total runs, total cost, duration_secs, phase breakdown
- [x] In `main.rs`, for fatal errors before `run_workflow` is reached (bad TOML, missing file, executor not found): if JSONL mode, emit `FatalErrorEvent` to stdout before exiting with code 2

**Tests:**
- [x] JSONL run emits `start` as first event and `summary` as last event
- [x] `summary` event `status` field matches exit reason (completed/canceled/max_cycles/budget_cap)
- [x] `summary.phases` array has correct per-phase cost and run counts
- [x] Fatal error before engine start emits `fatal_error` event with `run_id: null`
- [x] `start` event includes correct `rings_version` and `schema_version: 1`

---

### Task 4: Emit per-run events (run_start, run_end, completion_signal, executor_error)

**Files:** `src/engine.rs`

**Steps:**
- [x] Before each `executor.spawn()` call: if JSONL mode, emit `RunStartEvent` with run number, cycle, phase, iteration, total_iterations, and `template_context` (the same variables passed to prompt rendering, as a JSON object)
- [x] After each successful run completes and cost is parsed: if JSONL mode, emit `RunEndEvent` with cost, tokens, exit_code, produces_violations, cost_confidence
- [x] When completion signal is detected: if JSONL mode, emit `CompletionSignalEvent` with the signal string
- [x] When executor exits non-zero: if JSONL mode, emit `ExecutorErrorEvent` with error_class (from FailureReason enum), exit_code, and error message

**Tests:**
- [x] Each run produces exactly one `run_start` and one `run_end` event
- [x] `run_start.template_context` includes phase_name, cycle, max_cycles, iteration, run, cost_so_far_usd
- [x] `run_end.cost_usd` is null (not 0) when cost parsing fails
- [x] `completion_signal` event is emitted between the triggering `run_end` and `summary`
- [x] `executor_error` event has correct `error_class` for quota/auth/unknown failures
- [x] Events appear in chronological order: run_start → run_end → (optional completion_signal)

---

### Task 5: Emit delay and budget events

**Files:** `src/engine.rs`

**Steps:**
- [x] Before each inter-run delay sleep: if JSONL mode, emit `DelayStartEvent` with `reason: "inter_run"`, delay_secs
- [x] After delay completes: emit `DelayEndEvent`
- [x] Before each inter-cycle delay (if cycle_delay configured): emit `DelayStartEvent` with `reason: "inter_cycle"`
- [x] During quota backoff waits: emit `DelayStartEvent` with `reason: "quota_backoff"`, then `DelayEndEvent` after
- [x] When budget cap is reached: if JSONL mode, emit `BudgetCapJsonlEvent` (instead of or in addition to display::print_budget_cap_reached)
- [x] When max_cycles is reached without completion: if JSONL mode, emit `MaxCyclesEvent`
- [x] When canceled (Ctrl+C): if JSONL mode, emit `CanceledEvent`

**Tests:**
- [x] Inter-run delay produces `delay_start` and `delay_end` events with `reason: "inter_run"`
- [x] Budget cap produces `budget_cap` event with correct cost and cap values
- [x] Max cycles produces `max_cycles` event with cycle count and cost
- [x] Cancellation produces `canceled` event with runs_completed and cost

---

### Task 6: stdout/stderr separation and integration test

**Files:** `src/main.rs`, `src/engine.rs`

**Steps:**
- [ ] Verify: in JSONL mode, all structured events go to stdout (println!), human display is suppressed
- [ ] Verify: in JSONL mode, only unstructured fatal errors (before first event can be emitted) go to stderr
- [ ] Verify: in Human mode, stdout is empty (all display goes to stderr via eprintln!)
- [ ] Add `--step` + `--output-format jsonl` conflict check: exit 2 with error message (per spec)
- [ ] Write integration test: run a complete 2-cycle workflow in JSONL mode, capture stdout, parse each line as JSON, verify event sequence: start → run_start → run_end → ... → summary
- [ ] Write integration test: verify stdout is empty in human mode

**Tests:**
- [ ] Full JSONL workflow produces parseable JSON on every stdout line
- [ ] Event sequence is: start, (run_start, run_end)+, completion_signal?, summary
- [ ] `jq` can filter events by run_id (all events share the same run_id)
- [ ] `--step --output-format jsonl` exits 2 with error message
- [ ] Human mode produces zero bytes on stdout

---
