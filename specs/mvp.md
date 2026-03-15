# rings MVP Scope

← [specs/index.md](index.md)

The MVP is the smallest version of rings that can be used to develop rings itself. It runs Claude Code through TOML-defined phase/cycle loops, tracks cost, and supports safe interruption and resumption. Everything else is deferred.

## Goals

1. `rings run` a workflow and watch it execute
2. See cost per run and running total — no surprise spend
3. Ctrl+C saves state; `rings resume` picks up exactly where it left off
4. Per-run log files capture everything claude wrote
5. Shippable as a single static binary

## CLI — 2 Commands

### `rings run <workflow.toml>`

Starts a new workflow execution.

```
rings run my-task.rings.toml
rings run my-task.rings.toml --output-dir ./logs
rings run my-task.rings.toml --verbose
rings run my-task.rings.toml --max-cycles 10
rings run my-task.rings.toml --delay 5
```

Flags:
| Flag | Description |
|------|-------------|
| `--output-dir <path>` | Override output directory for this run |
| `--max-cycles <n>` | Override max_cycles from workflow file |
| `--delay <secs>` | Override delay_between_runs from workflow file |
| `--verbose` | Stream executor output live to terminal |
| `--no-completion-check` | Skip the startup warning if signal not found in prompts |

### `rings resume <run-id>`

Resumes a previously canceled or failed run from the last completed step.

```
rings resume run_20240315_143022_a1b2c3
rings resume run_20240315_143022_a1b2c3 --verbose
```

Flags: same as `rings run` (overrides apply to the resumed execution).

### Not in MVP

`rings list`, `rings show`, `rings inspect`, `rings lineage`, `rings cleanup`

Shell completions and man page generation are also deferred.

## Workflow TOML — Supported Fields

```toml
[workflow]
# Required
completion_signal = "RINGS_DONE"   # substring match only — line/regex modes deferred
context_dir = "./src"
max_cycles = 20                    # required in MVP; no unlimited mode

# Optional
output_dir = "./rings-output"      # default: ~/.local/share/rings/runs/
delay_between_runs = 5             # seconds between runs; default: 0

[[phases]]
name = "builder"                   # required; must be unique
prompt = "./prompts/builder.md"    # exactly one of prompt or prompt_text
# prompt_text = "..."              # inline alternative
runs_per_cycle = 3                 # default: 1
```

### Deferred workflow fields

`completion_signal_mode`, `completion_signal_phases`, `budget_cap_usd`,
`delay_between_cycles`, `timeout_per_run_secs`, all `manifest_*` fields,
`snapshot_cycles`, `[executor]` block.

## Prompt Template Variables

Available inside prompt files and `prompt_text`:

| Variable | Value |
|----------|-------|
| `{{phase_name}}` | Current phase name |
| `{{cycle}}` | Current cycle number |
| `{{max_cycles}}` | Configured max cycles |
| `{{run}}` | Global run number |
| `{{iteration}}` | Iteration within this phase this cycle |
| `{{runs_per_cycle}}` | Total runs per cycle for this phase |
| `{{cost_so_far_usd}}` | Cumulative cost to this point |

Deferred: `{{phase_cost_usd}}`, `{{cycle_cost_usd}}`.

## Execution

**Executor:** hardcoded Claude Code (`claude --dangerously-skip-permissions -p -`). The `[executor]` configuration block is deferred.

**Completion detection:** substring match only. `completion_signal_mode` and `completion_signal_phases` are deferred.

**SIGINT (Ctrl+C):** save state, print cancellation summary with resume command, exit 130.

**Error handling:** classify non-zero exits as quota / auth / unknown. Pause and save state in all cases. Print resume instructions. Exit code 3.

**Deferred execution features:** `timeout_per_run_secs`, `delay_between_cycles`, `--include-dir`, step-through mode (`--step`, `--step-cycles`), advisory checks (most), strict parsing mode.

## Observability

### Terminal output (human mode only — JSONL deferred)

```
● rings  my-task.rings.toml
  Run ID: run_20240315_143022_a1b2c3

  Cycle 1/20 ─────────────────────────────────────────
  ↻  builder  1/3   $0.023   [00:14]
  ↻  builder  2/3   $0.031   [00:18]
  ↻  builder  3/3   $0.028   [00:16]
  ↻  reviewer 1/1   $0.019   [00:11]
  Cycle cost: $0.101

  Cycle 2/20 ─────────────────────────────────────────
  ↻  builder  1/3   $0.025 ...

✓  Completed on cycle 2, run 7 (phase: reviewer)
   Total cost: $0.203  ·  7 runs  ·  elapsed: 4m12s
   Audit log: ~/.local/share/rings/runs/run_20240315_143022_a1b2c3/
```

### Cost tracking

- Cost shown inline after each run completes
- Cycle subtotal shown at each cycle boundary
- Final summary on completion or cancellation
- Parsed from claude output using the `claude-code` pattern profile (lenient — parse failures warn but never halt)

### Per-run log files

Every run's raw executor output is written to:
```
<output_dir>/<run-id>/runs/<NNN>.log
```

`--verbose` additionally streams this output live to the terminal.

### Deferred observability

JSONL output mode, OpenTelemetry, file manifest tracking, `costs.jsonl` aggregation,
`rings inspect`, `rings lineage`, advisory check warnings.

## State Files

Written to `<output_dir>/<run-id>/`:

```
run.toml       — metadata: workflow path, start time, run_id, rings_version
state.json     — last completed position: {cycle, phase_index, iteration}; updated after every run
costs.jsonl    — one entry per completed run: {run, cycle, phase, cost_usd}
runs/
  001.log      — raw executor output for run 1
  002.log      — raw executor output for run 2
  ...
```

`run.toml` format:
```toml
run_id = "run_20240315_143022_a1b2c3"
workflow_file = "/abs/path/to/my-task.rings.toml"
started_at = "2024-03-15T14:30:22Z"
rings_version = "0.1.0"
status = "running"   # running | completed | canceled | failed
```

### Deferred state features

`summary.md`, ancestry fields (`parent_run_id`, `continuation_of`), `failure_reason` detail beyond basic error class, config file loading (XDG), run lock files.

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Completion signal detected |
| `1` | `max_cycles` reached without completion |
| `2` | Invalid workflow / missing files / executor not found |
| `3` | Executor exited non-zero (quota, auth, or unknown). State saved. |
| `130` | Canceled by user (Ctrl+C) |

## Distribution

Single static binary. MVP targets the developer's own platform first; multi-platform release pipeline is deferred.

```bash
cargo build --release
```

## Example Workflow (dogfooding rings)

```toml
[workflow]
completion_signal = "RINGS_DONE"
context_dir = "."
max_cycles = 30
delay_between_runs = 10

[[phases]]
name = "builder"
runs_per_cycle = 3
prompt_text = """
You are working on the rings CLI (Rust). Review TASK.md for the current objective.
Check the existing code and make progress toward the goal.
Run `cargo check` to verify your changes compile.
When the task described in TASK.md is fully complete, print exactly: RINGS_DONE
"""

[[phases]]
name = "reviewer"
runs_per_cycle = 1
prompt_text = """
Review the recent changes to the rings codebase.
Check that the implementation matches the relevant spec in specs/.
Run `cargo check` and `cargo test` if tests exist.
Write a brief assessment to REVIEW.md — what's done, what still needs work.
If everything is complete and correct, print exactly: RINGS_DONE
"""
```
