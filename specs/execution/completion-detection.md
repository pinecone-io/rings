# Completion Detection

## Signal-Based Completion

A workflow completes successfully when the `completion_signal` string is found anywhere in a phase run's output (stdout+stderr combined).

The check is **case-sensitive exact substring match** by default. The signal may appear anywhere in the output.

### Completion signal match modes

The match behavior can be configured per-workflow via `completion_signal_mode`:

| Mode | Behavior | When to use |
|------|----------|-------------|
| `substring` (default) | Signal matches anywhere in output | Simple, but risks false positives if signal appears in prose |
| `line` | Signal must occupy an entire line (trimmed) | Recommended for most workflows — prevents prose false positives |
| `regex` | Signal is a regular expression | Advanced use cases requiring pattern matching |

```toml
[workflow]
completion_signal = "TASK_COMPLETE"
completion_signal_mode = "line"   # default: "substring"
```

**Case sensitivity:** All three modes are case-sensitive. The signal string (and regex patterns) are matched against the literal bytes of executor output with no case folding. Use a regex with `(?i)` if case-insensitive matching is needed.

**False positive risk with `substring` mode:** If the completion signal is a common phrase that might appear in Claude Code's prose (e.g., "Once the task is TASK_COMPLETE, move on"), the workflow will terminate prematurely. The startup advisory check warns if the signal appears in the prompt outside of an instruction context, but cannot catch all cases. Use `line` mode to require the signal to be on its own line.

## When the Check Runs

The completion check runs after each individual run completes (not during streaming). This means:

- If the builder phase runs 3 times per cycle, the check occurs after runs 1, 2, and 3.
- If the signal is found after run 2, run 3 is never started.
- If the signal is found mid-cycle, the remaining phases in that cycle are skipped.

## Restricting which phases can trigger completion

By default, any phase can trigger completion. To restrict completion detection to specific phases:

```toml
[workflow]
completion_signal = "TASK_COMPLETE"
completion_signal_phases = ["reviewer"]   # only reviewer can complete the workflow
```

When `completion_signal_phases` is set, rings still scans all phases' output — but only triggers completion when the signal appears in output from a listed phase. Output from other phases is still written to logs and scanned for resume commands, but the completion check is skipped.

**Use case:** Prevent a builder phase from short-circuiting a reviewer. The reviewer acts as a gate — the workflow only completes when the reviewer is satisfied.

## Prompt Design Guidance (informational)

The completion signal mechanism works best when prompts instruct Claude Code to print the signal as part of its output when the task is done. Example prompt snippet:

```
When the implementation is complete and all tests pass, print exactly:
TASK_COMPLETE
```

rings does not enforce how the signal appears in output — it only checks whether the string is present.

## Startup Warning

At startup, rings reads each prompt file and checks whether the `completion_signal` string appears anywhere in the file contents.

If the signal is absent from ALL prompt files, rings prints:

```
⚠  Warning: completion_signal "TASK_COMPLETE" was not found in any prompt file.

   If no prompt instructs Claude Code to output this signal, the workflow will
   run until max_cycles is reached or you cancel with Ctrl+C.

   Prompt files checked:
     - prompts/builder.md  (no match)
     - prompts/reviewer.md (no match)

   Use --no-completion-check to suppress this warning.

Continue? [y/N]:
```

If the user answers `N` (or presses Enter), rings exits without running anything.

If the signal IS found in at least one prompt, rings notes this in verbose output but does not pause execution:

```
[verbose] Completion signal "TASK_COMPLETE" found in:
[verbose]   prompts/builder.md (line 24)
```

## max_cycles Termination

When `max_cycles` is reached without the completion signal being found, rings exits with code 1 and prints:

```
✗  Workflow ended: max_cycles (50) reached without detecting completion signal.

   Run ID:   run_20240315_143022_a1b2c3
   Cycles:   50
   Total runs: 200
   Total cost: $4.23

   To review output: rings show run_20240315_143022_a1b2c3
   Audit logs: ~/.local/share/rings/runs/run_20240315_143022_a1b2c3/
```
