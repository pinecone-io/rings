# Ready to Implement Queue

Finalized, reviewed implementation plans ready for coding.
Each entry is the synthesized output of a full plan-create + plan-review cycle.

---

## Batch: Process Safety + Cost Safety — 2026-03-16

**Features:** F-020 (Timeout Per Run), F-052 (SIGTERM Handling), F-054 (Subprocess Graceful Shutdown), F-055 (Context Directory Lock), F-112 (Budget Cap), F-115 (Low-Confidence Cost Warning), F-116 (No Budget Cap Warning)

---

### Blockers

**B-1: Executor trait has no cancellation or kill surface** *(impl-rust, impl-architecture, impl-testing, impl-process-mgmt, impl-performance, impl-memory)*

`ClaudeExecutor::run()` calls `child.wait_with_output()` and blocks indefinitely. There is no way to send SIGTERM to the subprocess from outside the call. The cancellation flag (`AtomicBool`) is only checked *after* `executor.run()` returns.

**Required fix:** Split spawn from wait. Introduce a `RunHandle` trait:
```rust
pub trait Executor: Send + Sync {
    fn spawn(&self, invocation: &Invocation, verbose: bool) -> Result<Box<dyn RunHandle>>;
}

pub trait RunHandle: Send {
    fn wait(&mut self) -> Result<ExecutorOutput>;
    fn pid(&self) -> u32;
    fn send_sigterm(&self) -> Result<()>;
    fn send_sigkill(&self) -> Result<()>;
    fn partial_output(&self) -> Result<String>;
}
```
Engine calls `spawn()`, gets a handle, polls with `try_wait()` in 100ms slices checking cancel flag and timeout, sends signals when needed. `MockExecutor` implements `RunHandle` trivially.

---

**B-2: Signal handler only sets a flag; subprocess never killed during active run** *(impl-rust, impl-architecture, impl-error-handling, impl-process-mgmt)*

Current `ctrlc` handler: `canceled.store(true, SeqCst)`. Engine checks the flag only between runs. Fix: with the `RunHandle` abstraction (B-1), the engine's wait loop checks the cancel flag each iteration. When set, it calls `handle.send_sigterm()`, waits up to 5s, then `handle.send_sigkill()`.

Double Ctrl+C (F-053): use an `AtomicU8` cancel counter. First signal → SIGTERM path. Second signal → SIGKILL immediately.

---

**B-3: Inter-run delay (`thread::sleep`) is not interruptible** *(impl-process-mgmt, impl-cross-platform, impl-performance)*

Replace `thread::sleep` with a polling loop (~100ms slices) checking the cancel flag.

---

**B-4: Duration string parsing does not exist** *(impl-rust, impl-deps, impl-regex, impl-serialization)*

Implement `fn parse_duration_secs(s: &str) -> anyhow::Result<u64>` in `src/duration.rs`. Accepts integer seconds or single-char suffix strings (`"30s"`, `"5m"`, `"1h"`). All other forms are errors. TOML field needs an untagged enum:
```rust
#[derive(Deserialize)]
#[serde(untagged)]
enum DurationField { Secs(u64), Str(String) }
```

---

**B-5: `nix` crate not in dependencies** *(impl-deps, impl-cross-platform)*

Add:
```toml
[target.'cfg(unix)'.dependencies]
nix = { version = "0.29", features = ["signal", "process"] }
```
Use `nix::sys::signal::kill(Pid::from_raw(pid), Signal::SIGTERM/SIGKILL)` for sending signals. Use `kill(pid, 0)` (signal 0) for PID liveness check in F-055.

---

**B-6: Context Directory Lock entirely absent** *(impl-rust, impl-architecture, impl-filesystem, impl-error-handling, impl-cross-platform)*

No `.rings.lock` file is created, checked, or removed anywhere. `--force-lock` flag does not exist. Both `RunArgs` and `ResumeArgs` need `--force-lock`. Lock must be acquired in `run_inner` AND `resume_inner`.

---

**B-7: Lock file format unspecified** *(impl-serialization, impl-regex)*

Use JSON (consistent with `state.json`):
```json
{"run_id":"run_20240315_143022_a1b2c3","pid":12345}
```
Write atomically via `OpenOptions::new().write(true).create_new(true)` (`O_CREAT|O_EXCL`). Parse errors on read-back → treat as stale.

---

**B-8: `budget_cap_usd` and `timeout_per_run_secs` missing from data model** *(impl-cli-framework, impl-serialization, impl-error-handling)*

Add to `WorkflowConfig` and `PhaseConfig`. Add `--budget-cap`, `--timeout-per-run`, `--force-lock` to `RunArgs`. Add `--budget-cap`, `--timeout-per-run`, `--force-lock` to `ResumeArgs`.

---

**B-9: `print_budget_cap_reached()` display format unspecified** *(impl-agent-ux, impl-docs)*

```
Error: Budget cap of $X.XX reached (spent $Y.YY).
rings is stopping. Resume is available.
```
Exit code 4. `run.toml` status: `"stopped"`.

---

**B-10: No budget cap warning text unspecified** *(impl-docs)*

Use verbatim spec text from `cost-tracking.md`: `"Warning: No budget cap configured. Use --budget-cap or budget_cap_usd to prevent unbounded spend."` Adapt prefix from `Warning:` to `⚠  ` for visual consistency. Record prefix deviation in REVIEW.md under Conflicts.

---

**B-11: Low-confidence warning display format unspecified** *(impl-docs, impl-agent-ux)*

`ParseWarning` struct (add to `src/cost.rs`):
```rust
pub struct ParseWarning {
    pub run_number: u32,
    pub cycle: u32,
    pub phase: String,
    pub confidence: ParseConfidence,
    pub raw_match: Option<String>,
}
```
Display via `print_parse_warnings(warnings: &[ParseWarning])` in `display.rs`. Cap display at 10, then `"... and N more low-confidence cost parse warnings."`. Full Vec accumulated for accurate counts.

---

**B-12: `ClaudeRunHandle` verbose mode read race** *(impl-process-mgmt)*

In verbose mode, use `child.wait()` (not `wait_with_output()`). Join reader threads after wait. Use `try_wait()` polling as the primary wait mechanism.

---

**B-13: `std::process::exit()` bypasses `ContextLock::Drop`** *(impl-memory, impl-rust)*

All exit paths must flow through `main()`'s return — not direct `process::exit()` calls. `run_inner`/`resume_inner` return `Result<i32>`; `main()` calls `process::exit()` only after stack unwind, which triggers `ContextLock::Drop`. Verify no new direct `process::exit()` calls are introduced in Steps 7–9.

---

**B-14: `MockExecutor !Send` conflicts with `Executor: Send + Sync`** *(impl-rust, impl-architecture, impl-memory)*

`MockExecutor` uses `RefCell` → `!Send`. Fix: change to `Mutex<Vec<ExecutorOutput>>`. `MockRunHandle` must use `Arc<Mutex<...>>` or `Arc<AtomicBool>` for scriptable state.

---

**B-15: `DurationField` untagged enum with toml 0.8 needs verification** *(impl-rust, impl-serialization)*

Add a test in Step 1: `toml::from_str("timeout_per_run_secs = 300")` must produce `DurationField::Secs(300)`. If `i64`→`u64` coercion fails, use `Secs(i64)` and validate `>= 0`.

---

**B-16: `CancelState` AtomicU8 memory ordering unspecified** *(impl-rust, impl-memory)*

Use `SeqCst` throughout. Second-signal transition must use `compare_exchange`, not plain `store`.

---

**B-17: `#[cfg(unix)]` guards required throughout** *(impl-deps, impl-cross-platform)*

Windows is out of scope. Add `#[cfg(not(unix))] compile_error!("rings requires a Unix platform")` in `src/main.rs`. Gate all `nix` usage and `lock.rs` under `#[cfg(unix)]`.

---

**B-18: `RunHandle::wait()` must use `try_wait()` polling** *(impl-process-mgmt, impl-memory)*

Never use blocking `child.wait()`. Use `try_wait()` in 100ms slices. Reader threads joined inside `ClaudeRunHandle::wait()` after `try_wait()` returns `Some(exit_status)`.

---

**B-19: Partial output not written to run log on cancellation/timeout** *(impl-filesystem)*

After capturing `handle.partial_output()`, write to run log (`write_run_log(&runs_dir, run_spec.global_run_number, &partial)`) before saving state and exiting.

---

**B-20: `MockRunHandle` API unspecified** *(impl-testing, impl-architecture)*

```rust
pub struct MockRunHandle {
    pub output: ExecutorOutput,
    pub wait_delay_ms: u64,
    pub ignores_sigterm: bool,
    pub sigterm_called: Arc<AtomicBool>,
    pub sigkill_called: Arc<AtomicBool>,
}
```
Must not use `RefCell` (see B-14).

---

**B-21: Exit code 4 unhandled in `main.rs`** *(impl-error-handling, impl-agent-ux, impl-cli-framework)*

Add `4 => "stopped"` arm in `final_status` match. Add `4 =>` display arm in summary match. Engine returns `EngineResult { exit_code: 4 }`, `main.rs` calls `print_budget_cap_reached()` in the `4 =>` arm.

---

**B-22: `LockError` propagation type unspecified** *(impl-error-handling)*

`thiserror`-derived enum. Variants: `ActiveProcess { run_id: String, pid: u32, context_dir: PathBuf }`, `ContextDirMissing { path: PathBuf }`. Error message format:
```
Error: Another rings run (run_20240315_143022_a1b2c3, PID=12345) is already using context_dir.
Wait for it to finish or use --force-lock to override.
```

---

**B-23: `ParseWarning` struct undefined** — See B-11.

---

**B-24: State save order on cancellation/timeout is wrong** *(impl-performance)*

Correct order: SIGTERM → **save state** → wait up to 5s → SIGKILL if needed → write partial output to run log → exit.

---

**B-25: Budget cap warning flags not persisted across resume** *(impl-error-handling, impl-architecture)*

After reconstructing `cumulative_cost` from `costs.jsonl` on resume:
```rust
budget_warned_80 = cumulative_cost >= 0.8 * budget_cap;
budget_warned_90 = cumulative_cost >= 0.9 * budget_cap;
```

---

**B-26: Timeout exit code 2 conflicts with `specs/cli/exit-codes.md`** *(impl-docs)*

Use exit 2 for timeout (per `engine.md`). Record conflict with `exit-codes.md` in REVIEW.md under Conflicts as an Open Question for future spec reconciliation.

---

### Open Decisions

| ID | Decision | Recommendation |
|----|----------|----------------|
| D-1 | `RunHandle` abstraction: Option A (spawn/wait split) vs Option B (CancellationToken) | **Option A** — cleaner separation, more testable |
| D-2 | PGID capture: `CommandExt::process_group(0)`, capture `child.id()` as both PID and PGID | **Confirmed** — no `getpgid` call needed |
| D-3 | `write_run_log` timing for verbose mode | On subprocess completion; join reader threads before writing |
| D-4 | Lock file atomic write primitive | `OpenOptions::create_new(true)` (`O_CREAT\|O_EXCL`), not temp+rename |
| D-5 | Stale lock detection | `kill(pid, 0)` (POSIX signal 0); `EPERM` → treat as live; `ESRCH` → stale |
| D-6 | `cumulative_cost` reconstruction on resume | Reconstruct from `costs.jsonl` at resume init |
| D-7 | `ParseWarning` struct | See B-11; add to `src/cost.rs` |
| D-8 | CLI flag name: `--timeout-per-run` vs `--timeout-per-run-secs` | Use `--timeout-per-run`; record deviation from `engine.md` in REVIEW.md under Conflicts |
| D-9 | `budget_cap_usd` config precedence | CLI overrides TOML; already in `specs/state/configuration.md` |
| D-10 | `tempfile` crate promotion | Move from `[dev-dependencies]` to `[dependencies]` in Cargo.toml (Step 1) |
| D-11 | `Executor: Send + Sync` bound | Add it; change `MockExecutor` to `Mutex`-based |
| D-12 | Windows platform scope | Out of scope; add `compile_error!` guard |
| D-13 | `partial_output()` return type | `Result<String>`; treat `Err` as empty, log and continue |
| D-14 | `print_budget_cap_reached()` caller | `main.rs` `4 =>` arm, not inside engine |
| D-15 | `--timeout-per-run` on `ResumeArgs` | Add it (missing from B-8) |
| D-16 | F-116 advisory text | Use verbatim `cost-tracking.md` text; adapt prefix to `⚠  `; record in REVIEW.md Conflicts |
| D-17 | `canceled_at` vs `failure_reason` on timeout | On timeout: `canceled_at = null`, `failure_reason = "timeout"` |
| D-18 | Budget cap comparison operator | Use `>=` for all threshold and cap-exceeded checks |
| D-19 | SIGPIPE handling | `SIG_IGN` in `main()` under `#[cfg(unix)]` |
| D-20 | `parse_warnings` display cap | Show ≤10 warnings, then `"... and N more."` |
| D-21 | Lock `create_new` retry bound | Exactly 1 retry after stale removal |

---

### Test Requirements

**Duration parsing (Step 1):**
- `"30s"` → 30, `"5m"` → 300, `"1h"` → 3600
- `"0"` / `"0s"` → `Err` (zero timeout fires immediately)
- `"5min"` / `"1h30m"` / `""` → `Err`
- `"  30s  "` (leading/trailing spaces) → 30 (trim before parse)
- `"30S"` (uppercase suffix) → `Err`
- `"5165088294h"` (overflow) → `Err` (use `checked_mul`)
- `toml::from_str("timeout_per_run_secs = 300")` → `DurationField::Secs(300)` (B-15 verification)

**Context directory lock (Step 6):**
- `kill(pid, 0)` returns `EPERM` → treated as live → `LockError::ActiveProcess`
- Lock file has `pid = 0` → treated as stale
- `context_dir` does not exist → `LockError::ContextDirMissing`
- Second `create_new` fails after stale removal → `LockError::ActiveProcess`
- Lock file is empty → treated as stale, acquire succeeds with warning

**Cancellation and timeout (Step 7):**
- `sigterm_called` on `MockRunHandle` is `true` after cancel flag fires
- `sigkill_called` on `MockRunHandle` is `true` when `ignores_sigterm = true` and 5s expires
- `state.json` written BEFORE SIGKILL is sent (B-24)
- `NNN.log` exists and contains partial output for interrupted run (B-19)

**Budget cap (Step 8):**
- Resume where `cumulative_cost >= 0.8 * cap` → 80% warning does NOT re-fire on first subsequent run
- Exit code 4 → `run.toml` status is `"stopped"`
- `print_budget_cap_reached()` output appears before process exits

**ParseWarning (Step 9):**
- `ParseConfidence::Low` with non-None `raw_match` → warning includes raw match snippet
- `ParseConfidence::None` → "could not be parsed" message (no snippet)
- 11 low-confidence runs → shows 10 warnings + `"... and 1 more"`

---

### Spec Gaps

| ID | Gap | Resolution |
|----|-----|------------|
| SG-1 | `timeout_per_run_secs` absent from `specs/state/configuration.md` precedence table | Add during implementation |
| SG-13 | `--force-lock` absent from `specs/state/configuration.md` | CLI-only; no TOML/env-var form. Record as Decision in REVIEW.md |
| SG-14 | `canceled_at` vs `failure_reason` on timeout not in `cancellation-resume.md` | See D-17; record in REVIEW.md Decisions |
| SG-15 | `pct` field missing from `budget_warning` JSONL event | Add `pct: u8` (80 or 90) to event |
| SG-16 | `scope` field in JSONL events extends beyond spec schema | Intentional forward-compatible addition; record in REVIEW.md Decisions |
| SG-17 | `context_dir` existence not validated in `Workflow::validate` | Add check in `Workflow::validate` |

---

### Implementation Steps

#### Step 1: Foundational — Add `nix` dependency and duration parser

**Files:** `Cargo.toml`, new `src/duration.rs`

- Add `nix = { version = "0.29", features = ["signal", "process"] }` under `[target.'cfg(unix)'.dependencies]`
- Move `tempfile = "3"` from `[dev-dependencies]` to `[dependencies]`
- Implement `pub fn parse_duration_secs(s: &str) -> anyhow::Result<u64>` in `src/duration.rs`
- All edge cases handled (see Test Requirements); tests in same file

---

#### Step 2: Data model changes — `WorkflowConfig`, `PhaseConfig`, `Workflow`

**Files:** `src/workflow.rs`

- Add `DurationField` untagged enum (`u64` or `String`)
- Add to `WorkflowConfig`: `budget_cap_usd: Option<f64>`, `timeout_per_run_secs: Option<DurationField>`
- Add to `PhaseConfig`: `budget_cap_usd: Option<f64>`, `timeout_per_run_secs: Option<DurationField>`
- `Workflow::validate` resolves `DurationField` to `u64` seconds; validates `budget_cap > 0` if set
- Add `context_dir` existence check to `Workflow::validate`

---

#### Step 3: CLI changes — new flags on `RunArgs` and `ResumeArgs`

**Files:** `src/cli.rs`

- Add `RunArgs::budget_cap: Option<f64>` (`--budget-cap <DOLLARS>`)
- Add `RunArgs::timeout_per_run: Option<String>` (`--timeout-per-run <DURATION>`)
- Add `RunArgs::force_lock: bool` (`--force-lock`)
- Add `ResumeArgs::budget_cap: Option<f64>`
- Add `ResumeArgs::timeout_per_run: Option<String>`
- Add `ResumeArgs::force_lock: bool`
- Validate `budget_cap > 0` in `run_inner`/`resume_inner`
- Wire CLI overrides: CLI `budget_cap` and `timeout_per_run` override TOML values

---

#### Step 4: `RunHandle` abstraction — refactor `Executor` trait

**Files:** `src/executor.rs`

- Add `RunHandle` trait: `wait()`, `pid()`, `send_sigterm()`, `send_sigkill()`, `partial_output() -> Result<String>`
- Add `spawn()` method to `Executor` trait
- `ClaudeRunHandle` implements `RunHandle` wrapping `std::process::Child`
  - `CommandExt::process_group(0)` so signals reach the whole process group
  - `child.id()` is both PID and PGID
  - `ESRCH` on signal calls → treat as success (process already gone)
- `MockExecutor` returns `MockRunHandle` (see B-20 for struct definition)
- `MockExecutor` uses `Mutex<Vec<ExecutorOutput>>` (not `RefCell`)
- Existing `run()` becomes a thin wrapper over `spawn()` + `wait()`

---

#### Step 5: `CancelState` and signal handling refactor

**Files:** `src/main.rs`, new `src/cancel.rs`

- Define `CancelState` backed by `AtomicU8`: `0=NotCanceled`, `1=Canceling`, `2=ForceKill`
- Use `SeqCst` throughout; second-signal transition uses `compare_exchange`
- Install `ctrlc` handler once in `main()`, share `Arc<CancelState>`
- On first signal: set `Canceling`; on second signal while `Canceling`: set `ForceKill`
- Under `#[cfg(unix)]`: set `SIG_IGN` for SIGPIPE in `main()` before any subprocess spawning

---

#### Step 6: Context Directory Lock — `src/lock.rs`

**Files:** new `src/lock.rs`, `src/main.rs`

- Define `LockFile { run_id: String, pid: u32 }` with `Serialize`/`Deserialize`
- Define `LockError` (thiserror): `ActiveProcess { run_id, pid, context_dir }`, `ContextDirMissing { path }`
- Define `ContextLock { path: PathBuf }` with `Drop` (removes lock file)
- `ContextLock::acquire(context_dir, run_id, force) -> Result<ContextLock, LockError>`
  - `create_new(true)` for atomic acquisition
  - On `EEXIST`: read existing, parse JSON, `kill(pid, 0)` for liveness
    - `EPERM` → treat as live → `LockError::ActiveProcess`
    - `ESRCH` / parse error → stale; remove, retry once; second `EEXIST` → `LockError::ActiveProcess`
    - `pid = 0` → treat as stale
  - `force = true`: overwrite unconditionally
- Entire module under `#[cfg(unix)]`
- Acquire in `run_inner` and `resume_inner` before `run_workflow`

---

#### Step 7: Timeout and cancellation in engine loop

**Files:** `src/engine.rs`, `src/state.rs`

- Add `failure_reason: Option<String>` to `StateFile` with `#[serde(default)]`
- Engine calls `executor.spawn()` → holds `RunHandle`
- Wait loop: `try_wait()` in 100ms slices, checking:
  1. `ForceKill` → SIGKILL immediately
  2. `Canceling` → SIGTERM, save state, wait up to 5s, SIGKILL if needed, write partial output to run log, exit 130
  3. Timeout expired → SIGTERM, save state, wait up to 5s, SIGKILL, write partial output to run log, `failure_reason = "timeout"`, exit 2
  4. Normal exit → proceed
- Replace `thread::sleep(delay)` with 100ms polling loop checking cancel flag
- Record exit code 2 / timeout conflict with `exit-codes.md` in REVIEW.md under Conflicts

---

#### Step 8: Budget cap — engine loop + display

**Files:** `src/engine.rs`, `src/display.rs`, `src/state.rs`

- Add `phase_costs: HashMap<String, f64>` to engine-local state
- Add `budget_warned_80: bool`, `budget_warned_90: bool` per scope (global + per-phase)
- After each run, update `phase_costs[phase_name]` and `cumulative_cost`
- Check global and per-phase budget caps using `>=`:
  - ≥80%: emit `⚠  Budget: $X.XX spent — 80% of $Y.YY cap.` (once)
  - ≥90%: emit `⚠  Budget: $X.XX spent — 90% of $Y.YY cap. Approaching limit.` (once)
  - ≥100%: `print_budget_cap_reached(scope)`, save state, exit 4
- JSONL: emit `budget_warning { scope, pct: u8 }` and `budget_cap` events
- Add `4 => "stopped"` arm to `final_status` and summary match in `main.rs`
- On resume: initialize `budget_warned_80`/`budget_warned_90` from reconstructed `cumulative_cost`

---

#### Step 9: No budget cap warning + low-confidence warning

**Files:** `src/main.rs`, `src/engine.rs`, `src/cost.rs`, `src/display.rs`

**F-116:** Startup advisory in `run_inner`/`resume_inner` after workflow load:
- If no `budget_cap_usd` in TOML and no `--budget-cap` CLI flag → emit `⚠  Warning: No budget cap configured...` (verbatim from spec, with `⚠  ` prefix; record prefix deviation in REVIEW.md Conflicts)

**F-115:**
- Fix `cost_confidence` serialization: add `#[serde(rename_all = "lowercase")]` to `ParseConfidence`
- In engine loop: if `cost.confidence` is `Low` or `None`, add `ParseWarning` to accumulator
- Add `parse_warnings: Vec<ParseWarning>` to `EngineResult`
- In `main.rs`: after engine completes, display parse warnings via `print_parse_warnings`

---

#### Step 10: Pre-commit checklist

```
Pre-commit checklist
--------------------
[ ] just validate — all gates pass (fmt, lint, tests)
[ ] No unwrap()/expect() added to production code
[ ] Relevant spec in specs/ consulted; implementation is consistent
[ ] REVIEW.md updated with decisions, conflicts, or open questions
[ ] Commit message uses a conventional commit prefix
```

Steps 1–9 may ship as sequential commits (preferred) or a single `feat:` commit. Each step must pass `just validate` independently.

---

### Cross-Feature Dependencies

```
Step 1 (nix + duration parser)
  └─ Step 2 (data model) — depends on DurationField
       └─ Step 3 (CLI) — depends on data model
       └─ Step 8 (budget cap) — depends on data model

Step 4 (RunHandle) — prerequisite for Step 7

Step 5 (CancelState) — prerequisite for Step 7

Step 6 (ContextLock)
  └─ depends on Step 1 (nix for kill(pid, 0))
  └─ depends on Step 3 (--force-lock on RunArgs/ResumeArgs)

Step 9
  └─ depends on Step 2 (budget_cap_usd for F-116)
  └─ depends on Step 4 (ParseConfidence fix for F-115)

Step 7 — must come after Steps 4 and 5
Step 8 — must come after Steps 2 and 7
Steps 4 and 5 can be developed in parallel with Steps 1–3 and 6
```
