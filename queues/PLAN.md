# Implementation Plan — 2026-03-16

## Vote Tally

| Rank | F-NNN | Feature | Votes | Voters |
|------|-------|---------|-------|--------|
| 1 | F-112 | Budget Cap | 13 | review-cli, review-devops, review-data-eng, review-ai-newcomer, review-gen-z, review-security, review-token-opt, review-reliability, review-scripter, review-founder, review-enterprise, review-agent-ux, review-workflow-author |
| 2 | F-020 | Timeout Per Run | 12 | review-cli, review-devops, review-data-eng, review-ai-newcomer, review-gen-z, review-token-opt, review-reliability, review-scripter, review-founder, review-prompt-eng, review-agent-ux, review-workflow-author |
| 3 | F-055 | Context Directory Lock | 12 | review-cli, review-devops, review-data-eng, review-gen-z, review-security, review-reliability, review-scripter, review-oss, review-founder, review-enterprise, review-agent-ux, review-workflow-author |
| 4 | F-095 | --output-format (JSONL) | 9 | (not selected — different spec surface) |
| 5 | F-115 | Low-Confidence Cost Warning | 9 | review-data-eng, review-ai-newcomer, review-gen-z, review-token-opt, review-reliability, review-founder, review-prompt-eng, review-enterprise, review-workflow-author |
| 6 | F-037 | Error Classification | 7 | (excluded — opens deep chain) |
| 6 | F-070 | rings list | 7 | (excluded — different spec surface) |
| 6 | F-062 | Project Config File | 7 | (excluded — different spec surface) |
| 6 | F-081 | --dry-run | 7 | (excluded — different spec surface) |
| 6 | F-116 | No Budget Cap Warning | 7 | review-devops, review-ai-newcomer, review-gen-z, review-token-opt, review-founder, review-enterprise, review-agent-ux |

## Selected Features

**Batch: Process Safety + Cost Safety**

| F-NNN | Feature | Summary |
|-------|---------|---------|
| F-020 | Timeout Per Run | `timeout_per_run_secs` TOML/CLI; SIGTERM→5s→SIGKILL; exit 2 |
| F-052 | SIGTERM Handling | Install SIGTERM handler; handle identically to SIGINT |
| F-054 | Subprocess Graceful Shutdown | SIGTERM to executor, 5s wait, then SIGKILL; capture partial output |
| F-055 | Context Directory Lock | `.rings.lock` with PID; stale detection; `--force-lock` |
| F-112 | Budget Cap | `budget_cap_usd` TOML/CLI; exit 4; 80%/90% warnings; per-phase caps |
| F-115 | Low-Confidence Cost Warning | Accumulate + display warnings when confidence is Low or None |
| F-116 | No Budget Cap Warning | Startup advisory when no budget cap configured |

**Ranking overrides:**
- F-052 and F-054 ranked 13th/13th (4 votes each) but are structural complements to F-055 (12 votes). Implementing F-055 without F-052+F-054 leaves a broken shutdown contract: the lock would not be released on SIGTERM, causing stale locks on every systemd/container stop.
- F-037 excluded despite 7 votes: it anchors a deep error-handling chain (F-038→F-039→F-044 quota backoff) better handled as its own future batch.

---

## Implementation Review Findings

### Blockers — Must resolve before coding begins

**B-1: Executor trait has no cancellation or kill surface** *(impl-rust, impl-architecture, impl-testing, impl-process-mgmt, impl-performance, impl-memory)*

`ClaudeExecutor::run()` calls `child.wait_with_output()` and blocks indefinitely. There is no way to send SIGTERM to the subprocess from outside the call. The cancellation flag (`AtomicBool`) is only checked *after* `executor.run()` returns — meaning SIGINT/SIGTERM during a live run does nothing until the subprocess finishes naturally.

This blocks F-020 (timeout), F-052 (SIGTERM), and F-054 (graceful shutdown).

**Required architectural change — choose one before starting:**

*Option A (recommended):* Split spawn from wait. Introduce a `RunHandle` trait:
```rust
pub trait Executor: Send + Sync {
    fn spawn(&self, invocation: &Invocation, verbose: bool) -> Result<Box<dyn RunHandle>>;
}

pub trait RunHandle: Send {
    fn wait(&mut self) -> Result<ExecutorOutput>;
    fn pid(&self) -> u32;
    fn send_sigterm(&self) -> Result<()>;
    fn send_sigkill(&self) -> Result<()>;
    fn partial_output(&self) -> String; // captured so far
}
```
Engine calls `spawn()`, gets a handle, waits in a polling loop checking the cancel flag and timeout, sends signals when needed. `MockExecutor` implements `RunHandle` trivially.

*Option B:* Pass a `CancellationToken` into `run()`. Smaller diff but mixes concerns. Less testable.

Decision must be recorded in REVIEW.md before any code is written.

---

**B-2: Signal handler only sets a flag; subprocess never killed during active run** *(impl-rust, impl-architecture, impl-error-handling, impl-process-mgmt)*

Current `ctrlc` handler: `canceled.store(true, SeqCst)`. Engine checks the flag only between runs. A hung `claude` subprocess holds the process for its full duration despite user Ctrl+C.

Fix: with the `RunHandle` abstraction (B-1), the engine's wait loop checks the cancel flag in each iteration. When set, it calls `handle.send_sigterm()`, waits up to 5s polling `try_wait()`, then `handle.send_sigkill()`.

Double Ctrl+C (F-053): use an `AtomicU8` cancel counter. First signal → SIGTERM path. Second signal → SIGKILL immediately.

---

**B-3: Inter-run delay (`thread::sleep`) is not interruptible** *(impl-process-mgmt, impl-cross-platform, impl-performance)*

`engine.rs` uses `std::thread::sleep(Duration::from_secs(delay))`. A Ctrl+C during this sleep is not noticed until the sleep completes (up to 600s per spec sanity check). Spec (cancellation-resume.md) explicitly requires: "Ctrl+C During a Delay: rings triggers the normal cancellation flow immediately."

Fix: replace `thread::sleep` with a polling loop (~100ms slices) checking the cancel flag.

---

**B-4: Duration string parsing does not exist** *(impl-rust, impl-deps, impl-regex, impl-serialization)*

`timeout_per_run_secs` accepts integer seconds OR duration strings (`"5m"`, `"30s"`). No parser exists. Required decisions on edge cases before implementation:

| Input | Required behavior |
|-------|------------------|
| `"30s"` | 30 |
| `"5m"` | 300 |
| `"1h"` | 3600 |
| `"0"` or `"0s"` | Validation error (zero timeout fires immediately) |
| `"5min"` | Validation error (only single-char suffixes `s`, `m`, `h`) |
| `"1h30m"` | Validation error (no compound forms) |
| `""` | Validation error |
| overflow | Validation error |
| negative integer string | Validation error (`u64` parse rejects naturally) |

Implement `fn parse_duration_secs(s: &str) -> Result<u64>` in a utility module. Never panics. All errors return `Err` with a descriptive message. Test all cases above.

TOML field needs a custom serde deserializer (untagged enum) to accept both integer and string:
```rust
#[derive(Deserialize)]
#[serde(untagged)]
enum DurationField { Secs(u64), Str(String) }
```

---

**B-5: `nix` crate not in dependencies; required for signal sending and PID liveness** *(impl-deps, impl-cross-platform)*

`ctrlc` (with `termination` feature, already present) handles *receiving* SIGINT/SIGTERM by rings. But *sending* SIGTERM/SIGKILL to child processes and checking PID liveness require `nix` (or `libc` directly).

Add to `Cargo.toml`:
```toml
[target.'cfg(unix)'.dependencies]
nix = { version = "0.29", features = ["signal", "process"] }
```

Use `nix::sys::signal::kill(Pid::from_raw(pid), Signal::SIGTERM)` and `Signal::SIGKILL`. Use `nix::sys::signal::kill(Pid::from_raw(pid), None)` (signal 0) for PID liveness check in F-055 — this is POSIX-portable across Linux and macOS, unlike `/proc/<pid>` which is Linux-only.

---

**B-6: Context Directory Lock entirely absent** *(impl-rust, impl-architecture, impl-filesystem, impl-error-handling, impl-cross-platform)*

No `.rings.lock` file is created, checked, or removed anywhere. `--force-lock` flag does not exist in `cli.rs`. Both `RunArgs` and `ResumeArgs` need `--force-lock`. Lock must be acquired in `run_inner` AND `resume_inner` (resume restarts execution against the same `context_dir`).

Lock file must be written atomically and read back for stale detection. Malformed/truncated lock files must be treated as stale (warn + proceed), not as hard errors.

---

**B-7: Lock file format unspecified** *(impl-serialization, impl-regex)*

Chosen format: **JSON** (consistent with `state.json`, uses existing `serde_json` dep):
```json
{"run_id":"run_20240315_143022_a1b2c3","pid":12345}
```

Write atomically using `OpenOptions::new().write(true).create_new(true)` (`O_CREAT|O_EXCL`) — this is the atomic lock-acquisition primitive; temp+rename is for updates, not initial creation. On any parse error when reading back, treat as stale.

---

**B-8: `budget_cap_usd` and `timeout_per_run_secs` missing from entire data model** *(impl-cli-framework, impl-serialization, impl-error-handling)*

Neither field exists in `WorkflowConfig`, `PhaseConfig`, CLI args, or `EngineConfig`. All must be added:
- `WorkflowConfig::budget_cap_usd: Option<f64>`
- `WorkflowConfig::timeout_per_run_secs: Option<DurationField>`
- `PhaseConfig::budget_cap_usd: Option<f64>`
- `PhaseConfig::timeout_per_run_secs: Option<DurationField>`
- `RunArgs::budget_cap: Option<f64>` (CLI override: `--budget-cap`)
- `RunArgs::timeout_per_run: Option<String>` (CLI override: `--timeout-per-run`)
- `ResumeArgs::budget_cap: Option<f64>`
- `ResumeArgs::force_lock: bool`
- `RunArgs::force_lock: bool`

---

**B-9: `print_budget_cap_reached()` output is unspecified** *(impl-agent-ux, impl-docs)*

The spec defines the JSONL `budget_cap` event format but the human-mode message is entirely absent from all spec files. Before implementation, define the message. Proposed canonical form to record in REVIEW.md as a decision:

```
✗  Budget cap reached: $5.03 spent (cap: $5.00).
   Workflow stopped after cycle 3, builder (run 42).

   To resume with a higher cap:
     rings resume <run-id> --budget-cap 10.00
```

Per-phase variant:
```
✗  Phase budget cap reached for "builder": $2.05 spent (cap: $2.00).
```

---

**B-10: Timeout user-facing message is unspecified** *(impl-agent-ux, impl-docs)*

No display string exists for timeout-reached. Exit code 2 is shared with config errors, so the message is the only distinguishing signal. Proposed canonical form to record in REVIEW.md:

```
✗  Run 14 timed out after 300s (cycle 3, builder, iteration 2/3).
   State saved. To resume: rings resume <run-id>
```

---

**B-11: `failure_reason` missing from `StateFile`** *(impl-rust, impl-architecture, impl-process-mgmt)*

Spec requires `failure_reason = "timeout"` recorded in run state when timeout fires. `StateFile` has no such field. Add `failure_reason: Option<String>` (with `#[serde(default)]` for backward compat with existing state files).

---

**B-12: Verbose mode `wait_with_output()` races with reader threads under SIGKILL** *(impl-performance, impl-memory)*

In verbose mode: stdout/stderr are `take()`n into reader threads, then `child.wait_with_output()` is called on the same `Child`. Under normal conditions this is accidentally safe, but when SIGKILL is sent mid-run (timeout or double Ctrl+C), the reader threads may block on a broken pipe. Fix: use `child.wait()` (not `wait_with_output()`) in verbose mode; join reader threads after `wait()` returns. Also: reader thread panics are silently swallowed — propagate errors from `.join()`.

---

### Open Decisions

**D-1: Executor trait architecture (B-1)**
Recommendation: Option A (`RunHandle` trait). Cleaner separation, more testable, better for future async migration. Record in REVIEW.md.

**D-2: Process group vs PID for signal delivery**
Recommendation: Use process group (`nix::unistd::getpgid` + `nix::sys::signal::killpg`), not just child PID. `claude` (Node.js) spawns child processes; SIGTERM to PID alone leaves grandchildren as orphans. Spawn `claude` in a new process group via `CommandExt::process_group(0)`. Record in REVIEW.md.

**D-3: `ctrlc::set_handler` called twice (run + resume)**
Recommendation: Install handler once in `main()`, pass `Arc<CancelState>` to both `cmd_run` and `cmd_resume`. `CancelState` is a three-value type (NotCanceled/Canceling/ForceKill) backed by `AtomicU8`. Record in REVIEW.md.

**D-4: F-115 trigger: `Partial` or `Low`/`None` only**
Recommendation: `Low` and `None` only, per `output-parsing.md`. `Partial` confidence (cost found, token counts absent) is explicitly excluded from warnings in the existing spec. Record in REVIEW.md under Conflicts (feature description vs spec).

**D-5: `BudgetScope` JSONL serialization**
Default serde enum serialization produces `{"Phase":"builder"}`, not the spec-required `"phase:builder"`. Implement `Display` + custom `Serialize`/`Deserialize` for `BudgetScope`. Record in REVIEW.md.

**D-6: Per-phase cost tracking storage**
Engine currently tracks only a single `cumulative_cost: f64`. Per-phase budget caps require `phase_costs: HashMap<String, f64>`. Live in engine-local state (not `EngineResult`); reset on resume by reconstructing from `costs.jsonl`.

**D-7: Parse warning accumulation**
Add `parse_warnings: Vec<ParseWarning>` to `EngineResult`. Engine accumulates; `main.rs` displays. This keeps display policy out of the engine and makes warnings testable without stderr capture.

**D-8: `--timeout-per-run` flag naming**
The spec names the field `timeout_per_run_secs` but it accepts duration strings, making the `_secs` suffix misleading. CLI flag recommendation: `--timeout-per-run <DURATION>` (no `_secs`). TOML field name stays `timeout_per_run_secs` (spec-defined). Record in REVIEW.md under Decisions.

**D-9: NFS/read-only filesystem behavior for lock**
`O_CREAT|O_EXCL` is not reliable on NFSv3. Do not add a workaround; document the limitation in REVIEW.md under Open Questions. `--force-lock` is the escape hatch.

**D-10: `state.json.tmp` temp file naming**
Currently uses a fixed `.tmp` extension — collision risk under concurrent access. Recommendation: use `tempfile::NamedTempFile` for all atomic writes (state.json, run.toml). Addresses Finding 6 from impl-filesystem. Add `tempfile` to deps if not already present (it is in dev-deps; promote to regular dep).

---

### Test Requirements

**Duration parsing (unit, `src/` or `tests/duration.rs`):**
- `"30s"` → 30
- `"5m"` → 300
- `"1h"` → 3600
- `"300"` (bare integer string) → 300
- `"0"`, `"0s"` → Err
- `"abc"` → Err
- `""` → Err
- `"5min"` → Err
- `"1h30m"` → Err
- Very large value causing overflow → Err

**Context directory lock (unit, `src/lock.rs` or `tests/lock.rs`):**
- Lock file created at `<context_dir>/.rings.lock` with correct PID
- Lock file removed on `Drop`
- `acquire()` returns error when lock held by live process (use current process PID as "live" signal)
- `acquire()` removes stale lock and warns when PID not running (use `kill(pid, 0)` → ESRCH)
- `--force-lock` bypasses all checks
- Two simultaneous `acquire()` calls: exactly one succeeds (race safety via `O_EXCL`)

**Budget cap (unit via engine integration):**
- `cumulative_cost < budget_cap_usd` → workflow continues
- `cumulative_cost == budget_cap_usd` → exit 4
- `cumulative_cost > budget_cap_usd` → exit 4
- State is saved before exit 4 (state.json exists and is valid)
- 80% threshold emits one `budget_warning` event (scope="global")
- 90% threshold emits one `budget_warning` event (scope="global")
- 80% threshold fires exactly once (not on every subsequent run)
- Per-phase cap fires independently of global cap
- Per-phase `budget_cap` JSONL event has `scope="phase:builder"`

**Low-confidence warning (unit via engine integration):**
- `ParseConfidence::Low` → warning accumulated in `EngineResult.parse_warnings`
- `ParseConfidence::None` → warning accumulated
- `ParseConfidence::Full` → no warning
- `ParseConfidence::Partial` → no warning (per D-4)
- Multiple low-confidence runs → multiple warnings accumulated

**No budget cap warning (unit):**
- Workflow with no `budget_cap_usd` → advisory warning emitted
- Workflow with `budget_cap_usd = 5.0` → no advisory
- CLI `--budget-cap` set → no advisory

**Timeout (unit, requires `RunHandle` abstraction):**
- Mock handle exits before timeout → no signals sent
- Mock handle ignores SIGTERM → SIGKILL sent after 5s
- Timeout records `failure_reason = "timeout"` in state
- Timeout exits with code 2
- State is saved before exit 2
- Duration string passed through `Invocation`, not in `ClaudeExecutor::build_args()`

**Signal handling (unit, requires `RunHandle` abstraction):**
- Cancel flag set → SIGTERM sent to subprocess handle
- Cancel flag set while in delay → delay interrupted, cancellation proceeds
- `canceled_at` is non-null in state after cancellation
- Exit code is 130 on cancellation
- Second cancel signal → SIGKILL sent immediately (double Ctrl+C)

---

### Spec Gaps

**SG-1:** `timeout_per_run_secs` absent from precedence table in `specs/state/configuration.md`. Record in REVIEW.md under Open Questions; do not edit spec.

**SG-2:** `print_budget_cap_reached()` human-mode message not defined anywhere in specs. Decision recorded in REVIEW.md under Decisions (see B-9).

**SG-3:** Timeout-reached display message not defined. Decision recorded in REVIEW.md (see B-10).

**SG-4:** `--timeout-per-run` (or `--timeout-per-run-secs`) absent from `specs/cli/commands-and-flags.md`.

**SG-5:** `--force-lock` absent from `specs/cli/commands-and-flags.md`.

**SG-6:** Duration string acceptance on CLI flag is unspecified (spec only defines it for TOML field). Decision: CLI accepts same formats as TOML (recorded in REVIEW.md).

**SG-7:** F-115 feature description says warn on `Partial`, but `output-parsing.md` excludes `Partial`. Record in REVIEW.md under Conflicts.

**SG-8:** Lock error message uses literal `context_dir` as placeholder — should be the resolved path. Record in REVIEW.md.

**SG-9:** F-116 output destination (stderr vs JSONL advisory_warning event) not stated in `cost-tracking.md`. By pattern from `engine.md` advisory checks: stderr in human mode, `advisory_warning` JSONL event in JSONL mode. Record in REVIEW.md under Decisions.

**SG-10:** Budget warning (80%/90%) human-mode format unspecified. Implement as:
```
⚠  Budget: $4.00 spent — 80% of $5.00 cap.
```

**SG-11:** Whether 80%/90% warning fires once or on every subsequent run is unspecified. Decision: fires once per threshold (per D-3 in the engine loop). Record in REVIEW.md.

**SG-12:** GitHub issue URL in `output-parsing.md` warning deduplication section is a placeholder (`https://github.com/owner/rings/issues`). Record in REVIEW.md under Open Questions.

---

### Discarded Concerns

- **F-115 inline vs accumulated:** `output-parsing.md` is authoritative — warnings accumulate and display in a summary block at the end. The word "immediately" in F-115's feature description is loose wording, not a contradiction.
- **`ctrlc` `termination` feature coverage of SIGTERM:** Already enabled in `Cargo.toml`. Not a missing dependency.
- **`f64` for cost accumulation:** Acceptable precision for v1 dollar amounts. Document as known limitation in REVIEW.md. No `rust_decimal` migration in this batch.
- **`MockExecutor` is `!Send`:** Safe as long as the timeout watchdog shares only the PID (not the executor). Only becomes a concern if the executor itself must be `Send`. No change needed now.
- **`lazy_static` vs `std::sync::LazyLock`:** Pre-existing nit unrelated to this batch.

---

## Implementation Steps

### Step 1: Foundational — Add `nix` dependency and duration parser

**Files:** `Cargo.toml`, new `src/duration.rs`

- Add `nix = { version = "0.29", features = ["signal", "process"] }` under `[target.'cfg(unix)'.dependencies]`
- Implement `pub fn parse_duration_secs(s: &str) -> anyhow::Result<u64>` in `src/duration.rs`
- All edge cases handled (see Test Requirements)
- Tests in the same file

---

### Step 2: Data model changes — `WorkflowConfig`, `PhaseConfig`, `Workflow`

**Files:** `src/workflow.rs`

- Add `DurationField` untagged enum (u64 or String)
- Add to `WorkflowConfig`:
  - `budget_cap_usd: Option<f64>`
  - `timeout_per_run_secs: Option<DurationField>`
- Add to `PhaseConfig`:
  - `budget_cap_usd: Option<f64>`
  - `timeout_per_run_secs: Option<DurationField>`
- `Workflow::validate` resolves `DurationField` to `u64` seconds; validates budget cap > 0 if set
- Resolved values available on `Workflow` struct (or directly on `PhaseConfig` after validation)

---

### Step 3: CLI changes — new flags on `RunArgs` and `ResumeArgs`

**Files:** `src/cli.rs`

- Add `RunArgs::budget_cap: Option<f64>` (`--budget-cap <DOLLARS>`)
- Add `RunArgs::timeout_per_run: Option<String>` (`--timeout-per-run <DURATION>`)
- Add `RunArgs::force_lock: bool` (`--force-lock`)
- Add `ResumeArgs::budget_cap: Option<f64>`
- Add `ResumeArgs::force_lock: bool`
- Validate `budget_cap > 0` in `run_inner`/`resume_inner` (exit 2 on invalid)
- Wire CLI overrides: CLI `budget_cap` and `timeout_per_run` override TOML values in `run_inner`/`resume_inner`

---

### Step 4: `RunHandle` abstraction — refactor `Executor` trait

**Files:** `src/executor.rs`

- Add `RunHandle` trait with: `wait()`, `pid()`, `send_sigterm()`, `send_sigkill()`, `partial_output()`
- Add `spawn()` method to `Executor` trait (or keep `run()` and add a parallel `spawn()`)
- `ClaudeRunHandle` implements `RunHandle` wrapping `std::process::Child`
  - Spawn with `CommandExt::process_group(0)` so signals reach the whole process group
  - Store accumulated output in `Arc<Mutex<String>>` for `partial_output()` access
- Fix verbose mode: use `child.wait()` (not `wait_with_output()`); join reader threads after wait; propagate join errors
- `MockExecutor` implements `spawn()` → returns a `MockRunHandle` with scriptable signal/exit behavior
- Existing `run()` method becomes a thin wrapper over `spawn()` + `wait()` for backward compat

---

### Step 5: `CancelState` and signal handling refactor

**Files:** `src/main.rs`, new `src/cancel.rs`

- Define `CancelState` backed by `AtomicU8`: `0=NotCanceled`, `1=Canceling`, `2=ForceKill`
- Install `ctrlc` handler once in `main()`, share `Arc<CancelState>` with `cmd_run` and `cmd_resume`
- On first signal: set `Canceling`
- On second signal while `Canceling`: set `ForceKill`
- Engine passes `Arc<CancelState>` to the run loop

---

### Step 6: Context Directory Lock — `src/lock.rs`

**Files:** new `src/lock.rs`, `src/main.rs`

- Define `LockFile { run_id: String, pid: u32 }` with `Serialize`/`Deserialize`
- Define `ContextLock { path: PathBuf }` with `Drop` (removes lock file)
- `ContextLock::acquire(context_dir, run_id, force) -> Result<ContextLock, LockError>`
  - `OpenOptions::new().write(true).create_new(true)` for atomic creation
  - On `EEXIST`: read existing lock, parse JSON, call `kill(pid, 0)` for liveness
    - Live: return `LockError::ActiveProcess { run_id, pid }`
    - Not running / parse error: remove stale lock, emit warning, retry acquire
  - `force = true`: skip all checks, overwrite any existing lock
- Acquire in `run_inner` before `run_workflow`; acquire in `resume_inner` before `run_workflow`
- Lock guard held for the entire run lifetime; dropped on any exit path

---

### Step 7: Timeout and cancellation in engine loop

**Files:** `src/engine.rs`, `src/state.rs`

- Add `failure_reason: Option<String>` to `StateFile` with `#[serde(default)]`
- Engine calls `executor.spawn()` → holds `RunHandle`
- Wait loop: poll `handle.wait()` with `try_wait()` in 100ms slices, checking:
  1. Cancel flag (`Canceling`) → send SIGTERM to process group, start 5s SIGKILL timer
  2. Cancel flag (`ForceKill`) → send SIGKILL immediately
  3. Timeout expired → send SIGTERM, start 5s timer, then SIGKILL; record `failure_reason="timeout"`, exit 2
  4. Normal exit → proceed
- Replace `thread::sleep(delay)` with 100ms-slice polling loop checking cancel flag
- On cancellation: capture `handle.partial_output()`, scan for resume commands, save state, exit 130
- On timeout: save state with `failure_reason="timeout"`, exit 2

---

### Step 8: Budget cap — engine loop + display

**Files:** `src/engine.rs`, `src/display.rs`, `src/state.rs`

- Add `phase_costs: HashMap<String, f64>` to engine-local state
- Add `budget_warned_80: bool`, `budget_warned_90: bool` per scope (global + per-phase)
- After each run, update `phase_costs[phase_name]` and `cumulative_cost`
- Check global budget cap:
  - ≥80%: emit `⚠  Budget: $X.XX spent — 80% of $Y.YY cap.` (once)
  - ≥90%: emit `⚠  Budget: $X.XX spent — 90% of $Y.YY cap. Approaching limit.` (once)
  - ≥100%: `print_budget_cap_reached(scope="global")`, save state, exit 4
- Check per-phase budget caps independently (same threshold logic)
- `print_budget_cap_reached()` format per B-9
- JSONL mode: emit `budget_warning` event (with `scope` field) and `budget_cap` event

---

### Step 9: No budget cap warning + low-confidence warning

**Files:** `src/main.rs`, `src/engine.rs`, `src/cost.rs`

**F-116:** In `run_inner`/`resume_inner` startup advisory checks (after workflow load, before engine starts):
- If `workflow.budget_cap_usd.is_none()` and CLI `--budget-cap` not set → emit advisory warning
- Follows existing pattern of `--no-completion-check` advisory; no suppression flag defined by spec

**F-115:**
- Fix `cost_confidence` serialization in `cost.rs`: add `#[serde(rename_all = "lowercase")]` to `ParseConfidence` and use it in `CostEntry` instead of `format!("{:?}", …).to_lowercase()`
- In engine loop after each run: if `cost.confidence` is `Low` or `None` (not `Partial`), add `ParseWarning` to accumulator
- Add `parse_warnings: Vec<ParseWarning>` to `EngineResult`
- In `main.rs`, after engine completes (on any exit), display accumulated parse warnings

---

### Step 10: Pre-commit checklist

Before committing, verify per CLAUDE.md:

```
Pre-commit checklist
--------------------
[ ] just validate — all gates pass (fmt, lint, tests)
[ ] No unwrap()/expect() added to production code
[ ] Relevant spec in specs/ consulted; implementation is consistent
[ ] REVIEW.md updated with decisions, conflicts, or open questions from this task
[ ] Commit message uses a conventional commit prefix (feat/fix/test/refactor/chore/docs)
```

All steps (1-9) can ship as a single `feat:` commit or as sequential commits per step. Sequential commits are strongly preferred — each step should pass `just validate` independently.

---

## Cross-Feature Dependencies in This Batch

```
Step 1 (nix + duration parser)
  └─ Step 2 (data model) depends on DurationField from Step 1
       └─ Step 3 (CLI) depends on data model
       └─ Step 8 (budget cap) depends on data model

Step 4 (RunHandle) is prerequisite for:
  └─ Step 7 (timeout + cancellation in engine)

Step 5 (CancelState) is prerequisite for:
  └─ Step 7 (engine reads CancelState)

Step 6 (ContextLock) depends on:
  └─ Step 1 (nix for kill(pid, 0))
  └─ Step 3 (--force-lock on RunArgs/ResumeArgs)

Step 9 depends on:
  └─ Step 2 (budget_cap_usd field for F-116 check)
  └─ Step 4 (ParseConfidence fix for F-115)

Steps 4 and 5 can be developed in parallel with Steps 1-3 and 6.
Step 7 must come after both Step 4 and Step 5.
Step 8 must come after Step 2 and Step 7.
Step 9 can come after Step 2 and Step 4.
```

Do not mark any feature as `PLANNED` in the inventory until this plan is reviewed and approved.

---

## Supplementary Review Findings (Wave 2 — 2026-03-16)

A second pass with all 15 impl-review agents identified the following issues not covered in Wave 1. Items are grouped by type; items marked **CORRECTION** amend earlier content.

---

### Corrections to Wave 1 Content

**CORRECTION — Discarded Concern "MockExecutor is `!Send`" is wrong**

Wave 1 listed this as a discarded concern. It is NOT discarded — it is a blocker. See B-14 below.

**CORRECTION — D-2 PGID capture**: `CommandExt::process_group(0)` makes the child's PGID equal to its own PID. No `getpgid` call is needed. Capture `child.id()` immediately after `spawn()` as both PID and PGID. The mention of `nix::unistd::getpgid` in D-2 is removed.

**CORRECTION — SG-1 partially stale**: `budget_cap_usd` is already present in the `specs/state/configuration.md` precedence table. SG-1 applies to `timeout_per_run_secs` only.

**CORRECTION — SG-9 is not a gap**: The F-116 advisory text IS spec-defined in `cost-tracking.md`: `"Warning: No budget cap configured. Use --budget-cap or budget_cap_usd to prevent unbounded spend."` Use this verbatim text; adapt only the prefix from `Warning:` to `⚠  ` for visual consistency with other advisory messages. Record the prefix change in REVIEW.md under Conflicts.

**CORRECTION — D-8 should be tagged Conflict, not just Decision**: `specs/execution/engine.md` uses the flag name `--timeout-per-run-secs`. The plan's choice of `--timeout-per-run` is a deviation from a named spec value. Record in REVIEW.md under Conflicts (not just Decisions).

**CORRECTION — `tempfile` promotion missing from Step 1**: D-10 says to promote `tempfile` from `[dev-dependencies]` to `[dependencies]`. This change must be added explicitly to Step 1's Cargo.toml changes: move `tempfile = "3"` to `[dependencies]`. An implementer following only the steps section will miss this and get a compile error when `NamedTempFile` is used in production code.

---

### New Blockers

**B-13: `std::process::exit()` bypasses `ContextLock::Drop`** *(impl-memory, impl-rust)*

`std::process::exit()` does NOT run Rust destructors. The exit paths — exit 2 (timeout), exit 4 (budget cap), exit 130 (cancellation) — all currently call or will call `process::exit()` at some point. If `ContextLock` is held as a local variable in `run_inner`/`resume_inner` and the engine returns an exit code to `main()` which calls `process::exit(code)`, the destructors DO run (Rust's `main()` return invokes cleanup). But if any exit path calls `process::exit()` directly (bypassing `main()`'s return), `ContextLock::Drop` is skipped, leaving a live-PID stale lock.

Fix: ensure all exit paths flow through the `main()` return path, not direct `process::exit()` calls. Concretely: `run_inner` and `resume_inner` must return `Result<i32>` (as they do now), and `main()` calls `std::process::exit(exit_code)` only after the call site's stack frame has fully unwound (which triggers all destructors on local variables in `main()`). This is the current pattern and it is safe — verify it stays this way in Steps 7–9 and no new direct `process::exit()` calls are introduced inside the engine or lock module.

---

**B-14: `MockExecutor !Send` conflicts with `Executor: Send + Sync`** *(impl-rust, impl-architecture, impl-memory)*

`MockExecutor` uses `RefCell<Vec<ExecutorOutput>>`, which is `!Send`. Step 4 proposes adding `pub trait Executor: Send + Sync`. This causes a compile error for all `#[cfg(feature = "testing")]` tests. The Discarded Concern in Wave 1 was wrong: this IS a blocker.

Fix: change `MockExecutor` to use `Mutex<Vec<ExecutorOutput>>` instead of `RefCell`. Also: `MockRunHandle` must NOT use `RefCell` for its scriptable state — it must use `Arc<Mutex<...>>` or `Arc<AtomicBool>` so it satisfies `RunHandle: Send`. Record this decision in REVIEW.md.

---

**B-15: `DurationField` untagged enum deserialization with toml 0.8** *(impl-rust, impl-serialization)*

TOML 0.8 represents integers as `i64` internally. When deserializing `timeout_per_run_secs = 300` via `#[serde(untagged)] enum DurationField { Secs(u64), Str(String) }`, the deserializer attempts `Secs(u64)` first. Whether toml 0.8's serde integration coerces `i64` → `u64` in an untagged context needs to be verified with an explicit test before committing to this design. If coercion fails, use `Secs(i64)` and validate `>= 0` after deserialization. Add a test in Step 1: `toml::from_str("timeout_per_run_secs = 300")` must produce `DurationField::Secs(300)`.

---

**B-16: `CancelState` AtomicU8 memory ordering unspecified** *(impl-rust, impl-memory)*

The plan says "use `AtomicU8`" but never specifies memory orderings. Concretely:

- Second-signal transition (`Canceling` → `ForceKill`) must use `compare_exchange(CANCELING, FORCE_KILL, AcqRel, Acquire)` — not a plain `store`. A plain store races with a near-simultaneous first signal.
- Loads in the engine poll loop: use `Acquire`.
- Stores in the signal handler: use `Release` (or `SeqCst` throughout for simplicity).

Recommendation: use `SeqCst` throughout and document "do not optimize to Relaxed — correctness on ARM requires at minimum Acquire/Release." Record in REVIEW.md under Decisions.

---

**B-17: `#[cfg(unix)]` guards required throughout; platform scope unresolved** *(impl-deps, impl-cross-platform)*

`nix` is under `[target.'cfg(unix)'.dependencies]`, but the following code paths will fail to compile on non-Unix targets without explicit `#[cfg(unix)]` guards:

- `ClaudeRunHandle`: `CommandExt::process_group(0)` and all `nix::sys::signal::kill`/`killpg` calls
- `src/lock.rs`: the entire module uses `nix` for PID liveness checks
- All tests in `lock.rs` that call `nix` APIs

Platform decision required before Step 4: **Windows is out of scope for this batch.** Add a top-level `#[cfg(not(unix))] compile_error!("rings requires a Unix platform")` in `src/main.rs`. Gate `ClaudeRunHandle`'s signal/process-group code under `#[cfg(unix)]` with a non-Unix stub returning `Err(anyhow!("Unix-only"))`. Gate `lock.rs` entirely under `#[cfg(unix)]`. Record this scope decision in REVIEW.md under Decisions.

---

**B-18: `RunHandle::wait()` must use `try_wait()` polling, not blocking `child.wait()`** *(impl-process-mgmt, impl-memory)*

The plan (B-12 fix) says "use `child.wait()` instead of `wait_with_output()` in verbose mode." This is ambiguous. `ClaudeRunHandle::wait()` must be implemented as a `try_wait()` polling loop (100ms slices), not a call to blocking `child.wait()`. Rationale: if the subprocess writes more than the OS pipe buffer (64 KB on Linux) to stdout/stderr while the main thread blocks in `wait()`, the subprocess deadlocks waiting for the reader to drain the buffer — which the reader thread does — but a blocking `wait()` can conflict with that on some implementations. Using `try_wait()` as the poll mechanism avoids this entirely. Reader threads (in verbose mode) are joined inside `ClaudeRunHandle::wait()` after `try_wait()` returns `Some(exit_status)`.

---

**B-19: Partial output not written to run log on cancellation/timeout** *(impl-filesystem)*

`write_run_log` is currently called only after a run completes. With the `RunHandle` abstraction, cancellation and timeout interrupt the run mid-execution. Step 7 must explicitly add: "after capturing `handle.partial_output()`, write it to the run log (`write_run_log(&runs_dir, run_spec.global_run_number, &partial)`) before saving state and exiting." Without this, `NNN.log` for the interrupted run is silently absent, creating a gap in the numbered log sequence that confuses post-mortem debugging.

---

**B-20: `MockRunHandle` API is completely unspecified** *(impl-testing, impl-architecture)*

The plan mentions `MockRunHandle` but never defines it. At minimum it needs:

```rust
pub struct MockRunHandle {
    pub output: ExecutorOutput,       // what wait() returns on clean exit
    pub wait_delay_ms: u64,           // simulate subprocess duration
    pub ignores_sigterm: bool,        // true = wait() hangs until sigkill
    pub sigterm_called: Arc<AtomicBool>,
    pub sigkill_called: Arc<AtomicBool>,
}
```

Without `sigterm_called` and `sigkill_called` as observable state, the signal-handling tests have no assertion mechanism — the test could pass vacuously. `MockRunHandle` must NOT use `RefCell` (see B-14). Add this struct definition to Step 4.

---

**B-21: Exit code 4 has no display handler in `main.rs`; `run.toml` gets wrong status** *(impl-error-handling, impl-agent-ux, impl-cli-framework)*

`main.rs` handles exit codes 0, 1, 3, 130 — exit code 4 falls through to `_ => {}` with no display and `final_status` maps to `"failed"`. Both `run_inner` and `resume_inner` need:

1. A `4 => "stopped"` (or `"budget_cap_reached"`) arm in the `final_status` match.
2. A `4 =>` arm in the summary `match` block.

Decision required (D-14): does the engine call `print_budget_cap_reached()` before returning, or does `main.rs` call it in the `4 =>` arm? Recommendation: follow the existing pattern — engine returns `EngineResult { exit_code: 4 }`, `main.rs` calls the display function. This keeps all display policy in `main.rs`. Add this to Step 8.

---

**B-22: `LockError` propagation type unspecified** *(impl-error-handling)*

Step 6 returns `LockError` but never defines it as a type. It must be a `thiserror`-derived enum with a `Display` impl that embeds `run_id` and `pid`. The spec-required error message is:

```
Error: Another rings run (run_20240315_143022_a1b2c3, PID=12345) is already using context_dir.
Wait for it to finish or use --force-lock to override.
```

Using a plain `anyhow!()` string loses the structured data. `LockError` variants needed at minimum: `ActiveProcess { run_id: String, pid: u32, context_dir: PathBuf }`, `ContextDirMissing { path: PathBuf }`. Convert to `anyhow::Error` at the call site in `run_inner`/`resume_inner`.

---

**B-23: `ParseWarning` struct undefined** *(impl-regex, impl-architecture)*

D-7 and Step 9 reference `ParseWarning` but never define it. Add to `src/cost.rs`:

```rust
pub struct ParseWarning {
    pub run_number: u32,
    pub cycle: u32,
    pub phase: String,
    pub confidence: ParseConfidence,
    pub raw_match: Option<String>, // None when confidence is None
}
```

Display format for `print_parse_warnings(warnings: &[ParseWarning])` (add to `display.rs`, matching `output-parsing.md` lines 96–103):
- `Low` case: `⚠  Run N (cycle C, phase): cost parse low-confidence — "<raw_match>" matched by generic pattern.`
- `None` case: `⚠  Run N (cycle C, phase): cost could not be parsed. Totals may be inaccurate.`

Cap display at 10 warnings, then `"   ... and N more low-confidence cost parse warnings."` Full Vec is still accumulated for accurate counts. Add `print_parse_warnings` to Step 9.

---

**B-24: State save order on cancellation/timeout is wrong** *(impl-performance)*

Current plan order for cancellation/timeout: SIGTERM → wait 5s → SIGKILL → capture partial output → save state → exit.

If rings itself is killed by an external SIGKILL (e.g., systemd `TimeoutStopSec` expires, OOM killer, `kill -9`) during the 5s grace period, state is never saved.

Corrected order: SIGTERM → **save state** → wait up to 5s for clean exit → SIGKILL if needed → write partial output to run log → exit.

This ensures state is persisted before the grace period begins. If rings is hard-killed after the save, the run is resumable from the last completed position. Write partial output to the run log after the subprocess is done (not before) since the content isn't fully available yet — but the critical state.json write must happen first.

---

**B-25: Budget cap warning flags not persisted — re-fire after resume** *(impl-error-handling, impl-architecture)*

`budget_warned_80` and `budget_warned_90` are engine-local booleans, reset to `false` on every `run_workflow` invocation. After resume, the engine reconstructs `cumulative_cost` from `costs.jsonl` (per D-6) but starts with `budget_warned_80 = false`. If the reconstructed cost is already > 80% of cap, the 80% warning fires again.

Fix: after reconstructing `cumulative_cost` from `costs.jsonl` in resume init, initialize:
```rust
budget_warned_80 = cumulative_cost >= 0.8 * budget_cap;
budget_warned_90 = cumulative_cost >= 0.9 * budget_cap;
```
This requires no new `state.json` fields and self-heals from the reconstructed cost. Add this to Step 8.

---

**B-26: Timeout exit code 2 conflicts with `specs/cli/exit-codes.md`** *(impl-docs)*

`specs/cli/exit-codes.md` reserves exit code 2 for: "Fatal error: invalid workflow file, missing prompt file, or `claude` not found on PATH." There is no exit code for timeout in the exit-codes spec. The plan (Step 7) uses exit 2 for timeout, but this is a spec conflict never recorded as such. `specs/execution/engine.md` does use exit 2 for timeout — the two specs disagree.

This must be recorded in REVIEW.md under Conflicts before Step 7 is coded. Implementer should use exit 2 for timeout (per engine.md) and document the exit-codes.md gap as an Open Question requiring future spec reconciliation.

---

### New Open Decisions

**D-11: `Executor: Send + Sync` bound — add it**
Recommendation: yes. Change `MockExecutor` to `Mutex`-based; add bound to `Executor` trait. Record in REVIEW.md.

**D-12: Windows platform scope — out of scope for this batch**
Add `#[cfg(not(unix))]` `compile_error!`. Record scope decision in REVIEW.md.

**D-13: `partial_output()` return type**
Recommendation: return `Result<String>`. On mutex poison, return `Err(...)`. Callers treat `Err` as empty output (log partial_output failure and proceed with state save). This is safer than silently returning `""`.

**D-14: `print_budget_cap_reached()` caller**
Recommendation: engine returns exit 4; `main.rs` calls the display function in a `4 =>` arm. Consistent with existing pattern for all other exit codes.

**D-15: Add `timeout_per_run: Option<String>` to `ResumeArgs`**
Currently missing from B-8. Add it for consistency with `budget_cap` on `ResumeArgs`. Record in REVIEW.md.

**D-16: F-116 advisory text**
Use verbatim spec text from `cost-tracking.md`, adapt prefix to `⚠  `. Record prefix deviation in REVIEW.md under Conflicts.

**D-17: `canceled_at` on timeout stays null**
On timeout: `canceled_at = null`, `failure_reason = "timeout"`. On cancellation: `canceled_at` is set, `failure_reason` stays null. Record in REVIEW.md.

**D-18: Budget cap comparison uses `>=`**
Both 80%/90% thresholds and cap-exceeded check use `>=`. Record in REVIEW.md.

**D-19: SIGPIPE for stdin write**
Set `SIG_IGN` for SIGPIPE in `main()` before any subprocess spawning, OR handle `ErrorKind::BrokenPipe` explicitly at the `write_all` call site. Recommendation: `SIG_IGN` in `main()` is the conventional Rust CLI approach. Requires `nix::sys::signal::unsafe_signal(Signal::SIGPIPE, SigHandler::SigIgn)` under `#[cfg(unix)]`.

**D-20: `parse_warnings` display cap**
Show at most 10 warnings in the terminal block, then `"... and N more."` Full Vec accumulated for accurate count. No deduplication in this batch (F-036 is BACKLOG).

**D-21: Lock `create_new` retry bound — exactly 1 retry**
After removing a stale lock, retry `create_new` exactly once. If the second attempt also returns `EEXIST`, return `LockError::ActiveProcess` immediately (a concurrent process won the race). Record in REVIEW.md.

---

### Additional Test Requirements

**Duration parsing (add to Step 1):**
- `"  30s  "` (leading/trailing spaces) → 30 (trim before parse)
- `"30S"` (uppercase suffix) → `Err`
- `"5165088294h"` (multiply by 3600 overflows `u64`) → `Err` (use `checked_mul`)
- `toml::from_str("[workflow]\ntimeout_per_run_secs = 300\n...")` → `DurationField::Secs(300)` (B-15 verification test)

**Context directory lock (add to Step 6):**
- `kill(pid, 0)` returns `EPERM` → treated as live process; `acquire()` returns `LockError::ActiveProcess`
- Lock file has `pid = 0` → treated as stale (pid 0 is never a valid user process)
- `context_dir` does not exist → returns `LockError::ContextDirMissing` (not a generic lock error)
- Second `create_new` fails after stale removal → returns `LockError::ActiveProcess` (race condition handled)
- Lock file is empty (crash-during-write) → treated as stale, acquire succeeds with warning

**Cancellation and timeout (add to Step 7):**
- `sigterm_called` on `MockRunHandle` is `true` after cancel flag fires
- `sigkill_called` on `MockRunHandle` is `true` when `ignores_sigterm = true` and 5s expires
- `state.json` exists and is valid BEFORE SIGKILL is sent (B-24: save-before-wait order)
- `NNN.log` for the interrupted run exists and contains partial output (B-19)

**Budget cap (add to Step 8):**
- Resume where `cumulative_cost >= 0.8 * cap` (reconstructed from `costs.jsonl`) → 80% warning does NOT re-fire on first subsequent run
- Exit code 4 → `run.toml` status is `"stopped"` (not `"failed"`)
- `print_budget_cap_reached()` output appears before process exits (test by capturing stderr)

**`ParseWarning` struct (add to Step 9):**
- `ParseConfidence::Low` with non-None `raw_match` → warning includes the raw match snippet
- `ParseConfidence::None` with None `raw_match` → warning uses "could not be parsed" message (no snippet)
- Display cap: 11 low-confidence runs → terminal shows 10 warnings + "... and 1 more"

---

### Additional Spec Gaps

**SG-13: `--force-lock` absent from `specs/state/configuration.md` precedence table**
Intentionally has no config-file or env-var equivalent. Record as Decision in REVIEW.md: "`--force-lock` is CLI-only with no TOML or env-var form; it is an escape hatch, not a workflow-level setting."

**SG-14: `canceled_at` vs. `failure_reason` on timeout not addressed in `cancellation-resume.md`**
See D-17. Record in REVIEW.md under Decisions.

**SG-15: `pct` field missing from Step 8's JSONL `budget_warning` event bullet list**
The spec defines this field. Add `pct: u8` (80 or 90) to the `budget_warning` JSONL event emitted in Step 8.

**SG-16: `scope` field in JSONL events extends beyond spec schema**
The spec's `budget_warning` event schema does not include a `scope` field. Adding it for per-phase support is an extension. Record in REVIEW.md under Decisions as an intentional forward-compatible addition.

**SG-17: `context_dir` existence not validated in `Workflow::validate`**
Lock acquisition will fail with `ENOENT` and a cryptic error if `context_dir` does not exist. Fix in `Workflow::validate`: add a check that `context_dir` exists as a directory (or add `LockError::ContextDirMissing` handling in `ContextLock::acquire`). The validation-at-parse-time approach is cleaner. Record in REVIEW.md.

**SG-18: F-115 parse warning display format is partially defined in `output-parsing.md`**
`specs/execution/output-parsing.md` lines 96–103 show the warning block format. `print_parse_warnings` in `display.rs` must match this format. This is not a gap — it is an existing spec definition that the plan failed to cross-reference.

---

### Additional Discarded Concerns (Wave 2)

- **`getpgid` call**: Not needed when `process_group(0)` is used. PGID = `child.id()` by POSIX invariant.
- **`f64` precision for `budget_cap_usd` in TOML**: `5.00` is exactly representable as IEEE 754. Safe. Known limitation already documented.
- **`costs.jsonl` reconstruction performance**: One-time startup read. At 1000 runs, the file is at most hundreds of KB — trivially fast. Not a concern.
- **Verbose mode pipe buffer deadlock**: With `try_wait()` polling and reader threads draining pipes concurrently, there is no deadlock scenario. The fix in B-18 resolves the ordering concern.
- **`NamedTempFile::persist()` cross-filesystem**: Always use `tempfile::Builder::new().tempfile_in(parent_dir)` (not `NamedTempFile::new()`) to create the temp file in the same directory as the destination. `rename()` within the same filesystem never crosses a mount boundary.
- **Lock file unknown fields in JSON**: `serde_json` silently ignores unknown fields by default. Do NOT add `#[serde(deny_unknown_fields)]` — forward compatibility requires accepting unknown fields from newer rings versions.
- **`ClaudeRunHandle::send_sigterm()`/`send_sigkill()` on dead process**: `nix` returns `ESRCH`. Treat `ESRCH` as success (process is already gone — desired outcome). All other errors: log warning and continue. Do not propagate `ESRCH` as an error.
