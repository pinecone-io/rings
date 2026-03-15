# Run Ancestry and Lineage Chain

## Overview

Each rings run records its relationship to prior runs. When a run is resumed, the new execution links back to the run it continues from. When a user starts a fresh run on the same workflow, they can optionally tag it as a continuation of previous work. This builds a navigable chain of runs through time.

## Ancestry Fields in `run.toml`

```toml
run_id = "run_20240315_150012_x9y8z7"
workflow_file = "/abs/path/to/my-task.rings.toml"
started_at = "2024-03-15T15:00:12Z"
rings_version = "0.1.0"
status = "running"

# Set when this run was created by `rings resume <prior-run-id>`
parent_run_id = "run_20240315_143022_a1b2c3"

# Set when the user explicitly declares ancestry with --parent-run
# (same as parent_run_id when using resume; may differ for manual continuations)
continuation_of = "run_20240315_143022_a1b2c3"

# Depth in the ancestry chain (root run = 0, first resume = 1, etc.)
ancestry_depth = 1
```

## How Ancestry Is Established

**Automatic (resume):**
When `rings resume <run-id>` creates a new run, `parent_run_id` is automatically set to the resumed run's ID.

**Manual (fresh run on same workflow):**
```bash
rings run workflow.toml --parent-run run_20240315_143022_a1b2c3
```
This starts a fresh execution but records the ancestry link. Useful when the user wants to start over from the beginning but maintain a chain showing the work history.

**No ancestry (default):**
A plain `rings run workflow.toml` without `--parent-run` creates a root run with `parent_run_id = null` and `ancestry_depth = 0`.

## Ancestry in OTel

When OTel is enabled, the resumed run's root span carries a **span link** pointing to the parent run's root span. Span links are the correct mechanism for cross-trace relationships — parent-child context is only valid within a single trace. Using a parent span from a different trace ID would violate the W3C trace context spec.

```
Trace A (run_20240315_143022_a1b2c3):
  Span: rings.run [root, span_id=aaaa]
    ...canceled after cycle 3...

Trace B (run_20240315_150012_x9y8z7):
  Span: rings.run [root, links=[{trace_id=Trace A, span_id=aaaa}]]
    attribute: rings.parent_run_id = "run_20240315_143022_a1b2c3"
    ...continues from cycle 4...
```

The span link enables "follow the chain" navigation in tools like Jaeger or Honeycomb that support linked traces. The `rings.parent_run_id` attribute is also set as a searchable string on the root span for backends that don't surface links prominently.

## Lineage Chain Traversal

The full chain for a run can be reconstructed by following `parent_run_id` links through the run metadata files stored in the output directory.

`rings lineage <run-id>` automates this (see inspect-command.md).

## Chain Metadata Summary

When displaying lineage, rings aggregates across the chain:

- Total cycles across all runs in the chain
- Total cost across the entire chain
- Total duration (wall time) across all runs
- Final completion status

This gives a true picture of "how much did this task actually cost?" — even when spread across multiple resume sessions.

## Storage

Ancestry fields are stored in `run.toml` (human-readable) and also in `state.json` under `"ancestry"`:

```json
{
  "ancestry": {
    "parent_run_id": "run_20240315_143022_a1b2c3",
    "continuation_of": "run_20240315_143022_a1b2c3",
    "ancestry_depth": 1
  }
}
```
