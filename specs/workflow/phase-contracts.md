# Phase Contracts

## Overview

Phases can optionally declare what files they consume and produce. These declarations are advisory in v1 — rings warns but does not fail when contracts are not satisfied. They serve two purposes:

1. **Documentation**: make data flow explicit in the workflow file for human readers
2. **Lineage enrichment**: enrich audit logs and OTel spans with declared vs actual file changes

## Syntax

```toml
[[phases]]
name = "builder"
prompt = "prompts/builder.md"
runs_per_cycle = 3

# Optional: glob patterns for files this phase reads as primary inputs.
# Used for lineage tracking and documentation. Not validated at runtime.
consumes = [
  "specs/**/*.md",
  "prompts/builder.md",
]

# Optional: glob patterns for files this phase is expected to produce or modify.
# rings warns after each run if none of the matched files were touched.
produces = [
  "src/**/*.rs",
  "tests/**/*.rs",
]

[[phases]]
name = "reviewer"
prompt = "prompts/reviewer.md"
runs_per_cycle = 1

consumes = ["src/**/*.rs", "tests/**/*.rs"]
produces = ["review-notes.md"]
```

## `consumes` Behavior

At startup, rings performs a **soft validation** of each `consumes` pattern:

1. **File presence check**: rings scans `context_dir` for files matching the pattern. If no existing files match AND the pattern does not appear as a substring in the phase's prompt text (whether from a file or inline), rings warns:

```
⚠  Phase "reviewer" declares consumes = ["review-notes.md"]
   but no matching files exist in context_dir ("./src")
   and the pattern is not mentioned in the prompt.
   This phase may silently do nothing if its expected inputs are never created.
   Suppress with --no-completion-check or fix the consumes declaration.
```

**Why both checks:** A pattern may not match any files yet because the workflow hasn't run — the file will be created by a prior phase on the first cycle. If the prompt mentions the filename, the user clearly intends this. If neither the file exists nor the prompt mentions it, this is likely a misconfiguration: a typo in the pattern, a wrong path, or a forgotten prompt instruction.

**When the warning fires at startup:**
- Pattern matches zero files in `context_dir` at the time rings starts, AND
- Pattern string (or its non-glob prefix) does not appear as a substring in the prompt text

**When the warning is suppressed:**
- Any file in `context_dir` matches the pattern (files exist; the phase will have inputs on this run), OR
- The pattern text appears in the prompt (user clearly knows about these files and references them), OR
- `--no-contract-check` is passed (suppresses contract warnings without affecting the completion signal check), OR
- `--no-completion-check` is passed (suppresses all advisory startup warnings including both completion signal and contract checks)

The startup warning is printed once, not repeated per-run.

**Per-run warning (cycle 2+):** On the second and subsequent cycles, if a `consumes` pattern still matches no files in `context_dir` immediately before a run starts, rings emits a per-run warning:

```
⚠  Phase "reviewer" (run 9, cycle 2): consumes = ["review-notes.md"]
   but no matching files found in context_dir. The phase may operate on missing inputs.
```

This is distinct from the startup warning: by cycle 2, any files a prior phase was supposed to produce should already exist. A missing file at this point suggests the producing phase failed silently or wrote to an unexpected location.

Execution continues regardless.

- Stored in audit log metadata for lineage reporting.
- Shown in `rings inspect` output under "declared data flow".
- Included as OTel span attribute `rings.phase.consumes`.

## `produces` Behavior

After each run, rings intersects the `produces` patterns against the file diff for that run:

- **Match found**: normal operation.
- **No match**: rings emits a warning to stderr:

```
⚠  Phase "builder" declared produces = ["src/**/*.rs", "tests/**/*.rs"]
   but no matching files were modified in run 7 (cycle 2, iteration 2/3).
   This may indicate the phase did not complete its intended work.
```

The warning is advisory only by default. Execution continues. The unmatched patterns are recorded in the `produces_violations` field of the `run_end` JSONL event (always present as an array, empty when all contracts are satisfied):

```jsonl
{"event":"run_end","run_id":"run_...","run":7,...,"produces_violations":["src/**/*.rs"],...}
```

`produces_violations` is an array of strings, each being a `produces` glob pattern that matched no changed files in that run. Empty array (`[]`) when all contracts are satisfied or when no `produces` contracts are declared.

`produces` validation requires file manifest tracking to be enabled (`manifest_enabled = true`, which is the default). When `manifest_enabled = false`, `produces` contract checks are silently skipped and `produces_violations` is always `[]`.

### `produces_required`

For phases where producing output files is non-negotiable, set `produces_required = true`:

```toml
[[phases]]
name = "builder"
produces = ["src/**/*.rs"]
produces_required = true  # exit code 2 if builder produces no matching files
```

When `produces_required = true`, a run that matches no `produces` patterns is treated as a hard error: rings records the run as failed, saves state, and exits with code 2. This makes the phase's contract enforceable rather than advisory.

`produces_required` defaults to `false`. It requires `manifest_enabled = true` — setting `produces_required = true` with manifests disabled is a validation error at startup.

## Lineage Graph Output

`rings inspect <run-id> --show data-flow` uses `consumes`/`produces` declarations to render the declared data flow through the workflow:

```
Data flow (declared):
  specs/**/*.md  ──→  [builder]  ──→  src/**/*.rs
                                      tests/**/*.rs
  src/**/*.rs   ──→  [reviewer] ──→  review-notes.md
  tests/**/*.rs ──→  [reviewer]
```

Combined with actual file change data from manifests, rings can also show:

```
Data flow (actual, cycle 2):
  src/main.rs    modified by builder (run 5)
  src/engine.rs  modified by builder (run 6, 7)
  review-notes.md  created by reviewer (run 8)
```

## Design Philosophy

Phase contracts are not a type system or enforcement mechanism — they are structured comments that happen to be machine-readable. The goal is to make workflow intent visible and to catch obvious misconfigurations (a phase that is supposed to produce files but consistently doesn't).

rings does not define or enforce how phases communicate with each other. If a reviewer phase needs to leave structured feedback for the builder, the user decides the file format and instructs both prompts accordingly. rings tracks that a file changed; what the file contains is out of scope.

Phase contracts are not a type system or enforcement mechanism by default — they are structured comments that happen to be machine-readable. For strict enforcement, use `produces_required = true` on individual phases.
