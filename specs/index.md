# rings — Specification Index

`rings` orchestrates iterative AI prompt workflows ("Ralph loops") using Claude Code. Runs repeat across phases and cycles until a completion signal is detected, tracking cost and state throughout. All model interaction is delegated to the `claude` subprocess; rings handles only orchestration, state, and observability.

→ [Product overview, design principles, and target user](overview.md)
→ [MVP scope — minimum to build first](mvp.md)
→ [Feature inventory — numbered list of every feature with one-line summaries and spec links](feature_inventory.md)

## Core Concepts

| Term | Definition |
|------|-----------|
| **Run** | Single `claude` invocation for one phase — the atomic unit of execution |
| **Phase** | A named prompt with `runs_per_cycle` and an optional `completion_signal` |
| **Cycle** | One full pass through all phases in declaration order |
| **Workflow** | Complete execution: repeated cycles until signal detected or `max_cycles` reached |

State is shared between runs implicitly through the filesystem (`context_dir`). rings never passes output between phases directly.

## Sections

### [Workflow](workflow/index.md)
Defining what runs: TOML workflow file format, the cycle/run execution model, and phase file contracts.

### [Execution](execution/index.md)
How runs happen: the engine loop, Claude Code subprocess integration, prompt templating, output parsing, rate limiting, and error handling.

### [CLI](cli/index.md)
All user-facing commands and flags, exit codes, the `inspect` deep-dive tool, shell completions, and binary distribution.

### [State](state/index.md)
Configuration loading and precedence, safe cancellation and resume, and run ancestry tracking.

### [Observability](observability/index.md)
Runtime terminal output, structured audit logs, cost tracking, file lineage snapshots, and optional OpenTelemetry integration.
