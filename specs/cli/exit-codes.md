# Exit Codes

rings uses standard exit codes. All codes are documented in the man page and `--help` output for the `run` subcommand.

## `rings run`

| Code | Meaning |
|------|---------|
| `0` | Workflow completed successfully (completion signal detected) |
| `1` | Workflow ended without completion: `max_cycles` reached |
| `2` | Fatal error: invalid workflow file, missing prompt file, `claude` not found on PATH, or a `produces_required` phase contract was violated |
| `3` | Claude exited non-zero (quota exhausted, auth failure, or unknown error). State saved — use `rings resume`. |
| `4` | Budget cap reached (`--budget-cap` or `budget_cap_usd`). State saved — use `rings resume`. |
| `130` | Canceled by user (SIGINT / Ctrl+C) or by SIGTERM. rings uses `130` for both signals — this is the conventional exit code for keyboard interruption and is broadly understood by process managers. SIGTERM is treated as a graceful stop request, not a crash. |

## `rings resume`

| Code | Meaning |
|------|---------|
| `0` | Resumed run completed successfully |
| `1` | Resumed run hit max_cycles |
| `2` | Cannot resume: state file missing, corrupt, or run already completed |
| `3` | Claude error during resumed run — state saved again |
| `4` | Budget cap reached during resumed run — state saved |
| `130` | Canceled during resumed execution (SIGINT or SIGTERM) |

## `rings list` / `rings show` / `rings inspect` / `rings lineage`

| Code | Meaning |
|------|---------|
| `0` | Success |
| `2` | Run ID not found, data unreadable, or other error |

## Error Output

All error messages are written to **stderr**, never stdout. This ensures:
- `rings run workflow.toml > output.txt` captures only JSONL events (if in JSONL mode), not error text
- Errors are visible in the terminal even when output is redirected
- Automation can distinguish error messages (stderr) from structured output (stdout)

Error messages follow this format:
```
Error: <concise description of what went wrong>

<optional context or suggestion>
```

Examples:
```
Error: Cannot read workflow file: ./missing.rings.toml
  No such file or directory (os error 2)

Error: Invalid workflow file: ./my.rings.toml
  Line 4: phases must have a unique name, but "builder" appears twice

Error: 'claude' not found on PATH.
  rings requires Claude Code to be installed.
  See: https://claude.ai/code

Error: Phase "builder" failed: claude exited with code 1
  See run log: ~/.local/share/rings/runs/run_20240315_143022_a1b2c3/runs/007.log
```

## Distinguishing Completion vs Timeout in Scripts

```bash
rings run workflow.toml
exit_code=$?

case $exit_code in
  0)   echo "Done! Signal detected." ;;
  1)   echo "Timed out (max_cycles reached)." ;;
  2)   echo "Fatal error — check stderr." ;;
  3)   echo "Claude error (quota/auth). State saved — check stderr for resume command." ;;
  4)   echo "Budget cap reached. State saved — resume when ready or adjust --budget-cap." ;;
  130) echo "Canceled (Ctrl+C or SIGTERM)." ;;
esac
```
