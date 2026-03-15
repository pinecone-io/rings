# Workflow — Section Index

← [specs/index.md](../index.md)

Defines what rings runs: the TOML workflow file schema, how cycles and runs are sequenced, and phase input/output file contracts.

## Files

| File | Contents |
|------|----------|
| [workflow-file-format.md](workflow-file-format.md) | Complete TOML schema, all fields, validation rules, prompt sourcing options |
| [cycle-model.md](cycle-model.md) | Run/cycle/workflow definitions, execution order, completion checks, termination conditions |
| [phase-contracts.md](phase-contracts.md) | `consumes`/`produces` declarations for file lineage tracking and data-flow documentation |

## Related

- [Execution → engine.md](../execution/index.md) — how the engine processes a workflow at runtime
- [Execution → prompt-templating.md](../execution/prompt-templating.md) — template variables available inside phase prompts
- [Execution → completion-detection.md](../execution/completion-detection.md) — how `completion_signal` values are matched against run output
