# CLI Commands and Flags

## Top-Level Usage

```
rings [OPTIONS] <COMMAND>
```

`rings -h` or `rings --help` prints the top-level help.
`rings <COMMAND> -h` prints help for that subcommand.

---

## Commands

### `rings run <WORKFLOW>`

Execute a workflow.

```
USAGE:
    rings run [OPTIONS] <WORKFLOW>

ARGS:
    <WORKFLOW>    Path to the workflow TOML file

OPTIONS:
    -I, --include-dir <DIR>     Additional context directory to make available.
                                May be specified multiple times. Files from these
                                directories are listed in the prompt preamble
                                passed to Claude Code.
    -n, --max-cycles <N>        Override max_cycles from the workflow file.
    -o, --output-dir <DIR>      Directory for audit logs, cost reports, and
                                state files. Overrides workflow file setting.
                                Default: ~/.local/share/rings/runs/<run-id>/
        --dry-run               Preview execution plan without running anything.
                                Prints the full cycle/phase/run schedule and exits.
                                Compatible with --output-format jsonl: emits a
                                dry_run_plan event containing the plan as structured
                                JSON, suitable for CI workflow validation.
        --step                  Pause after every run. Shows a summary of what changed
                                and what the run cost, then waits for confirmation before
                                proceeding. Useful when testing a new prompt.
                                Incompatible with --output-format jsonl (exit code 2).
        --step-cycles           Pause only at cycle boundaries (after all phases in a
                                cycle complete), not after every individual run.
                                Less granular than --step but less disruptive for
                                multi-run phases.
        --no-completion-check   Suppress ONLY the completion signal warning
                                (signal not found in any prompt file). Does not
                                suppress contract warnings.
        --no-contract-check     Suppress ONLY consumes/produces contract warnings.
                                Does not suppress the completion signal check.
                                Use when you intentionally have phases whose inputs
                                don't exist at startup (created by prior phases).
    -d, --delay <SECS>          Delay in seconds between each individual run.
                                Overrides delay_between_runs in workflow file.
        --cycle-delay <SECS>    Additional delay between cycles (stacks with --delay).
        --quota-backoff         Auto-retry after quota errors instead of exiting.
        --quota-backoff-delay <SECS>
                                Seconds to wait before retrying after quota error
                                (default: 300).
        --quota-backoff-max-retries <N>
                                Max quota retry attempts before giving up (default: 3).
        --strict-parsing        Treat output parse failures as hard errors. When cost
                                parsing confidence is Low or None, halt execution,
                                save state, and exit with code 2. Default: off.
        --budget-cap <DOLLARS>  Stop execution, save state, and exit with code 4 if
                                the running cost total exceeds this value. Compares
                                against cumulative cost including any prior resumed
                                sessions in the ancestry chain.
        --parent-run <RUN_ID>   Declare this run as a continuation of a prior run.
                                Records ancestry link without resuming state.
                                Useful for starting fresh on the same task while
                                maintaining a traceable work history.
    -h, --help                  Print help information
```

**Exit codes:**
- `0` — completion signal detected
- `1` — max_cycles reached without completion
- `2` — workflow file invalid or phase error
- `130` — canceled by user (SIGINT)

---

### `rings resume <RUN_ID>`

Resume a previously canceled or interrupted workflow run.

```
USAGE:
    rings resume [OPTIONS] <RUN_ID>

ARGS:
    <RUN_ID>    Run ID to resume (shown at cancellation time and in run list)

OPTIONS:
    -o, --output-dir <DIR>          Override output directory (defaults to original run's location)
        --output-format <FORMAT>    Output format: human (default) or jsonl. Applies to all
                                    output from the resumed run, matching rings run behavior.
    -h, --help                      Print help information
```

`rings resume` loads the saved state from the run's output directory, reconstructs the cycle/phase position, and continues from the next unstarted run.

Partial runs (canceled mid-execution) are not re-attempted; execution resumes from the next complete step.

---

### `rings list`

List recent runs with their status and cost summary.

```
USAGE:
    rings list [OPTIONS]

OPTIONS:
    -n, --limit <N>              Number of runs to show (default: 20)
        --status <STATUS>        Filter by run status: running, completed, canceled, failed
        --workflow <PATH>        Filter by workflow file path (substring match)
        --since <DATE>           Show runs started after this date.
                                 Accepts ISO 8601 date (2024-03-15) or relative
                                 duration (7d, 24h, 30m).
        --output-format <FORMAT> Output format: human (default) or jsonl.
                                 Alias: --format (for backwards compatibility)
    -h, --help                   Print help information
```

Output columns: `RUN_ID | DATE | WORKFLOW | STATUS | CYCLES | COST`

Pipeline example:
```bash
rings list --status canceled --output-format jsonl | jq -r .run_id | xargs -I {} rings resume {}
```

---

### `rings cleanup`

Remove run data for old runs to free disk space.

```
USAGE:
    rings cleanup [OPTIONS]

OPTIONS:
        --older-than <DURATION>  Remove runs older than this duration (default: 30d).
                                 Accepts duration strings: 7d, 30d, 90d, 24h.
        --dry-run                Show what would be deleted without deleting anything.
    -y, --yes                    Skip confirmation prompt (for scripting).
        --output-format <FORMAT> Output format: human (default) or jsonl.
    -h, --help                   Print help information
```

Removes run directories (logs, manifests, cost data) for runs older than the threshold. Does not remove runs with `status = "running"`. In JSONL mode, emits one event per deleted run.

---

### `rings show <RUN_ID>`

Shorthand for `rings inspect --show summary`. Prints a single-screen summary for the run including ancestry info if present.

---

### `rings inspect <RUN_ID>`

Deep inspection of a single run. Supports multiple `--show` views: `summary`, `cycles`, `files-changed`, `data-flow`, `costs`, `claude-output`. See inspect-command.md for full detail.

```
OPTIONS:
    --show <VIEW>                View to display (repeatable). Default: summary
    --cycle <N>                  Filter to a specific cycle
    --phase <NAME>               Filter to a specific phase
    --output-format <FORMAT>     human or jsonl. Alias: --format
```

---

### `rings lineage <RUN_ID>`

Display the full ancestry chain for a run: all parent and child runs linked by `parent_run_id`, with aggregate cost and cycle totals. See inspect-command.md.

---

## Global Options

These options apply to all subcommands:

```
    -v, --verbose                   Enable verbose output (stream Claude Code stdout live)
        --no-color                  Disable colored output (also respects NO_COLOR env var)
        --output-format <FORMAT>    Output format: "human" (default) or "jsonl".
                                    Alias: --format (accepted for backwards compatibility).
                                    "human": rich terminal display to stderr
                                    "jsonl": newline-delimited JSON events to stdout,
                                             suitable for automation and monitoring.
                                    Pipeline examples:
                                      rings run wf.toml --output-format jsonl | jq 'select(.event == "run_end") | .cost_usd'
                                      rings list --output-format jsonl | jq 'select(.status == "canceled")'
                                      rings inspect <run-id> --output-format jsonl | jq .
        --version                   Print rings version
```

---

## Dry Run Mode

`rings run --dry-run workflow.toml` prints the execution plan without invoking Claude Code:

```
Dry run: my-workflow.toml
  completion_signal: "TASK_COMPLETE"
  context_dir:       ./src
  max_cycles:        10

  Cycle structure (repeating):
    Phase 1: builder  ×3  (prompt: prompts/builder.md)
    Phase 2: reviewer ×1  (prompt: prompts/reviewer.md)

  Total runs per cycle: 4
  Maximum total runs:   40

  Prompt check:
    ✓ "TASK_COMPLETE" found in prompts/builder.md (line 12)
    ✗ "TASK_COMPLETE" not found in prompts/reviewer.md
```

---

## Configuration File

rings looks for configuration in these locations (first found wins):

1. `.rings-config.toml` in the current working directory
2. `~/.config/rings/config.toml` (XDG config home)

```toml
# ~/.config/rings/config.toml

# Default output directory for all runs
# default_output_dir = "~/.local/share/rings/runs"

# Whether to show color output
# color = true
```

The workflow TOML file is separate from the config file. The config file holds user-level defaults, not workflow definitions.
