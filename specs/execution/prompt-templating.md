# Prompt Templating

## Overview

Prompt texts — whether inline (`prompt_text`) or loaded from a file (`prompt`) — can embed template variables. Before each run, rings substitutes these variables with values from the current execution context. This allows prompts to be self-aware: a prompt can tell the model which cycle it is on, how many iterations remain, and how much has been spent so far.

Template variables use double-curly-brace syntax: `{{variable_name}}`.

## Available Variables

| Variable | Type | Description |
|----------|------|-------------|
| `{{phase_name}}` | string | Name of the current phase (e.g. `builder`) |
| `{{cycle}}` | integer | Current cycle number, 1-indexed (e.g. `3`) |
| `{{max_cycles}}` | integer | Configured `max_cycles`. If unbounded, the string `"unlimited"` |
| `{{iteration}}` | integer | Current iteration within the cycle, 1-indexed (e.g. `2`) |
| `{{runs_per_cycle}}` | integer | Total iterations configured for this phase (e.g. `3`) |
| `{{run}}` | integer | Global run number across all phases and cycles, 1-indexed (e.g. `7`) |
| `{{cost_so_far_usd}}` | decimal | Cumulative cost in USD at the start of this run (e.g. `1.47`). `0.00` on the first run. |

## Example Usage

### Inline prompt with context header

```toml
[[phases]]
name = "builder"
runs_per_cycle = 3
prompt_text = """
# Builder — Cycle {{cycle}}/{{max_cycles}}, Iteration {{iteration}}/{{runs_per_cycle}}

You are a Rust engineer building the feature described in `specs/`.
This is global run {{run}}. Total cost so far: ${{cost_so_far_usd}}.

Review what was written in previous iterations by reading the current source files.
Continue where the previous iteration left off.

When the feature is complete and tests pass, print: TASK_COMPLETE
"""
```

### Prompt file with iteration context

```markdown
<!-- prompts/builder.md -->
# Builder Agent

**Session context:** Phase `{{phase_name}}`, cycle {{cycle}} of {{max_cycles}},
iteration {{iteration}} of {{runs_per_cycle}} (global run {{run}}).
Cumulative spend: ${{cost_so_far_usd}}.

You are building the feature described in `specs/`. Read the current state of `src/`
to understand what has been done. Continue from where the previous iteration left off.

[... rest of prompt ...]

When complete, print: TASK_COMPLETE
```

## Interpolation Behavior

- Substitution happens at **prompt-build time**, immediately before the subprocess is spawned for that run.
- For `prompt` (file references), the file is re-read on each run and then variables are substituted. Modifying the prompt file between runs takes effect on the next run.
- For `prompt_text` (inline), the TOML value is read once at startup but substitution is applied fresh each run with the current context values.
- Substitution is **pure string replacement** — there are no conditionals, loops, or expressions.

## Unknown Variables

An unrecognized variable name (e.g., `{{typo}}` or `{{custom_var}}`) produces a startup advisory warning:

```
Warning: Unknown template variable '{{typo}}' in prompt for phase "builder".
It will be passed through as-is (not substituted).
```

Unknown variables are **not** substituted — the literal text `{{typo}}` appears in the rendered prompt. Execution is not halted.

## Escaping

To include a literal `{{` in a prompt (i.e., to prevent substitution), escape it as `{{{{`:

```
{{{{phase_name}}}}  →  renders as  {{phase_name}}  (not substituted)
```

This is rarely needed but available for prompts that generate template-like output (e.g., generating Terraform or Helm templates).

## Audit Log Recording

The rendered (post-substitution) prompt is **not** stored in audit logs — only the template source is stored. However, the substitution context used for each run is recorded in the `run_start` JSONL event as `template_context`:

```jsonl
{
  "event": "run_start",
  "run_id": "run_...",
  "run": 7,
  "cycle": 3,
  "phase": "builder",
  "iteration": 2,
  "total_iterations": 3,
  "template_context": {
    "phase_name": "builder",
    "cycle": 3,
    "max_cycles": 10,
    "iteration": 2,
    "runs_per_cycle": 3,
    "run": 7,
    "cost_so_far_usd": 1.47
  },
  "timestamp": "..."
}
```

This allows a reader of audit logs to reconstruct exactly what context a given run received, without storing the full rendered prompt.

## What Templating is NOT

- **No access to file contents.** Variables cannot read files from `context_dir` or the filesystem. Use the prompt itself (or `--include-dir`) to pass file contents to Claude Code.
- **No environment variables.** `{{env.MY_VAR}}` is not supported — environment pass-through happens at the subprocess level, not in prompt text.
- **No expressions or logic.** `{{cycle * 2}}` is not valid. If a prompt needs conditional text based on cycle number, the user must structure that logic in how Claude Code interprets the context values.
- **No custom variables.** Only the built-in variables listed above are supported. User-defined variables are treated as unknown and passed through as-is with a warning.

## Design Rationale

The primary use cases for prompt templating are:

1. **Orientation** — helping the model understand where it is in the workflow (cycle 3 of 10, iteration 2 of 3) so it can calibrate its effort and approach appropriately.
2. **Progress awareness** — showing cost-so-far gives the model (and the user reading verbose output) a sense of how much work has been invested.
3. **Reproducibility** — the `template_context` in audit logs makes runs fully reproducible in documentation and debugging.

The feature deliberately avoids Turing-completeness (no conditionals, no loops). Prompts are documents, not programs. Complex logic belongs in the workflow structure (phases, cycles) not in prompt text.
