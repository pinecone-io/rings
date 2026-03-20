# TODO

Implementation tasks, ready to build. The `/build` command picks up the next task from here.

---

## F-074: `rings cleanup` — Remove Old Run Data

**Spec:** `specs/cli/commands-and-flags.md` lines 146–158

**Summary:** `rings cleanup` removes run directories older than a configurable threshold to free disk space. Skips runs with `status = "running"`. Supports `--dry-run`, `--yes`, and `--output-format jsonl`.

### Task 1: CLI subcommand and argument parsing

**Files:** `src/cli.rs`, `src/main.rs`

**Steps:**
- [ ] Add `Cleanup(CleanupArgs)` variant to `Command` enum in `src/cli.rs`
- [ ] Define `CleanupArgs` struct:
  - `--older-than <DURATION>` (default: `"30d"`) — accepts duration strings like `7d`, `30d`, `90d`, `24h`
  - `--dry-run` (`bool`) — show what would be deleted without deleting
  - `-y, --yes` (`bool`) — skip confirmation prompt
- [ ] Add `Command::Cleanup(args) => cmd_cleanup(args, cli.output_format)` match arm in `main.rs`

**Tests:**
- [ ] `rings cleanup` parses with no args (older-than defaults to "30d")
- [ ] `rings cleanup --older-than 7d` parses duration
- [ ] `rings cleanup --dry-run --yes` parses both flags

---

### Task 2: Cleanup logic

**Files:** `src/main.rs` (or new `src/cleanup.rs`)

**Steps:**
- [ ] Implement `cleanup_inner(args, output_format) -> Result<i32>`:
  1. Parse `--older-than` using `duration::SinceSpec` (already exists for `rings list --since`)
  2. Scan the base output directory for run directories (reuse pattern from `list::list_runs`)
  3. For each run dir: read `run.toml`, check `started_at` against the cutoff
  4. Skip runs where `status == "running"` (never delete active runs)
  5. Collect candidates as `(run_id, started_at, status, dir_path)`
- [ ] If no candidates found: print "No runs older than {duration} found." and exit 0
- [ ] If `--dry-run`: print what would be deleted (one line per run), exit 0
- [ ] If not `--yes` and stderr is a TTY: prompt `Delete N runs? [y/N]`, read stdin, abort on anything except `y`/`Y`
- [ ] Delete each candidate directory with `std::fs::remove_dir_all`
- [ ] Print summary: "Deleted N runs, freed approximately X MB"
- [ ] In JSONL mode: emit one `{"event":"cleanup_deleted","run_id":"...","path":"..."}` per deleted run, then a summary event

**Tests:**
- [ ] Runs older than threshold are identified for deletion
- [ ] Runs newer than threshold are skipped
- [ ] Running runs are never deleted regardless of age
- [ ] `--dry-run` lists candidates but does not delete
- [ ] `--yes` skips confirmation prompt
- [ ] Empty base directory returns 0 with "no runs found" message
- [ ] JSONL output emits one event per deleted run

---

## F-071: `rings show` — Single-Screen Run Summary

**Spec:** `specs/cli/commands-and-flags.md` line 162–163

**Summary:** `rings show <RUN_ID>` is a shorthand for `rings inspect --show summary`. Currently a stub that prints an error.

### Task 1: Implement show as inspect summary

**Files:** `src/main.rs`, `src/inspect.rs`

**Steps:**
- [ ] Implement the `Summary` view in `inspect_inner` for the `InspectView::Summary` match arm:
  1. Read `run.toml` from the run directory → `RunMeta`
  2. Read `state.json` → `StateFile` (cycles completed, cumulative cost)
  3. Read `costs.jsonl` → per-run cost entries
  4. Display: Run ID, status, workflow file, context_dir, started_at, duration, cycles completed, total cost, total tokens, phase cost breakdown
- [ ] Wire `cmd_show` to call `inspect_inner` with `show: vec![InspectView::Summary]` instead of printing an error stub
- [ ] Support `--output-format jsonl` — emit the summary as a single JSON object

**Tests:**
- [ ] `rings show <valid-run-id>` prints a summary with run ID, status, cost, cycles
- [ ] `rings show <invalid-run-id>` exits 2 with "Run directory not found"
- [ ] Summary includes phase cost breakdown when costs.jsonl exists
- [ ] JSONL mode emits a single JSON summary object
- [ ] Summary gracefully handles missing state.json (shows what it can from run.toml)

---

## F-080: `--cycle-delay` CLI Flag

**Spec:** `specs/cli/commands-and-flags.md` line 58, `specs/execution/rate-limiting.md`

**Summary:** Add `--cycle-delay <SECS>` CLI flag to override `delay_between_cycles` from the workflow file. The workflow TOML field and engine logic already exist — this just wires up the CLI override.

### Task 1: Add CLI flag and wire override

**Files:** `src/cli.rs`, `src/main.rs`

**Steps:**
- [ ] Add `--cycle-delay <SECS>` to `RunArgs` in `src/cli.rs`: `pub cycle_delay: Option<u64>`
- [ ] In `run_inner` in `src/main.rs`, apply the override: `if let Some(cd) = args.cycle_delay { workflow.delay_between_cycles = cd; }`
- [ ] Place the override after workflow parsing but before engine start (same pattern as `--delay`)

**Tests:**
- [ ] `rings run --cycle-delay 60 workflow.toml` parses correctly
- [ ] CLI override takes precedence over workflow TOML value
- [ ] Without the flag, workflow TOML value is used
- [ ] `--cycle-delay 0` disables cycle delay even if TOML sets one

---

## F-109 + F-110: Output Directory Hardening

**Spec:** `specs/observability/audit-logs.md`

**Summary:** Two small security improvements: (1) create output directories with mode 0700 so only the owner can read run logs, and (2) reject `output_dir` values containing `..` to prevent path traversal.

### Task 1: Restricted directory permissions

**Files:** `src/main.rs` (or wherever `create_dir_all` is called for the output directory)

**Steps:**
- [ ] Find all calls to `std::fs::create_dir_all` for the output/run directory
- [ ] On Unix: after creating the directory, set permissions to 0700 using `std::fs::set_permissions` with `std::os::unix::fs::PermissionsExt`
- [ ] Use `#[cfg(unix)]` guard — on non-Unix platforms, skip the permission change (document this limitation)
- [ ] Ensure the permission is set on the run-specific directory, not the parent `~/.local/share/rings/runs/`

**Tests:**
- [ ] Created run directory has mode 0700 on Unix
- [ ] Parent directory permissions are not changed
- [ ] Non-Unix builds compile without error (cfg guard works)

---

### Task 2: Path traversal protection

**Files:** `src/main.rs` or `src/workflow.rs`

**Steps:**
- [ ] Before using any `output_dir` value (from CLI `--output-dir` or workflow TOML), check if the path contains `..` components
- [ ] If `..` is found: print `Error: output_dir must not contain '..' components` and exit 2
- [ ] Apply the check in both `run_inner` (for `--output-dir` flag) and workflow parsing (for TOML `output_dir`)
- [ ] Use `std::path::Path::components()` and check for `Component::ParentDir`

**Tests:**
- [ ] `--output-dir /tmp/safe/path` is accepted
- [ ] `--output-dir /tmp/../etc/rings` is rejected with exit code 2
- [ ] TOML `output_dir = "../outside"` is rejected at workflow parse time
- [ ] Paths with `.` (current dir) are allowed (only `..` is dangerous)

---

## F-089: `--strict-parsing` CLI Flag

**Spec:** `specs/cli/commands-and-flags.md` lines 65–67

**Summary:** When `--strict-parsing` is set, treat cost parse confidence of `Low` or `None` as a hard error — halt execution, save state, exit code 2. Currently cost parsing failures are just warnings.

### Task 1: Add flag and enforcement logic

**Files:** `src/cli.rs`, `src/main.rs`, `src/engine.rs`

**Steps:**
- [ ] Add `--strict-parsing` flag to `RunArgs` in `src/cli.rs`: `pub strict_parsing: bool`
- [ ] Pass it through to `EngineConfig` as `strict_parsing: bool`
- [ ] In the engine, after cost parsing for each run: if `strict_parsing` and confidence is `Low` or `None`:
  1. Save state (same as budget cap flow)
  2. Print error: `Strict parsing enabled: cost confidence too low ({confidence}) on run {N}. Halting.`
  3. Set exit code to 2
  4. Break out of the run loop
- [ ] In JSONL mode, emit a `fatal_error` event before exiting

**Tests:**
- [ ] `--strict-parsing` with `Full` confidence: run continues normally
- [ ] `--strict-parsing` with `Partial` confidence: run continues (only Low/None trigger halt)
- [ ] `--strict-parsing` with `Low` confidence: run halts, state saved, exit 2
- [ ] `--strict-parsing` with `None` confidence: run halts, state saved, exit 2
- [ ] Without `--strict-parsing`: low confidence produces a warning but run continues (existing behavior)

---

## Tech Debt: Remove `unwrap()`/`expect()` from Production Code

**Ref:** CLAUDE.md rule — "No `unwrap()` or `expect()` in production code — all errors propagate via `?` and `anyhow`"

**Summary:** Audit found two `unwrap()`/`expect()` calls in production code paths that could cause hard panics instead of graceful errors.

### Task 1: Replace `.expect()` in Ctrl+C handler setup

**Files:** `src/main.rs`

**Steps:**
- [ ] Replace `.expect("Failed to install Ctrl+C handler")` on line 43 with proper error handling
- [ ] Since `main()` currently returns `()`, either: (a) convert main to return `Result<()>` via `process::exit` wrapper, or (b) use an `if let Err(e)` block that prints the error to stderr and exits with code 2
- [ ] Verify that failure to install the handler produces a clear user-facing error message, not a panic backtrace

**Tests:**
- [ ] Existing tests continue to pass (`just validate`)

---

### Task 2: Replace `.unwrap()` in dry-run phase position lookup

**Files:** `src/main.rs`

**Steps:**
- [ ] Replace `.unwrap()` on line 170 (phase position lookup in dry-run output) with `.unwrap_or(0)` or a safe fallback that cannot panic
- [ ] The current code iterates `plan.phases` and looks up each phase's index by name within the same collection — logically infallible, but should still be defended

**Tests:**
- [ ] Existing dry-run tests continue to pass
- [ ] `just validate` clean

---

## Tech Debt: Harden `costs.jsonl` Append Against Partial Writes

**Ref:** `specs/observability/audit-logs.md`

**Summary:** `append_cost_entry()` in `src/audit.rs` opens the file in append mode and writes a JSON line. If the process is killed mid-write (e.g., SIGKILL, OOM kill), the file can be left with a truncated JSON line. On resume, `recover_last_run_from_costs()` already skips malformed lines, but a partial line could still corrupt the next append if it doesn't end with a newline.

### Task 1: Atomic-ish cost entry append

**Files:** `src/audit.rs`

**Steps:**
- [ ] Serialize the full line (JSON + newline) to a `String` first (already done)
- [ ] Write the entire serialized bytes in a single `write_all()` call instead of `writeln!()` (which may split the write into data + newline)
- [ ] Call `file.sync_data()` after the write to flush to disk before returning
- [ ] Add a recovery safeguard: when reading `costs.jsonl` for resume, if the last line does not end with `\n`, truncate the file to remove the partial line before appending

**Tests:**
- [ ] Existing cost parsing and state recovery tests continue to pass
- [ ] Test that a costs.jsonl with a truncated last line (no trailing newline) is handled gracefully on read
- [ ] `just validate` clean

---

## Tech Debt: Validate Parsed Cost Values Are Non-Negative

**Ref:** `specs/observability/cost-tracking.md`, `specs/execution/output-parsing.md`

**Summary:** `parse_cost_from_output()` in `src/cost.rs` accepts any dollar amount matched by the regex, including negative values. A malformed or adversarial executor output like `Cost: $-10.00` would parse as `cost_usd: Some(-10.0)`, which would subtract from cumulative cost and could allow budget cap bypass.

### Task 1: Add non-negative validation to cost parser

**Files:** `src/cost.rs`

**Steps:**
- [ ] After extracting `cost_usd` from any regex match, clamp or reject negative values: if `cost < 0.0`, treat as `ParseConfidence::None` with `cost_usd: None`
- [ ] Also reject `NaN` and `Infinity` values (defense in depth against malformed f64 parsing)
- [ ] Log a warning when a negative/invalid cost is encountered (similar to low-confidence warning)

**Tests:**
- [ ] `parse_cost_from_output("Cost: $-10.00 ...")` returns confidence `None`, cost `None`
- [ ] `parse_cost_from_output("Cost: $0.00 ...")` still works (zero is valid)
- [ ] Existing cost parsing tests continue to pass
- [ ] `just validate` clean

---
