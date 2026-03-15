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
