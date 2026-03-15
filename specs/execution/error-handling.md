# Executor Error Handling

## Philosophy

When the executor exits with a non-zero exit code, rings **pauses and saves state** rather than aborting. The most common causes — quota exhaustion and authentication failures — are external conditions, not workflow bugs. The user should be able to fix the condition and resume exactly where they left off.

rings never silently discards progress. Any run that completes (even partially) is logged before rings exits.

## Error Classification

After a non-zero exit, rings scans the run's combined output for known error patterns to give a specific diagnosis. The patterns used depend on the `error_profile` configured for the executor.

## Error Profiles

Error classification behavior is controlled by the `error_profile` field in the `[executor]` config.

### `"claude-code"` (default)

#### Quota / Rate Limit

Detected by scanning output for patterns including (case-insensitive):
- `"usage limit reached"`
- `"rate limit"`
- `"quota exceeded"`
- `"too many requests"`
- `"429"`
- `"claude.ai/settings"`

#### Authentication Error

Detected by scanning output for patterns including (case-insensitive):
- `"authentication"`
- `"invalid api key"`
- `"unauthorized"`
- `"401"`
- `"please log in"`
- `"not logged in"`

### `"none"`

No pattern matching is performed. All non-zero exits are classified as `Unknown`. Use this for executors whose error output format is unknown or irrelevant.

### Custom profile

A TOML inline table with `quota_patterns` and `auth_patterns` string arrays:

```toml
[executor]
error_profile = { quota_patterns = ["rate limit", "quota exceeded", "429"], auth_patterns = ["unauthorized", "401", "invalid key"] }
```

All patterns are matched case-insensitively as substrings of the executor's combined output. The first category with a match wins; if neither matches, the error is classified as `Unknown`.

## Error Behavior by Classification

### Quota / Rate Limit

1. Save state (same as Ctrl+C cancellation).
2. Print quota-specific message with resume instructions.
3. Exit code `3`.

```
✗  Executor hit a usage limit on run 7 (cycle 2, builder).

   This is likely a quota or rate limit. No further runs will be attempted.

   Progress saved. To resume after your quota resets:
     rings resume run_20240315_143022_a1b2c3

   Executor output snippet:
     "Usage limit reached. Visit claude.ai/settings to view your usage."

   Run ID:    run_20240315_143022_a1b2c3
   Cost so far: $2.14
   Audit log:   ~/.local/share/rings/runs/run_20240315_143022_a1b2c3/
```

### Authentication Error

Same pause-and-save as quota, but the message directs the user to fix credentials rather than wait.

```
✗  Executor encountered an authentication error on run 7 (cycle 2, builder).

   This is likely an invalid or expired API key / session.
   This error is not recoverable by waiting — fix credentials before resuming.

   To fix: verify authentication for your executor, then:
     rings resume run_20240315_143022_a1b2c3
```

### Unknown Error

Any non-zero exit that doesn't match the configured error profile patterns.

```
✗  Executor exited with code 1 on run 7 (cycle 2, builder).

   Cause unknown. Last 10 lines of output:
     [... tail of run log ...]

   Progress saved. If the error is transient, you may resume:
     rings resume run_20240315_143022_a1b2c3

   Full output: ~/.local/share/rings/runs/run_20240315_143022_a1b2c3/runs/007.log
```

## Limit Detection in Executor Output

The executor may output remaining-limit information. rings scans for patterns like:

- `"N requests remaining"`
- `"limit: N"`
- `"usage: N/M"`

When found, rings logs this in the run's audit entry and includes it in the error message if relevant. This is best-effort — output format for limit information varies by executor.

**If `RINGS_OTEL_ENABLED=true`**, rings emits a `rings.quota_warning` span event when limit information is detected in output, even on a successful run (exit code 0). This allows dashboards to show quota headroom over time.

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Completed (signal detected) |
| `1` | max_cycles reached |
| `2` | Invalid workflow / missing files / executor binary not found |
| `3` | Executor exited non-zero — quota, auth, or unknown error. State saved. |
| `130` | Canceled by user (SIGINT) |

## State on Error

The state file written on error is identical in format to the cancellation state. The `status` field in `run.toml` is set to `"failed"` and a `failure_reason` field records the classified error type: `"quota"`, `"auth"`, or `"unknown"`.

```toml
status = "failed"
failure_reason = "quota"
failed_on_run = 7
failed_at = "2024-03-15T14:32:07Z"
```

## Retry Behavior

rings does **not** automatically retry failed runs. The decision to retry is left to the user via `rings resume`. This is intentional — automatic retry on quota errors would just hit the quota again immediately.

For automatic retry with backoff, see `specs/execution/rate-limiting.md`.
