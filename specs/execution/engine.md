# Execution Engine

## Responsibilities

The execution engine orchestrates the cycle/phase/run loop. It is the central component of rings, coordinating:

1. Cycle and run sequencing
2. Phase invocation (delegating to the process module)
3. Completion signal detection after each run
4. State persistence after each completed run
5. Cost accumulation per phase and globally
6. Runtime display updates

## Execution Loop (pseudocode)

```
load workflow
validate workflow (hard errors → exit 2)
run startup advisory checks (soft warnings)
generate run_id
initialize state (or load from saved state for resume)
print run header

for cycle in 1..=max_cycles:
  if cycle > 1: emit cycle_cost for previous cycle
  for phase in phases:
    for iteration in 1..=phase.runs_per_cycle:
      // Per-run advisory checks (cycle 2+)
      if cycle >= 2: check consumes files exist (warn if not)

      display: "Cycle N/max | Phase: builder | Run N/total"
      result = invoke_executor(phase, context_dir)
      record_cost(phase.name, result.cost)
      check produces contracts (warn if violated)
      compute file manifest diff
      append_to_audit_log(run_id, cycle, phase, iteration, result)
      save_state(run_id, cycle, phase_index, iteration)

      // completion_signal_phases restricts which phases can trigger completion.
      // If empty (default), any phase can trigger. If set, only listed phases can.
      if completion_signal in result.output and
         (completion_signal_phases is empty OR phase.name in completion_signal_phases):
        print_completion_summary()
        exit(0)

      if budget_cap_usd and cumulative_cost >= budget_cap_usd:
        print_budget_cap_reached()
        exit(4)

      if strict_parsing and cost_confidence in (Low, None):
        print_strict_parsing_error()
        exit(2)

      if --step and is_tty:
        action = show_step_prompt(run_spec, result, manifest_diff)
        if action == quit: save_state(); exit(130)
        if action == skip_cycle: break inner two loops, advance to next cycle

      apply inter-run delay if configured

  if --step-cycles and is_tty:
    action = show_cycle_step_prompt(cycle, cycle_cost, cycle_file_count)
    if action == quit: save_state(); exit(130)

  apply inter-cycle delay if configured

print_max_cycles_reached()
exit(1)
```

## Startup Sequence

1. Parse and validate workflow file (TOML syntax, required fields, duplicate phase names, runs_per_cycle ≥ 1)
2. Validate all `prompt` file paths exist and are readable (fail fast: list all missing before exiting)
3. Validate `context_dir` exists and is a readable directory
4. Validate `output_dir` path contains no `..` traversal sequences
5. Check executor binary is on PATH (fail early with install instructions if not; see `executor-integration.md`)
6. Run all startup advisory checks (see below)
7. Resolve `output_dir` (CLI flag > env var > workflow TOML > config file > XDG default)
8. Create `output_dir/<run-id>/` with permissions `0700`
9. Write `run.toml` metadata file (workflow path, start time, run_id)
10. Install SIGINT and SIGTERM handlers
11. Begin execution loop

## Startup Advisory Checks

These checks fire at startup (after validation) and produce warnings but never block execution unless the user declines an interactive prompt. All advisory checks can be suppressed with `--no-completion-check` (all checks) or `--no-contract-check` (contract checks only).

| Check | Warning trigger | Suppressed by |
|-------|----------------|---------------|
| Completion signal in prompts | Signal not found in any prompt source | `--no-completion-check` |
| `consumes` file presence | Pattern matches no files AND not mentioned in prompt | `--no-contract-check` |
| `delay_between_runs` sanity | Value > 600 seconds (likely minutes/seconds confusion) | — (always shown) |
| `output_dir` inside repo | `output_dir` resolves to a path containing a `.git` directory above it | — (always shown) |
| `context_dir` empty | `context_dir` contains zero files (Claude Code will see nothing) | — (always shown) |
| `produces` requires manifests | `produces` declared but `manifest_enabled = false` | — (always shown) |
| Disk space | Available disk space in output_dir < 100 MB: warning; < 10 MB: fatal error (exit 2) | — (always shown) |
| Sensitive files in context_dir | `context_dir` contains files matching credential patterns (`.env`, `*.key`, `*.pem`, etc.) | `--no-sensitive-files-check` |
| `snapshot_cycles` storage estimate | `snapshot_cycles = true` and estimated storage > 100 MB | — (interactive prompt in TTY, proceeds in non-TTY) |

### `output_dir` inside repo warning

```
⚠  output_dir resolves to a path inside a git repository:
   ./rings-output/ is under /home/user/my-project/ (which contains .git)
   rings run logs and cost data will be written here and may be accidentally committed.
   Consider adding rings-output/ to .gitignore, or omit output_dir to use the default
   off-repo location (~/.local/share/rings/runs/).
```

### `context_dir` empty warning

```
⚠  context_dir ("./src") contains no files.
   The executor will start with an empty working directory.
   If this is intentional (the executor will create files from scratch), ignore this warning.
```

### `delay_between_runs` sanity warning

```
⚠  delay_between_runs = 900 seconds (15 minutes) between each run.
   This is unusually long. If you meant 900 milliseconds, rings uses whole seconds.
   Use --delay to override for this run without editing the workflow file.
```

## Resume Sequence

When `rings resume <run-id>` is called:

1. Locate state file at `output_dir/<run-id>/state.json`
2. Load last completed position: `(cycle, phase_index, iteration)`
3. Reload the original workflow file (path stored in `run.toml`)
4. Begin execution loop starting from the next run after the saved position
5. Accumulate cost on top of previously recorded costs

## Error Handling

- If the executor exits with a non-zero code: classify the error using the configured `error_profile` (quota/auth/unknown), save state, print diagnosis and resume command, exit with code 3. See `specs/execution/error-handling.md`.
- If the executor binary is not found on PATH: print a clear error message. Exit code 2.
- If the workflow file cannot be parsed: print the TOML parse error with line numbers. Exit code 2.
- If a prompt file is not readable at startup: list all missing files and exit code 2 (fail fast, before any runs).

## Runtime Advisory Checks

These checks fire during execution and emit warnings to stderr (human mode) or `{"event":"advisory_warning",...}` events (JSONL mode). They never halt execution unless combined with a future `--strict-contracts` flag.

### Cost spike detection

After each run, rings computes a rolling average of `cost_usd` over the last 5 runs (minimum 3 runs of history required). If the current run costs more than **5× the rolling average**, rings emits:

```
⚠  Run 14 cost $0.43 — 6× higher than recent average ($0.07).
   This may indicate a runaway context size or unexpected model behavior.
   Review: ~/.local/share/rings/runs/<run-id>/runs/014.log
```

The threshold multiplier is not configurable in v1.

### No-files-changed streak

If manifest tracking is enabled and a phase that declares `produces` has not changed any files in its declared patterns for **3 consecutive runs**, rings emits:

```
⚠  Phase "builder" has not modified any produces files in the last 3 runs.
   The phase may be stalled or the produces patterns may be wrong.
   Patterns: ["src/**/*.rs", "tests/**/*.rs"]
```

This is distinct from the per-run `produces` warning — this fires only after a sustained streak, reducing noise from single-run no-ops.

### Completion signal in non-final phase

If the completion signal is detected in a phase that is NOT the last phase in the cycle, rings notes this in the completion summary:

```
✓  Completed on cycle 2, run 7 (phase: builder, iteration 2/3)
   Note: completion detected in builder, which is not the final phase (reviewer).
   The reviewer phase did not run on the completing cycle.
   If reviewer validation is required before declaring completion, consider using
   completion_signal_phases to restrict which phases can trigger completion.
```

This is informational only — the workflow exits normally. It flags a potential design issue where a builder can short-circuit a reviewer.

## Inter-Run Delay

After each completed run (before starting the next), if `delay_between_runs > 0`, rings sleeps for the configured duration with a live countdown display. An additional `delay_between_cycles` sleep is inserted at each cycle boundary. Delays are interruptible by Ctrl+C. See `specs/execution/rate-limiting.md`.

## Subprocess Timeout

Each executor subprocess invocation can be given a configurable timeout.

**Configuration:** `timeout_per_run_secs` in the workflow TOML (global or per-phase) or `--timeout-per-run-secs` CLI flag. Accepts integer seconds or duration strings (`"5m"`, `"30s"`). Default: no timeout.

**When the timeout fires:**
1. rings sends SIGTERM to the executor subprocess.
2. rings waits up to 5 seconds for it to exit.
3. If it has not exited, rings sends SIGKILL.
4. The run is recorded as failed with `failure_reason = "timeout"`.
5. State is saved. rings exits with code 2.

The timeout applies to the wall-clock duration of the executor subprocess, not including rings's own overhead (state saves, manifest computation, etc.).

## Parallelism

rings runs phases strictly sequentially. No parallel execution is supported in v1. This is intentional — phases share state through the filesystem and concurrent writes would be unsafe.
