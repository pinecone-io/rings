# Executor Integration

## What Is an Executor?

An executor is any subprocess that rings can invoke to run a phase. rings passes the prompt via stdin, captures stdout+stderr, and scans the output for completion signals, cost information, and errors.

**Requirements for a compatible executor:**
- Accepts a text prompt on stdin
- Runs to completion and exits (non-interactive)
- Writes output to stdout or stderr (or both — rings captures both)
- Exits with code `0` on success, non-zero on failure

rings ships with a built-in default executor profile for Claude Code. Any other tool that satisfies the interface above can be used.

## Default Executor: Claude Code

The built-in default invocation is:

```bash
claude --dangerously-skip-permissions -p -
```

This is equivalent to specifying in the workflow file:

```toml
[executor]
binary = "claude"
args = ["--dangerously-skip-permissions", "-p", "-"]
cost_parser = "claude-code"
error_profile = "claude-code"
resume_pattern = 'claude resume (?P<id>\S+)'
```

No `[executor]` block is required to use Claude Code — this configuration is applied automatically when the section is absent.

## Prompt Delivery

Prompts are passed via **stdin**, not as a command-line argument. This applies to any executor.

**Security rationale:** Passing prompt text as a command-line argument would expose it in `ps aux` output, readable by any user with access to the process list. Passing via stdin keeps prompt contents out of the process table entirely.

- The subprocess is spawned with its working directory set to `context_dir` from the workflow file.
- `stdin` is a pipe that rings writes the prompt to, then closes.
- `stdout` and `stderr` are both captured by rings.

## Additional Context (--include-dir)

If `--include-dir` is provided on the rings command line, rings prepends a context preamble to the prompt before writing it to the executor's stdin:

```
The following context files are available for reference:
- /path/to/specs/file1.md
- /path/to/specs/file2.md

<original prompt contents>
```

`--include-dir` may be specified multiple times. This is prompt-level injection handled entirely by rings before the executor sees the prompt. It is compatible with any executor.

## Output Handling

The executor's combined stdout+stderr is:

1. Buffered and scanned for the completion signal after each run completes.
2. Scanned for cost information using the configured `cost_parser` profile.
3. Scanned for resume commands using the configured `resume_pattern`.
4. Written to `output_dir/<run-id>/runs/<run-number>.log`.
5. If `--verbose` flag is set, also streamed live to the terminal.

Without `--verbose`, the terminal shows only rings's own status display. Executor output is suppressed from the terminal but always captured to logs.

## Executor Binary Check

At startup, rings checks that the configured executor binary is available:

```
$ which <binary>
```

If not found, rings exits immediately with:

```
Error: '<binary>' not found on PATH.
Check that the executor is installed and available in your shell environment.
```

For the default Claude Code executor, the error additionally includes a reference to the installation docs.

## Environment Variables

rings passes through the current shell environment to the executor subprocess without modification. This ensures:
- API keys and credentials are available when needed
- Any executor-specific configuration environment variables work normally
- Shell integrations are preserved

## Executor Version Tracking

If the executor outputs a recognizable version string, rings attempts to capture it and stores it in `run.toml` as `executor_version`. This is best-effort and relies on the executor printing its version in a detectable format. When a parse failure occurs, the stored version aids in correlating output format changes with specific executor releases.

## Per-Phase Executors

Individual phases can override the workflow-level executor. This enables mixed-executor workflows where different phases use different tools:

```toml
[executor]
binary = "claude"
args = ["--dangerously-skip-permissions", "-p", "-"]

[[phases]]
name = "builder"
prompt = "./prompts/builder.md"
# inherits [executor] defaults

[[phases]]
name = "reviewer"
prompt = "./prompts/reviewer.md"
executor.binary = "gemini"
executor.args = ["--prompt", "-"]
executor.cost_parser = "none"
executor.error_profile = "none"
```

Per-phase executor fields inherit from `[executor]` for any fields not explicitly overridden.

## Per-Phase Model Selection via `executor.extra_args` (F-181)

**Prerequisites:** F-023 (Per-Phase Executors), F-066 (Default Executor Config)

When different workflow phases benefit from different model tiers — for example, using a cheaper model for triage and a more capable model for synthesis — users can route individual phases to specific models using `executor.extra_args`.

`executor.extra_args` is a list of arguments that rings **appends** to the inherited `executor.args` for that phase. This avoids the footgun of full `args` replacement: users do not need to re-specify base flags like `--dangerously-skip-permissions -p -` everywhere they want to change the model. The append semantics also keep the mechanism executor-agnostic — users supply whatever flag their executor accepts for model selection.

```toml
[executor]
binary = "claude"
args = ["--dangerously-skip-permissions", "-p", "-"]

[[phases]]
name = "triage"
prompt = "./prompts/triage.md"
# Uses workflow default — no extra_args needed

[[phases]]
name = "synthesis"
prompt = "./prompts/synthesis.md"
executor.extra_args = ["--model", "claude-opus-4-6"]
# Effective args: ["--dangerously-skip-permissions", "-p", "-", "--model", "claude-opus-4-6"]
```

Per-phase `executor.extra_args` inherits from the workflow-level `[executor]` `extra_args` if defined, following the same override semantics as other per-phase executor fields. An `extra_args` value at the phase level **replaces** the workflow-level `extra_args` for that phase; it does not append again.

### Audit Log Requirements

The effective model per run must be recorded in both `run.toml` (F-105) and the `costs.jsonl` entry (F-107) for every run. Without this, the cost comparison between phases using different models is meaningless — you can see that phase A cost $0.02 and phase B cost $0.80, but not whether the difference is model tier or output length. The effective model value is the resolved model string after per-phase inheritance, extracted from the effective args where recognizable.

### Startup Validation

At startup, rings validates that `executor.extra_args` does not contain `--model` if `--model` is already present in the effective `executor.args` for that phase. A conflicting double `--model` flag produces a configuration error (exit code 2) rather than silently passing two flags to the executor.

### Completion Signal Reliability in Mixed-Model Workflows

Cheaper models are less reliably at emitting exact completion signal strings verbatim. When using a mixed-model workflow, it is recommended to use `completion_signal_phases` (F-013) to restrict completion detection to phases running capable models. This prevents a cheap model's reformulation of the signal from inadvertently triggering or suppressing workflow completion.

### Workflow Portability

Model availability varies by account tier. When the executor exits non-zero because a model string is unrecognized or unavailable, rings surfaces the executor's original error message clearly rather than a generic "executor exited non-zero", so the root cause is actionable.

### Interaction with Workflow File Change Detection

When F-050 (Workflow File Change Detection) is implemented, `executor.extra_args` and all executor fields must be treated as structurally significant. Changing the model between a run and its resume produces materially different output while the audit trail appears to show continuity. Resuming after any executor field change must require explicit acknowledgment or be blocked.
