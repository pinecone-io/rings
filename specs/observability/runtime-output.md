# Runtime Output

## Philosophy

rings's default terminal output is designed to give the user situational awareness without overwhelming them. Claude Code's output is captured to logs by default; rings shows its own compact status display.

With `--verbose`, Claude Code's raw output is streamed to the terminal (for debugging or monitoring).

## Status Line

During execution, rings displays a single-line status in the terminal:

```
⟳  Cycle 3/10  |  builder  (run 2/3)  |  $0.47 total
```

- The spinner (`⟳`) animates to show the process is alive.
- Cycle count shows current/max (or `current/?` if max_cycles is not set).
- Phase name and run-within-phase counts are shown.
- Running cost total is shown in real time.

The status line is redrawn in place (not a new line per update).

## Phase Transition Lines

When a new cycle begins, rings emits a newline with the previous cycle's cost:

```
● Cycle 2  →  builder  (run 1/3)  |  cycle 1 cost: $0.14
⟳  Cycle 2/10  |  builder  (run 1/3)  |  $1.23 total
```

When transitioning between phases within a cycle:

```
● Cycle 2  →  reviewer  (run 1/1)
⟳  Cycle 2/10  |  reviewer  (run 1/1)  |  $1.23 total
```

The per-cycle cost on cycle boundaries gives an early signal if a cycle is anomalously expensive compared to previous cycles.

## Startup Header

```
rings v0.1.0
Workflow:  my-task.rings.toml
Context:   ./src
Output:    ~/.local/share/rings/runs/run_20240315_143022_a1b2c3/
Phases:    builder ×10, reviewer ×1  (per cycle)
Max:       50 cycles = 550 runs max

Starting...
```

## Completion Output

On success:

```
✓  Completed on cycle 2, run 12 (phase: builder)
   Signal "TASK_COMPLETE" detected in output.

   Cycles completed:  2
   Total runs:        12
   Duration:          8m 14s
   Total cost:        $1.10

   Cost breakdown:
     builder   (10 runs)  $0.89
     reviewer   (2 runs)  $0.21

   Audit logs: ~/.local/share/rings/runs/run_20240315_143022_a1b2c3/
   Summary:    ~/.local/share/rings/runs/run_20240315_143022_a1b2c3/summary.md
```

## Cancellation Output (Ctrl+C)

When the user presses Ctrl+C:

1. rings sends SIGTERM to any running `claude` subprocess.
2. rings waits up to 5 seconds for the subprocess to exit, then sends SIGKILL.
3. rings captures any `claude resume <uuid>` lines from the partial output.
4. rings saves state.
5. rings prints:

```

Interrupted.

Run ID:    run_20240315_143022_a1b2c3
Progress:  cycle 3, phase builder, run 2/3 (23 runs completed)

Cost so far: $2.14
  builder  (18 runs)  $1.71
  reviewer  (5 runs)  $0.43

To resume this run:
  rings resume run_20240315_143022_a1b2c3

Partial Claude Code sessions (captured resume commands):
  claude resume abc-123-def-456
  claude resume xyz-789-uvw-012

Audit logs: ~/.local/share/rings/runs/run_20240315_143022_a1b2c3/
```

The `claude resume` commands are captured from the subprocess output so the user can manually resume any Claude Code sessions that were in progress.

## Verbose Mode

With `--verbose`, Claude Code's stdout is streamed live to the terminal interleaved with rings's own output. The status line is still displayed, but it appears below the streamed output.

Verbose mode is useful for debugging prompt behavior or monitoring what Claude Code is doing in real time.

## Output Format: Human vs Machine

rings has two output modes, selected by `--output-format` (or `output_format` in config):

### Human mode (default)

All output is designed for a human terminal viewer:
- ANSI colors and box-drawing characters
- Animated spinner during execution
- Aligned columns in summaries
- Friendly labels and units (`$1.23`, `8m 14s`, `42 runs`)
- Progress and status written to **stderr** (so stdout stays clean for piping)
- Claude Code output captured to logs (unless `--verbose`)

### Error events (both modes)

When Claude exits non-zero, rings emits (in addition to normal run_end):

```
# human mode — to stderr:
✗  Executor hit a usage limit on run 7 (cycle 2, builder).
   Waiting... OR  Progress saved. rings resume run_20240315_143022_a1b2c3

# jsonl mode — to stdout (all structured events go to stdout in JSONL mode):
{"event":"executor_error","run":7,"error_class":"quota","exit_code":1,"message":"Usage limit reached","timestamp":"..."}
```

In JSONL mode, all structured events including errors go to **stdout**. Only unstructured fatal errors that prevent JSONL output (e.g., invalid workflow file detected before any event could be emitted) go to stderr.

### JSONL mode (`--output-format jsonl`)

Each significant event is emitted as a newline-delimited JSON object on **stdout**. This is suitable for consumption by automation, monitoring scripts, or dashboards.

### JSONL event envelope

Every JSONL event carries at minimum: `event`, `run_id`, `timestamp`. This allows consumers to correlate any event back to a run with a simple `jq 'select(.run_id == "...")'` without special-casing event types.

### Event types:

```jsonl
{"event":"start","run_id":"run_20240315_143022_a1b2c3","workflow":"my.rings.toml","rings_version":"0.1.0","schema_version":1,"timestamp":"2024-03-15T14:30:22Z"}
{"event":"run_start","run_id":"run_...","run":1,"cycle":1,"phase":"builder","iteration":1,"total_iterations":3,"template_context":{"phase_name":"builder","cycle":1,"max_cycles":10,"iteration":1,"runs_per_cycle":3,"run":1,"cost_so_far_usd":0.00},"timestamp":"..."}
{"event":"run_end","run_id":"run_...","run":1,"cycle":1,"phase":"builder","iteration":1,"cost_usd":0.0234,"input_tokens":1234,"output_tokens":567,"exit_code":0,"produces_violations":[],"timestamp":"..."}
{"event":"completion_signal","run_id":"run_...","run":7,"cycle":2,"phase":"builder","signal":"TASK_COMPLETE","timestamp":"..."}
{"event":"canceled","run_id":"run_20240315_143022_a1b2c3","runs_completed":7,"cost_usd":1.42,"timestamp":"..."}
{"event":"executor_error","run_id":"run_...","run":7,"cycle":2,"phase":"builder","error_class":"quota","exit_code":1,"message":"Usage limit reached","timestamp":"..."}
{"event":"quota_backoff_start","run_id":"run_...","run":7,"retry":1,"max_retries":3,"delay_secs":300,"timestamp":"..."}
{"event":"quota_backoff_end","run_id":"run_...","run":7,"retry":1,"timestamp":"..."}
{"event":"delay_start","run_id":"run_...","run":7,"cycle":2,"phase":"builder","delay_secs":30,"reason":"inter_run","timestamp":"..."}
{"event":"delay_end","run_id":"run_...","run":7,"timestamp":"..."}
{"event":"budget_cap","run_id":"run_...","cost_usd":5.03,"budget_cap_usd":5.00,"runs_completed":42,"timestamp":"..."}
{"event":"max_cycles","run_id":"run_...","cycles":50,"runs_completed":200,"cost_usd":4.23,"timestamp":"..."}
{"event":"summary","run_id":"run_...","status":"completed","cycles":2,"runs":12,"cost_usd":1.10,"duration_secs":494,"phases":[{"name":"builder","runs":10,"cost_usd":0.89},{"name":"reviewer","runs":2,"cost_usd":0.21}],"timestamp":"..."}
{"event":"fatal_error","run_id":null,"message":"Invalid workflow file: ...","timestamp":"..."}
```

### `run_end` fields

| Field | Always present | Description |
|-------|---------------|-------------|
| `run_id` | yes | Run ID |
| `run` | yes | Global run number |
| `cost_usd` | no (null if parse failed) | Cost in USD |
| `input_tokens` | no | Input tokens |
| `output_tokens` | no | Output tokens |
| `exit_code` | yes | executor subprocess exit code |
| `produces_violations` | yes | Array of `produces` patterns that matched no changed files this run. Empty array when all contracts satisfied or no contracts declared. Always present so consumers can filter without null-checks. |
| `cost_confidence` | yes | Cost parse confidence: `"full"`, `"partial"`, `"low"`, or `"none"`. Always present. |
| `total_iterations` | yes | Equals `runs_per_cycle` for this phase. If `runs_per_cycle` is 1 (the default), `total_iterations` is 1. |

### `error` vs `executor_error` vs `fatal_error`

| Event | When emitted | `error_class` |
|-------|-------------|---------------|
| `executor_error` | Executor subprocess exited non-zero | `quota`, `auth`, or `unknown` |
| `fatal_error` | rings itself cannot continue (invalid workflow, missing file, etc.) | — |

`fatal_error` has `run_id: null` if the error occurred before a run ID was assigned. It is the last event in the stream; no `summary` follows.

In JSONL mode:
- No spinner, no color, no box-drawing
- The final `summary` event mirrors what would be printed in human mode

### stderr vs stdout

| Mode | stdout | stderr |
|------|--------|--------|
| human | (empty — all human output goes to stderr) | status display, summaries, warnings, error messages |
| jsonl | all structured events (start, run_start, run_end, executor_error, delay_start, summary, etc.) | unstructured fatal errors only (before first event) |

This ensures `rings run ... | jq` works cleanly in JSONL mode, and that redirecting stdout in human mode captures nothing unexpected.

## Step-Through Mode

`--step` pauses execution after each run and waits for the user to decide what to do next. It is designed for prompt development — you can see exactly what Claude did before letting it run again.

### Pause prompt

After each run completes, rings clears the spinner, prints a compact run summary, and presents an interactive prompt:

```
─────────────────────────────────────────────────────
  Run 3  |  cycle 1  |  builder  (iteration 3/3)
  Cost:   $0.031  (1,204 input, 312 output tokens)
  Files:  2 modified  →  src/main.rs, src/engine.rs
  Signal: not detected

  [Enter] continue   [s] skip to next cycle   [v] view output   [q] quit
─────────────────────────────────────────────────────
```

The user can:

| Key | Action |
|-----|--------|
| `Enter` or `c` | Continue — run the next scheduled run |
| `s` | Skip to next cycle boundary — skip remaining runs in the current cycle and start the next cycle. If the current run is the last in the cycle, this is equivalent to `Enter`. |
| `v` | View output — print the full Claude Code output for this run (the contents of `runs/NNN.log`) to the terminal, then re-display the pause prompt |
| `q` or `Ctrl+C` | Quit — saves state and exits (same as Ctrl+C during normal execution, exit code 130) |

### What the summary shows

| Field | Source | Notes |
|-------|--------|-------|
| Run number | `RunSpec.global_run_number` | Global across all cycles |
| Cost | Parsed from output | Shows `unknown` if parse failed |
| Files modified | File manifest diff | Only shown if `manifest_enabled = true` |
| Completion signal | Output scan | Shows `detected ✓` or `not detected` |

If `manifest_enabled = false`, the Files line is omitted from the summary.

### `--step-cycles` variant

`--step-cycles` pauses only at cycle boundaries. The pause prompt shows a cycle-level summary instead of a per-run summary:

```
─────────────────────────────────────────────────────
  Cycle 1 complete
  Runs:   4  (builder ×3, reviewer ×1)
  Cost:   $0.12 this cycle  ($0.12 total)
  Files:  5 modified across cycle

  [Enter] continue   [v] view cycle summary   [q] quit
─────────────────────────────────────────────────────
```

`v` in `--step-cycles` mode shows the per-run cost and file breakdown for the completed cycle.

### Non-TTY behavior

In non-TTY contexts (CI, piped output), `--step` and `--step-cycles` are silently ignored and execution proceeds without pausing. This prevents pipeline hangs if `--step` is accidentally included in a CI invocation.

### JSONL mode with --step

Specifying `--step` with `--output-format jsonl` is a usage error. rings exits immediately with code 2:

```
Error: --step is incompatible with --output-format jsonl. Remove --step or use human output format.
```

## No-Color Mode

`--no-color` disables ANSI escape codes. rings respects the `NO_COLOR` environment variable as well (https://no-color.org/).
