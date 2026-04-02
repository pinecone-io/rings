# Workflow File Format

Workflows are defined in TOML files. By convention these are named `<task>.rings.toml` but any path may be passed to the CLI.

## Full Schema

```toml
[workflow]
# Required: string that signals successful task completion.
# rings scans each phase's output for this string after every run.
# If found, the entire workflow exits successfully.
completion_signal = "TASK_COMPLETE"

# Optional: how the completion_signal is matched against output.
# "substring" (default): signal matches anywhere in output
# "line": signal must be the entire content of a line (trimmed)
# "regex": signal is a regular expression
# Recommendation: use "line" to prevent false positives from signal
# appearing in Claude Code's prose mid-task.
completion_signal_mode = "line"

# Required: directory where Claude Code is invoked (its working directory).
# All phases share the same working directory. This is how phases share
# state — through files written and read in this directory.
# Relative paths are resolved relative to the workflow file's location,
# not the current working directory. Use "." for the same directory as
# the workflow file.
context_dir = "./src"

# Optional: maximum number of full cycles to execute before stopping.
# A cycle is one complete pass through all phases.
# If omitted, runs until completion_signal is found or the user cancels.
max_cycles = 50

# Optional: how the completion_signal is matched. Default: "substring".
# "line": signal must occupy an entire output line (recommended to prevent prose false positives)
# "regex": signal is a regular expression
completion_signal_mode = "line"

# Optional: restrict which phases can trigger completion.
# Default: any phase. List phase names that are authoritative for completion.
# completion_signal_phases = ["reviewer"]

# Optional: directory for audit logs, cost reports, and state files.
# Defaults to the XDG data home: ~/.local/share/rings/runs/<run-id>/
# Can be overridden per-run with --output-dir flag.
#
# WARNING: If you set output_dir to a path inside your repository (e.g. "./rings-output"),
# rings will write logs, costs, and state files there. Add that path to .gitignore
# or omit this setting to use the default off-repo location.
output_dir = "./rings-output"

# Optional: stop execution (save state, exit 4) if running cost exceeds this amount.
# Applies across ancestry chain (includes prior resumed sessions).
# Can be overridden per-run with --budget-cap flag.
# budget_cap_usd = 10.00

# Optional: delay between individual phase runs.
# Accepts integer seconds or duration string ("30s", "5m", "1h").
# Useful to avoid hitting rate limits on back-to-back invocations.
# Default: 0 (no delay). Overridden by --delay CLI flag.
delay_between_runs = 5
# delay_between_runs = "30s"  # equivalent to above

# Optional: additional delay between full cycles.
# Accepts integer seconds or duration string ("30s", "5m", "1h").
# Stacks with delay_between_runs. Runs at the end of each cycle before
# the next cycle begins. Default: 0. Overridden by --cycle-delay CLI flag.
delay_between_cycles = 30
# delay_between_cycles = "1m"  # equivalent to above

# Optional: timeout for each individual executor subprocess invocation.
# If the subprocess does not exit within this duration, rings sends SIGTERM,
# waits 5 seconds, then sends SIGKILL. The run is recorded as failed with
# error type "timeout", state is saved, and rings exits with code 2.
# Accepts integer seconds or duration string. Default: no timeout.
# timeout_per_run_secs = 300
# timeout_per_run_secs = "5m"  # equivalent

# Optional: named lock for concurrent workflow support.
# By default, only one rings process can run against this context_dir at a time.
# Set lock_name to allow multiple workflows to run concurrently in the same directory,
# each with its own independent lock. Two workflows with different lock_name values
# never block each other. See cancellation-resume.md for details.
# Must match [a-z0-9_-]+ if set.
# lock_name = "planner"

# Optional: deterministic gate checked before each cycle begins.
# If the command exits non-zero, the on_fail action is taken.
# The command runs in context_dir with a default timeout of 30 seconds.
# cycle_gate = { command = "test $(wc -l < TODO.md) -lt 50", on_fail = "stop" }
# cycle_gate = { command = "./scripts/pre-check.sh", on_fail = "error", timeout = "10s" }

# Optional: file manifest behavior
manifest_enabled = true        # set false to disable entirely
manifest_ignore = [            # glob patterns to exclude from manifests
  "**/.git/**",
  "**/target/**",
]
manifest_mtime_optimization = true  # use mtime to skip unchanged files (default: true)
snapshot_cycles = false             # copy context_dir at each cycle boundary

# Optional: executor configuration.
# Defines the binary and arguments used to invoke each phase.
# If omitted, the Claude Code built-in default is used.
[executor]

# Binary name or absolute path. Must be on PATH if not absolute.
# Default: "claude"
binary = "claude"

# Arguments passed to the binary. The prompt is always delivered via stdin.
# Default: ["--dangerously-skip-permissions", "-p", "-"]
args = ["--dangerously-skip-permissions", "-p", "-"]

# Cost parsing strategy. Controls how rings extracts cost data from executor output.
# Built-in profiles:
#   "claude-code" (default): matches Claude Code's output format
#   "none": skips cost extraction; all cost fields recorded as null, no warnings
# Custom: a TOML inline table with regex patterns using named capture groups.
#   Required capture: cost_usd    Optional captures: input_tokens, output_tokens
# cost_parser = "claude-code"
# cost_parser = { pattern = 'Cost: \$(?P<cost_usd>[\d.]+) \((?P<input_tokens>[\d,]+) input' }

# Error classification profile. Controls how rings classifies non-zero executor exits.
# Built-in profiles:
#   "claude-code" (default): recognizes Claude Code's quota and auth error messages
#   "none": no pattern matching; all non-zero exits classified as Unknown
# Custom: a TOML inline table with quota_patterns and auth_patterns string arrays.
# error_profile = "claude-code"
# error_profile = { quota_patterns = ["rate limit", "quota exceeded"], auth_patterns = ["unauthorized", "401"] }

# Regex to extract resumable session identifiers from executor output.
# Named capture group "id" is used as the session identifier.
# Set to "" to disable resume command extraction entirely.
# Default: Claude Code's "claude resume <uuid>" format.
# resume_pattern = 'claude resume (?P<id>\S+)'

[[phases]]
# Required: identifier for this phase, used in logs and cost reports.
name = "builder"

# Optional: per-phase budget cap in USD. Stop execution (save state, exit 4) if the
# cumulative cost of this phase across all cycles exceeds this amount. Applies
# independently of the global budget_cap_usd. Whichever limit triggers first stops execution.
# budget_cap_usd = 5.00

# Optional: per-phase subprocess timeout. Overrides the global timeout_per_run_secs for
# this phase only. Accepts integer seconds or duration string.
# timeout_per_run_secs = 120

# Optional: deterministic gate checked before this phase runs.
# If the command exits non-zero, the on_fail action is taken.
# The command runs in context_dir with a default timeout of 30 seconds.
# gate = { command = "test -f NEEDS_REVIEW.md", on_fail = "skip" }
# gate = { command = "just validate", on_fail = "error", timeout = "60s" }

# Prompt source: exactly one of `prompt` (file path) or `prompt_text` (inline) is required.
#
# Option A — file reference (recommended for long prompts or prompts shared across workflows):
# Relative paths are resolved relative to the workflow file's location.
prompt = "./prompts/builder.md"

# Option B — inline text (recommended for self-contained/portable workflows or documentation):
# Use TOML multiline strings. The workflow file becomes fully self-contained.
# prompt_text = """
# You are a builder agent working in the src/ directory.
# Your task is to implement the feature described in TASK.md.
# When done, print TASK_COMPLETE.
# """

# Optional: how many times this phase runs per cycle. Default: 1.
runs_per_cycle = 10

# Optional: per-phase executor override.
# Any field set here overrides the workflow-level [executor] for this phase only.
# Unset fields inherit from [executor].
# executor.binary = "gemini"
# executor.args = ["--prompt", "-"]
# executor.cost_parser = "none"
# executor.error_profile = "none"

[[phases]]
name = "reviewer"
prompt = "./prompts/reviewer.md"
runs_per_cycle = 1
```

## Prompt Sources: File vs Inline

Each phase must specify exactly one of:

| Key | Type | Description |
|-----|------|-------------|
| `prompt` | string | Path to a prompt file. Relative to the workflow file's location. |
| `prompt_text` | string | Prompt text inline in the TOML. Supports TOML multiline strings (`"""`). |

Specifying both is a validation error. Specifying neither is a validation error.

**When to use `prompt` (file reference):**
- Long prompts that would clutter the workflow file
- Prompts shared across multiple workflow files
- When you want to edit prompts in a dedicated file with syntax highlighting

**When to use `prompt_text` (inline):**
- Self-contained, portable workflows that travel as a single file
- Sharing a workflow example with a colleague or in documentation
- Simple prompts that don't warrant a separate file
- Demonstrating rings without requiring a directory structure

### Inline prompt example (fully self-contained workflow)

```toml
[workflow]
completion_signal = "TASK_COMPLETE"
context_dir = "."
max_cycles = 10

[[phases]]
name = "builder"
runs_per_cycle = 3
prompt_text = """
You are a builder agent. Review the code in this directory and make improvements.
Fix any failing tests. Add tests for uncovered behavior.
When you are satisfied that the code is correct and well-tested, print: TASK_COMPLETE
"""

[[phases]]
name = "reviewer"
prompt_text = """
Review the code written by the builder. Check for correctness, test coverage, and clarity.
Write your feedback to REVIEW_NOTES.md. Be specific about what still needs work.
"""
```

This single file is everything needed to run the workflow — no `prompts/` directory required.

## Validation Rules

1. `completion_signal` must be non-empty.
2. `phases` must contain at least one entry.
3. Every phase must have a unique `name`.
4. Every phase must have exactly one of `prompt` or `prompt_text`.
5. Every `prompt` path must resolve to a readable file at startup.
6. `runs_per_cycle` must be ≥ 1 if specified.
7. `context_dir` must be a readable directory at startup.
8. `completion_signal_mode` must be one of `substring`, `line`, or `regex` if specified.
9. `completion_signal_phases` must contain only names that match a declared phase.
10. `budget_cap_usd` must be a positive number if specified.
11. `executor.binary` must be a non-empty string if specified.
12. `executor.cost_parser` must be `"claude-code"`, `"none"`, or a valid custom pattern table if specified.
13. `executor.error_profile` must be `"claude-code"`, `"none"`, or a valid custom pattern table if specified.
14. `lock_name` must match `[a-z0-9_-]+` if specified.
15. Gate `command` must be a non-empty string if a gate is specified.
16. Gate `on_fail` must be one of `skip`, `stop`, or `error` if specified.
17. Gate `timeout` must be a valid duration string or positive integer if specified.
18. `cycle_gate.on_fail` defaults to `stop`. Phase `gate.on_fail` defaults to `skip`.

## Deterministic Gates

Gates are shell commands that run before a phase or cycle to decide whether execution should proceed. They provide deterministic control flow without consuming an AI invocation.

### Gate Configuration

A gate is a TOML inline table with the following fields:

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `command` | string | yes | — | Shell command executed via `sh -c`. Exit 0 = pass, non-zero = fail. |
| `on_fail` | string | no | `"stop"` (cycle) / `"skip"` (phase) | Action when the command exits non-zero: `"skip"`, `"stop"`, or `"error"`. |
| `timeout` | duration | no | `"30s"` | Maximum time the gate command may run. Accepts integer seconds or duration string. |

**`on_fail` actions:**

| Action | Behavior |
|--------|----------|
| `skip` | **Phase gate:** skip this phase, continue to the next phase in the cycle. **Cycle gate:** skip all phases this cycle, apply `delay_between_cycles`, then start the next cycle (useful for polling patterns). |
| `stop` | Graceful exit: save state and exit with code 0 (same as completion signal). |
| `error` | Hard exit: save state and exit with code 2 (configuration error). |

**Execution details:**

- Gate commands run in `context_dir` with the same environment as the executor.
- Gate commands inherit the process's stdin/stdout/stderr. Stdout and stderr are captured in the run log.
- If a gate command exceeds its timeout, it is killed (SIGTERM → 5s → SIGKILL) and treated as a failure.
- Gate evaluation is logged: the command, exit code, and resulting action are recorded in both human and JSONL output.

### Cycle Gate

`cycle_gate` is set on the `[workflow]` table. It runs once at the top of each cycle, before any phases execute. If it fails, no phases in that cycle run.

Default `on_fail`: **`stop`** — the most common intent is "stop iterating when the precondition no longer holds."

```toml
# Stop planning when TODO.md is big enough
[workflow]
completion_signal = "PLANNING_COMPLETE"
context_dir = "."
cycle_gate = { command = "test $(wc -l < TODO.md) -lt 50", on_fail = "stop" }
```

With `on_fail = "skip"` and `delay_between_cycles`, the cycle gate becomes a polling gate — rings waits for the condition to become true before running phases:

```toml
# Only run phases when there's uncommitted work; poll every 5 minutes
[workflow]
completion_signal = "DONE"
context_dir = "."
delay_between_cycles = "5m"
cycle_gate = { command = "git diff --quiet HEAD && exit 1 || exit 0", on_fail = "skip" }
```

### Phase Gate

`gate` is set on a `[[phases]]` entry. It runs once before the phase's first run in each cycle (not before every individual run within `runs_per_cycle`). If it fails, the entire phase is skipped for that cycle and execution advances to the next phase.

Default `on_fail`: **`skip`** — the most common intent is "conditionally include this phase."

```toml
[[phases]]
name = "reviewer"
prompt = "./prompts/reviewer.md"
gate = { command = "test -f NEEDS_REVIEW.md", on_fail = "skip" }
```

To run the gate before every individual run (not just the first), set `gate_each_run = true` on the phase:

```toml
[[phases]]
name = "builder"
prompt = "./prompts/builder.md"
runs_per_cycle = 5
gate = { command = "test $(wc -l < TODO.md) -lt 50" }
gate_each_run = true
```

### Gate Logging

Human output:
```
[cycle 3] cycle gate: `test $(wc -l < TODO.md) -lt 50` → exit 1 (fail → stop)
[cycle 3] phase "reviewer" gate: `test -f NEEDS_REVIEW.md` → exit 0 (pass)
```

JSONL output emits a `gate_result` event:
```json
{"event":"gate_result","run_id":"...","timestamp":"...","scope":"cycle","phase":null,"command":"test $(wc -l < TODO.md) -lt 50","exit_code":1,"passed":false,"action":"stop"}
{"event":"gate_result","run_id":"...","timestamp":"...","scope":"phase","phase":"reviewer","command":"test -f NEEDS_REVIEW.md","exit_code":0,"passed":true,"action":null}
```

### Structural Fingerprint

Gate configuration is included in the workflow's structural fingerprint. Changing a gate command, `on_fail` action, or adding/removing a gate constitutes a structural change that prevents resume with the old state.

## Completion Signal Warning

At startup, rings reads all prompt sources (both file-referenced and inline) and searches for the `completion_signal` string. If the signal does not appear in any prompt, rings prints a warning:

```
Warning: completion_signal "TASK_COMPLETE" was not found in any prompt file.
Without this signal in a prompt, Claude Code cannot signal completion and the
workflow will run until max_cycles is reached or you cancel manually.
Continue? [y/N]
```

The user may suppress this warning with `--no-completion-check`.

**Non-TTY behavior:** If rings's stdin is not a TTY (e.g., running in a CI pipeline or with input redirected), the interactive `Continue? [y/N]` prompt is skipped and rings defaults to **proceeding** (equivalent to answering `y`). This prevents blocking pipelines. A warning is still printed to stderr. Users who want the opposite behavior (fail fast in CI when the signal is missing) should use `--no-completion-check` to suppress the check entirely, or validate their prompts before running.

## Minimal Example

```toml
[workflow]
completion_signal = "RINGS_DONE"
context_dir = "."

[[phases]]
name = "builder"
prompt = "prompts/build.md"
```

## Pattern Reference

| Pattern     | Configuration                                  |
|-------------|------------------------------------------------|
| `ABABAB`    | both phases: `runs_per_cycle = 1`              |
| `AAABAAAB`  | builder: `runs_per_cycle = 3`, reviewer: `runs_per_cycle = 1` |
| `AAAAAABBBB` | `max_cycles = 1`, builder: `runs_per_cycle = 6`, reviewer: `runs_per_cycle = 4` |
