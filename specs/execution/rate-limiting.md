# Rate Limiting and Sleepy Mode

## Overview

rings supports configurable delays between runs to throttle execution pace. This serves two use cases:

1. **Preventive throttling** — slow down a long-running workflow to avoid exhausting quota mid-run
2. **Automatic backoff on quota errors** — rather than immediately failing and requiring manual resume, rings can wait and retry automatically

These are configured independently.

## Inter-Run Delay (Sleepy Mode)

A fixed delay inserted after every run, before the next one starts.

### Configuration

In the workflow TOML:

```toml
[workflow]
completion_signal = "DONE"
context_dir = "./src"

# Delay between each individual run.
# Accepts integer seconds or a duration string ("30s", "1m", "1h").
# Applies after every run regardless of phase.
# Default: 0 (no delay)
delay_between_runs = 30      # integer: seconds
# delay_between_runs = "30s" # string: same as above
# delay_between_runs = "5m"  # string: 5 minutes

# Additional delay inserted after each complete cycle.
# Stacks with delay_between_runs (both apply at cycle boundaries).
# Accepts integer seconds or a duration string.
# Default: 0
delay_between_cycles = 60
# delay_between_cycles = "1m"
```

**Duration string format:** Accepts suffixes `s` (seconds), `m` (minutes), `h` (hours). Examples: `"30s"`, `"5m"`, `"1h30m"`. An integer without a suffix is treated as seconds. Invalid duration strings (e.g., `"5 minutes"`) produce a config error at startup (exit code 2).

CLI override:
```bash
rings run --delay 30 workflow.toml           # sets delay_between_runs
rings run --cycle-delay 60 workflow.toml    # sets delay_between_cycles
```

Precedence: CLI flag > workflow TOML > 0 (no delay).

### Display During Delay

During a delay, rings replaces the spinner with a countdown display:

```
⏸  Waiting 30s before next run...  27s remaining  (Ctrl+C to cancel)
```

The countdown updates every second. Pressing Ctrl+C during a delay triggers the normal cancellation flow (save state, print resume command).

### Delay in JSONL Mode

In `--output-format jsonl`, rings emits a `delay_start` event:

```jsonl
{"event":"delay_start","delay_secs":30,"reason":"inter_run","timestamp":"..."}
{"event":"delay_end","timestamp":"..."}
```

### Dry Run Display

`--dry-run` includes delay information in its output:

```
  Cycle structure (repeating):
    Phase 1: builder  ×3  (prompt: prompts/builder.md)
    Phase 2: reviewer ×1  (prompt: prompts/reviewer.md)

  Delays:
    Between runs:   30s
    Between cycles: 60s

  Estimated minimum time per cycle: 4 runs × 30s + 60s = 180s
  Estimated minimum total time:     50 cycles × 180s = 2h 30m
```

## Automatic Backoff on Quota Error

When a quota error is detected (see error-handling.md), rings can automatically wait and retry instead of exiting immediately.

### Configuration

```toml
[workflow]
# Whether to automatically retry after a detected quota error.
# Default: false (exit with code 3 and save state)
quota_backoff = true

# How long to wait before retrying after a quota error, in seconds.
# Default: 300 (5 minutes)
quota_backoff_delay = 300

# Maximum number of quota retries before giving up and exiting.
# Default: 3
quota_backoff_max_retries = 3
```

CLI override:
```bash
rings run --quota-backoff --quota-backoff-delay 600 workflow.toml
```

### Backoff Display

```
⚠  Claude hit a usage limit on run 7. Quota backoff enabled.
   Waiting 5m 00s before retry...  4m 32s remaining  (Ctrl+C to cancel)
   (Retry 1/3)
```

After the wait, rings retries the same run (same cycle, phase, iteration). The retried run gets a new log file (`007-retry-1.log`).

### Backoff in OTel

A `rings.quota_backoff` span event is emitted on the current phase run span with attributes:
- `rings.retry_number`
- `rings.backoff_delay_secs`

### Giving Up

If `quota_backoff_max_retries` is exhausted without a successful run:

```
✗  Claude hit usage limits 3 times on run 7. Max retries exceeded.

   Progress saved. Resume after your quota resets:
     rings resume run_20240315_143022_a1b2c3
```

Exit code `3`.

### Interaction with delay_between_runs

The quota backoff delay is separate from and in addition to `delay_between_runs`. After a successful retry, the normal `delay_between_runs` still applies before the next run.

## Precedence Summary

| Setting | CLI flag | Workflow TOML | Default |
|---------|----------|---------------|---------|
| delay_between_runs | `--delay` | `delay_between_runs` | 0 (accepts int seconds or duration string) |
| delay_between_cycles | `--cycle-delay` | `delay_between_cycles` | 0 (accepts int seconds or duration string) |
| quota_backoff | `--quota-backoff` | `quota_backoff` | false |
| quota_backoff_delay | `--quota-backoff-delay` | `quota_backoff_delay` | 300 |
| quota_backoff_max_retries | `--quota-backoff-max-retries` | `quota_backoff_max_retries` | 3 |

## Use Case Examples

### Avoid quota exhaustion on a long overnight run

```toml
[workflow]
completion_signal = "DONE"
context_dir = "./src"
max_cycles = 100
delay_between_runs = 60      # 1 minute between each run
delay_between_cycles = 120   # extra 2 minutes between cycles

[[phases]]
name = "builder"
prompt = "prompts/build.md"
runs_per_cycle = 5
```

### Resilient unattended run with automatic recovery

```toml
[workflow]
completion_signal = "DONE"
context_dir = "./src"
delay_between_runs = 30
quota_backoff = true
quota_backoff_delay = 600      # wait 10 minutes on quota hit
quota_backoff_max_retries = 5  # try up to 5 times before giving up
```
