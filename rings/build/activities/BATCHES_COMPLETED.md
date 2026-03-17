## Batch: Process Safety + Cost Safety — 2026-03-16

**Features:** F-020 (Timeout Per Run), F-052 (SIGTERM Handling), F-054 (Subprocess Graceful Shutdown), F-055 (Context Directory Lock), F-112 (Budget Cap), F-115 (Low-Confidence Cost Warning), F-116 (No Budget Cap Warning)

---

### Task 1: Add `nix` dependency and duration parser

**Files:** `Cargo.toml`, new `src/duration.rs`

**Steps:**
- [x] Add `nix = { version = "0.29", features = ["signal", "process"] }` under `[target.'cfg(unix)'.dependencies]` in `Cargo.toml`
- [x] Move `tempfile = "3"` from `[dev-dependencies]` to `[dependencies]`
- [x] Implement `pub fn parse_duration_secs(s: &str) -> anyhow::Result<u64>` in `src/duration.rs`. Accepts integer seconds or single-char suffix strings (`"30s"`, `"5m"`, `"1h"`). All other forms are errors.
- [x] Add `DurationField` untagged enum to support both integer and string TOML values:
  ```rust
  #[derive(Deserialize)]
  #[serde(untagged)]
  enum DurationField { Secs(u64), Str(String) }
  ```
  Verify `toml::from_str("timeout_per_run_secs = 300")` produces `DurationField::Secs(300)`; if `i64`→`u64` coercion fails, use `Secs(i64)` and validate `>= 0`.

**Tests:**
- [x] `"30s"` → 30, `"5m"` → 300, `"1h"` → 3600
- [x] `"0"` / `"0s"` → `Err` (zero timeout fires immediately)
- [x] `"5min"` / `"1h30m"` / `""` → `Err`
- [x] `"  30s  "` (leading/trailing spaces) → 30 (trim before parse)
- [x] `"30S"` (uppercase suffix) → `Err`
- [x] `"5165088294h"` (overflow) → `Err` (use `checked_mul`) [Note: used `9999999999999999h` — original value doesn't overflow u64]
- [x] `toml::from_str("timeout_per_run_secs = 300")` → `DurationField::Secs(300)`

---

### Task 2: Data model changes — `WorkflowConfig`, `PhaseConfig`, `Workflow`

**Files:** `src/workflow.rs`

**Steps:**
- [x] Add to `WorkflowConfig`: `budget_cap_usd: Option<f64>`, `timeout_per_run_secs: Option<DurationField>`
- [x] Add to `PhaseConfig`: `budget_cap_usd: Option<f64>`, `timeout_per_run_secs: Option<DurationField>`
- [x] In `Workflow::validate`: resolve `DurationField` to `u64` seconds; validate `budget_cap > 0` if set
- [x] In `Workflow::validate`: add `context_dir` existence check

---

### Task 3: CLI changes — new flags on `RunArgs` and `ResumeArgs`

**Files:** `src/cli.rs`

**Steps:**
- [x] Add `RunArgs::budget_cap: Option<f64>` (`--budget-cap <DOLLARS>`)
- [x] Add `RunArgs::timeout_per_run: Option<String>` (`--timeout-per-run <DURATION>`)
- [x] Add `RunArgs::force_lock: bool` (`--force-lock`)
- [x] Add same three fields to `ResumeArgs`
- [x] Validate `budget_cap > 0` in `run_inner`/`resume_inner`
- [x] Wire CLI overrides: CLI `budget_cap` and `timeout_per_run` take precedence over TOML values

---

### Task 4: `RunHandle` abstraction — refactor `Executor` trait

**Files:** `src/executor.rs`

**Steps:**
- [x] Add `RunHandle` trait with methods: `wait() -> Result<ExecutorOutput>`, `pid() -> u32`, `send_sigterm() -> Result<()>`, `send_sigkill() -> Result<()>`, `partial_output() -> Result<String>`
- [x] Add `spawn(&self, invocation: &Invocation, verbose: bool) -> Result<Box<dyn RunHandle>>` to `Executor` trait; add `Send + Sync` bounds
- [x] Implement `ClaudeRunHandle` wrapping `std::process::Child`:
  - Use `CommandExt::process_group(0)` so signals reach the whole process group
  - `child.id()` serves as both PID and PGID
  - `ESRCH` on signal calls → treat as success (process already gone)
  - Wait loop uses `try_wait()` in 100ms slices (never blocking `child.wait()`)
  - In verbose mode: use `child.wait()` path, join reader threads after wait
- [x] Implement `MockRunHandle`:
  ```rust
  pub struct MockRunHandle {
      pub output: ExecutorOutput,
      pub wait_delay_ms: u64,
      pub ignores_sigterm: bool,
      pub sigterm_called: Arc<AtomicBool>,
      pub sigkill_called: Arc<AtomicBool>,
  }
  ```
- [x] Change `MockExecutor` from `RefCell` to `Mutex<Vec<ExecutorOutput>>`
- [x] Keep existing `run()` method as a thin wrapper over `spawn()` + `wait()`

---

### Task 5: `CancelState` and signal handling refactor

**Files:** `src/main.rs`, new `src/cancel.rs`

**Steps:**
- [x] Define `CancelState` backed by `AtomicU8`: `0=NotCanceled`, `1=Canceling`, `2=ForceKill`; use `SeqCst` throughout; second-signal transition uses `compare_exchange`, not plain `store`
- [x] Install `ctrlc` handler once in `main()`, share `Arc<CancelState>`; first signal → `Canceling`; second signal while `Canceling` → `ForceKill`
- [x] Under `#[cfg(unix)]`: set `SIG_IGN` for SIGPIPE in `main()` before any subprocess spawning
- [x] Add `#[cfg(not(unix))] compile_error!("rings requires a Unix platform")` in `src/main.rs`
- [x] Replace all `thread::sleep(delay)` in the engine with a 100ms polling loop that checks the cancel flag

---

### Task 6: Context Directory Lock

**Files:** new `src/lock.rs`, `src/main.rs`

**Steps:**
- [x] Define `LockFile { run_id: String, pid: u32 }` with `Serialize`/`Deserialize`; use JSON format: `{"run_id":"...","pid":12345}`
- [x] Define `LockError` (thiserror): variants `ActiveProcess { run_id: String, pid: u32, context_dir: PathBuf }` and `ContextDirMissing { path: PathBuf }`; error message: `"Error: Another rings run (RUN_ID, PID=N) is already using context_dir.\nWait for it to finish or use --force-lock to override."`
- [x] Define `ContextLock { path: PathBuf }` with `Drop` impl that removes the lock file
- [x] Implement `ContextLock::acquire(context_dir, run_id, force) -> Result<ContextLock, LockError>`:
  - Write atomically via `OpenOptions::new().write(true).create_new(true)` (`O_CREAT|O_EXCL`)
  - On `EEXIST`: read existing file, parse JSON; use `kill(pid, 0)` for liveness: `EPERM` → live → `LockError::ActiveProcess`; `ESRCH` or parse error → stale; remove and retry once; second `EEXIST` → `LockError::ActiveProcess`; `pid = 0` → treat as stale; empty file → treat as stale, acquire with warning
  - `force = true`: overwrite unconditionally
- [x] Gate entire module under `#[cfg(unix)]`
- [x] All exit paths must flow through `main()`'s return (not direct `process::exit()`) so `ContextLock::Drop` fires; `run_inner`/`resume_inner` return `Result<i32>`; `main()` calls `process::exit()` only after stack unwind
- [x] Acquire lock in both `run_inner` and `resume_inner` before `run_workflow`

**Tests:**
- [x] `kill(pid, 0)` returns `EPERM` → treated as live → `LockError::ActiveProcess`
- [x] Lock file has `pid = 0` → treated as stale, acquire succeeds
- [x] `context_dir` does not exist → `LockError::ContextDirMissing`
- [x] Second `create_new` fails after stale removal → `LockError::ActiveProcess`
- [x] Lock file is empty → treated as stale, acquire succeeds with warning

---

### Task 7: Timeout and cancellation in engine loop

**Files:** `src/engine.rs`, `src/state.rs`

**Steps:**
- [x] Add `failure_reason: Option<String>` to `StateFile` with `#[serde(default)]`
- [x] Engine calls `executor.spawn()` and holds a `RunHandle`; wait loop uses `try_wait()` in 100ms slices
- [x] In the wait loop, check in order:
  1. `ForceKill` → SIGKILL immediately
  2. `Canceling` → SIGTERM, save state, wait up to 5s, SIGKILL if subprocess ignores SIGTERM, write partial output to run log, exit 130
  3. Timeout expired → SIGTERM, save state, wait up to 5s, SIGKILL, write partial output to run log, set `failure_reason = "timeout"`, exit 2
  4. Normal exit → proceed
- [x] Correct save order on cancellation/timeout: SIGTERM → save state → wait up to 5s → SIGKILL if needed → write partial output to run log → exit
- [x] Record exit code 2 / timeout conflict with `exit-codes.md` in REVIEW.md under Conflicts

**Tests:**
- [x] `sigterm_called` on `MockRunHandle` is `true` after cancel flag fires
- [x] `sigkill_called` on `MockRunHandle` is `true` when `ignores_sigterm = true` and 5s expires
- [x] `state.json` is written BEFORE SIGKILL is sent
- [x] `NNN.log` exists and contains partial output for interrupted run

---

### Task 8: Budget cap — engine loop and display

**Files:** `src/engine.rs`, `src/display.rs`, `src/state.rs`

**Steps:**
- [x] Add `phase_costs: HashMap<String, f64>` to engine-local state; add `budget_warned_80: bool`, `budget_warned_90: bool` per scope (global + per-phase)
- [x] After each run, update `phase_costs[phase_name]` and `cumulative_cost`; check global and per-phase budget caps using `>=`:
  - ≥80%: emit `⚠  Budget: $X.XX spent — 80% of $Y.YY cap.` (once per scope)
  - ≥90%: emit `⚠  Budget: $X.XX spent — 90% of $Y.YY cap. Approaching limit.` (once per scope)
  - ≥100%: call `print_budget_cap_reached(scope)`, save state, exit 4
- [x] `print_budget_cap_reached` display format: `"Error: Budget cap of $X.XX reached (spent $Y.YY).\nrings is stopping. Resume is available."` Exit code 4, `run.toml` status `"stopped"`.
- [x] Emit JSONL events: `budget_warning { scope, pct: u8 }` and `budget_cap`; add `pct: u8` (80 or 90) to `budget_warning` event
- [x] Add `4 => "stopped"` arm to `final_status` and summary match in `main.rs`
- [x] Add `4 =>` display arm calling `print_budget_cap_reached()` in `main.rs`
- [x] On resume: reconstruct `cumulative_cost` from `costs.jsonl`; initialize `budget_warned_80`/`budget_warned_90` from reconstructed value

**Tests:**
- [x] Resume where `cumulative_cost >= 0.8 * cap` → 80% warning does NOT re-fire on first subsequent run (initialized from reconstructed cost)
- [x] Exit code 4 → `run.toml` status is `"stopped"` (added to both run_inner and resume_inner)
- [x] `print_budget_cap_reached()` output appears before process exits (called before state.write_atomic, which returns state's exit code 4)

---

### Task 9: No budget cap warning + low-confidence cost warning

**Files:** `src/main.rs`, `src/engine.rs`, `src/cost.rs`, `src/display.rs`

**Steps:**
- [x] **F-116:** In `run_inner`/`resume_inner` after workflow load: if no `budget_cap_usd` in TOML and no `--budget-cap` CLI flag, emit: `⚠  Warning: No budget cap configured. Use --budget-cap or budget_cap_usd to prevent unbounded spend.` (verbatim spec text with `⚠  ` prefix; record prefix deviation in REVIEW.md under Conflicts)
- [x] **F-115:** Add `ParseWarning` struct to `src/cost.rs`:
  ```rust
  pub struct ParseWarning {
      pub run_number: u32,
      pub cycle: u32,
      pub phase: String,
      pub confidence: ParseConfidence,
      pub raw_match: Option<String>,
  }
  ```
- [x] Fix `cost_confidence` serialization: add `#[serde(rename_all = "lowercase")]` to `ParseConfidence`
- [x] In engine loop: if `cost.confidence` is `Low` or `None`, add a `ParseWarning` to an accumulator vec
- [x] Add `parse_warnings: Vec<ParseWarning>` to `EngineResult`
- [x] Implement `print_parse_warnings(warnings: &[ParseWarning])` in `display.rs`: show up to 10, then `"... and N more low-confidence cost parse warnings."`; `ParseConfidence::Low` with non-None `raw_match` → include raw match snippet; `ParseConfidence::None` → "could not be parsed" message
- [x] In `main.rs`: after engine completes, call `print_parse_warnings` with the accumulated warnings

**Tests:**
- [x] `ParseConfidence::Low` with non-None `raw_match` → warning includes raw match snippet
- [x] `ParseConfidence::None` → "could not be parsed" message (no snippet)
- [x] 11 low-confidence runs → shows 10 warnings + `"... and 1 more"`

---
