# Plan Review Refactor Design
**Date:** 2026-03-16

## Summary

Replace the `plan-impl` workflow (5 sequential waves of parallel coordinator agents) with two focused workflows: `plan-create` and `plan-review`. The refactor eliminates prompt duplication, fixes context exhaustion by making rings itself the iterator, and replaces the overloaded `queues/PLAN.md` with a clean three-stage queue progression.

## Motivation

The existing `plan-impl` workflow has three problems:

1. **Wave structure worked around context limits** — 5 coordinator agents each dispatching 3 sub-agents was the workaround for context exhaustion, not a principled design.
2. **Prompt duplication** — all 5 wave prompts are ~90% identical boilerplate.
3. **PLAN.md does too much** — vote tally, selected features, review findings, and synthesis all accumulate in one file, making it large and hard to reason about.

## File Organization Conventions

- **`queues/`** — files intended to be consumed by other workflows; entries are appended and processed in order
- **`rings/<workflow-name>/wip/`** — ephemeral state internal to a workflow's cycles; cleaned up after use and never committed

All `wip/` paths in this doc are relative to `rings/plan-review/wip/`.

## Queue Progression

```
queues/SELECTED_FEATURES.md
         │
         ▼ (plan-create)
queues/PLAN_DRAFTS.md
         │
         ▼ (plan-review)
queues/READY_TO_IMPLEMENT.md
```

### `queues/SELECTED_FEATURES.md`

Input to `plan-create`. Contains the current batch of selected features. Written by the feature-election workflow (which currently writes `queues/PLAN.md` — updating feature-election to write `queues/SELECTED_FEATURES.md` instead is a required companion change outside the scope of this spec).

Format:

```markdown
## Batch: <batch name> — <date>

| F-NNN | Feature | Spec file |
|-------|---------|-----------|
| F-020 | Timeout Per Run | specs/timeout.md |
| F-055 | Context Directory Lock | specs/locking.md |
```

One batch per file (the file is overwritten each election run, not appended).

### `queues/PLAN_DRAFTS.md`

Output of `plan-create`, input to `plan-review`. Each entry is a structured plan for one batch.

Each entry begins with a status line:

```
## [DRAFT] Batch: <batch name> — <date>
```

Sections within each entry:

- Selected features (F-NNN, name, spec file)
- Source files to create or modify
- Key types, traits, and structs to introduce
- Test cases required (unit and integration)
- Cross-feature dependencies within the batch

When `plan-review` finishes synthesizing an entry, it updates the status marker in-place:

```
## [REVIEWED] Batch: <batch name> — <date>
```

`plan-review` processes the first entry whose status line begins `## [DRAFT]`.

### `queues/READY_TO_IMPLEMENT.md`

Output of `plan-review`. Each entry is the synthesized, reviewer-hardened implementation plan for one batch. Format:

```
## Batch: <batch name> — <date>

### Blockers
[Must-resolve items before coding begins, with recommended resolutions]

### Open Decisions
[Explicit choices with recommended defaults and tradeoffs]

### Test Requirements
[Specific test cases called out by reviewers]

### Spec Gaps
[Ambiguities to resolve during implementation]

### Implementation Steps
[Per feature: source files, key types/traits, test cases, cross-feature dependencies]
```

## Workflow 1 — `plan-create`

**Location:** `rings/plan-create/plan-create.rings.toml`
**Input:** `queues/SELECTED_FEATURES.md`
**Output:** `queues/PLAN_DRAFTS.md`
**Shape:** 1 phase, `max_cycles = 1`

Key TOML fields:
```toml
completion_signal = "PLAN_DRAFT_DONE"
completion_signal_mode = "line"
completion_signal_phases = ["draft"]
context_dir = "."

[executor]
binary = "claude"
args = ["--dangerously-skip-permissions", "-p", "-"]
```

### Phase: `draft`

Dispatches 3 core technical agents **in parallel** (single message, 3 tool calls):

| Agent | Focus |
|---|---|
| `impl-rust` | Idiomatic Rust patterns, trait design, ownership/lifetimes |
| `impl-architecture` | Module structure, separation of concerns, extensibility |
| `impl-deps` | Crate selection, dependency justification, feature flags |

Each agent reads `queues/SELECTED_FEATURES.md` and the relevant spec files. The coordinator collects their outputs and appends a structured `[DRAFT]` entry to `queues/PLAN_DRAFTS.md`, then emits `PLAN_DRAFT_DONE`.

## Workflow 2 — `plan-review`

**Location:** `rings/plan-review/plan-review.rings.toml`
**Input:** `queues/PLAN_DRAFTS.md` (first `[DRAFT]` entry)
**Output:** `queues/READY_TO_IMPLEMENT.md`
**Shape:** 2 phases, `max_cycles = 12`

Key TOML fields:
```toml
completion_signal = "PLAN_REVIEW_DONE"
completion_signal_mode = "line"
completion_signal_phases = ["synthesize"]
continue_signal = "RINGS_CONTINUE"
context_dir = "."
max_cycles = 12

[executor]
binary = "claude"
args = ["--dangerously-skip-permissions", "-p", "-"]
```

`continue_signal = "RINGS_CONTINUE"` is required. When the `review` phase emits `RINGS_CONTINUE`, rings skips the remaining phases in the current cycle (i.e., `synthesize`) and begins the next cycle. `synthesize` is only reached when `review` completes without emitting `RINGS_CONTINUE` — that is, when all reviewers have run.

**Note:** `continue_signal` is not yet documented in `specs/workflow/`. The `process-ideas` workflow uses this field in production (`rings/process-ideas/process-ideas.rings.toml`), confirming the behavior. A spec for this field should be filed as a follow-on task.

**Note:** `max_cycles` must equal the reviewer roster size. If reviewers are added or removed, `max_cycles` must be updated to match.

### Reviewer roster (12 agents)

`impl-testing`, `impl-error-handling`, `impl-cli-framework`, `impl-serialization`, `impl-process-mgmt`, `impl-filesystem`, `impl-cross-platform`, `impl-performance`, `impl-memory`, `impl-regex`, `impl-agent-ux`, `impl-docs`

### Phase: `review`

Each cycle:

1. **Debris cleanup (first cycle only):** If zero `wip/review-*.md` files exist, delete any other stale files in `rings/plan-review/wip/` before proceeding. This handles leftover state from a previously interrupted run.
2. Check `rings/plan-review/wip/` for existing `review-{persona}.md` files. Any existing file is treated as complete — this supports safe resume after interruption.
3. Determine the next reviewer: iterate the roster in order, pick the first whose `review-{persona}.md` does not exist in `rings/plan-review/wip/`.
4. Run that reviewer agent against the current `[DRAFT]` entry in `queues/PLAN_DRAFTS.md` and the relevant spec files.
5. Write findings to `rings/plan-review/wip/review-{persona}.md`.
6. If any reviewers remain in the roster: emit `RINGS_CONTINUE`.
7. If all 12 reviewers have run: do **not** emit `RINGS_CONTINUE` — the cycle falls through to `synthesize`.

### Phase: `synthesize`

Runs once, after all reviewers have completed:

1. Read all `rings/plan-review/wip/review-*.md` files.
2. Group findings into: blockers, open decisions, test requirements, spec gaps, nits.
3. Produce the final plan in the format defined by `queues/READY_TO_IMPLEMENT.md` above.
4. Append the final plan to `queues/READY_TO_IMPLEMENT.md`.
5. Update the processed entry's status in `queues/PLAN_DRAFTS.md` from `[DRAFT]` to `[REVIEWED]`.
6. Delete all files in `rings/plan-review/wip/`.
7. Emit `PLAN_REVIEW_DONE`. This is the `completion_signal` and terminates the workflow immediately.

## wip/ File Lifecycle

All wip files live in `rings/plan-review/wip/` (workflow-specific; not the repo root).

| File | Created by | Deleted by |
|---|---|---|
| `rings/plan-review/wip/review-{persona}.md` | `review` phase (one per cycle) | `synthesize` phase |

No wip files are written by `plan-create`.

## Deprecations

This is a clean-cut replacement. No migration of existing content is required. As part of this implementation:

- **Delete** `queues/PLAN.md`
- **Delete** `rings/plan-impl/` (entire directory)

These are replaced by `queues/SELECTED_FEATURES.md`, `queues/PLAN_DRAFTS.md`, `queues/READY_TO_IMPLEMENT.md`, `rings/plan-create/`, and `rings/plan-review/`.
