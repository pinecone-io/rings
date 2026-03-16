You are coordinating the initial technical draft for the plan-create workflow.

## Phase identity

You are the `draft` phase of `plan-create`.

## Setup

Read `queues/SELECTED_FEATURES.md`. Find the batch header (`## Batch: ...`) and extract the feature table (F-NNN, feature name, spec file path).

For each selected feature, read its spec file in full.

If `queues/SELECTED_FEATURES.md` is empty or contains no batch header, stop immediately and print:

```
PLAN_DRAFT_DONE
```

## Dispatch

Using the Agent tool, launch the following 3 agents **in a single message with 3 parallel tool calls**. Give each the prompt below, substituting the actual feature list and spec content.

Agents: `impl-rust`, `impl-architecture`, `impl-deps`

---
*"You are a member of the rings initial planning panel. Before reviewing, orient yourself:*
- *Read `specs/index.md` and `specs/overview.md` to understand what rings is*
- *Read the relevant source files in `src/` that relate to your area of expertise*
- *Read the spec files listed below for the selected features*

*The following features have been selected for the next implementation batch. Produce an initial technical plan from your area of expertise. For each feature, identify: source files to create or modify, key types/traits/structs to introduce, test cases required (unit and integration), and any cross-feature dependencies.*

*Selected features and their specs:*
*[list F-NNN · Name · spec file for each]*

*Return your findings as structured markdown."*

---

## Record draft

Collect the three agents' outputs. Append the following to `queues/PLAN_DRAFTS.md`:

```markdown
## [DRAFT] Batch: <batch name from SELECTED_FEATURES.md> — <today's date>

### Selected Features

| F-NNN | Feature | Spec file |
|-------|---------|-----------|
[feature table rows]

### Source Files

[Merged list of source files to create or modify, deduplicated across all three agents]

### Key Types, Traits, and Structs

[Merged list of types/traits/structs to introduce, with brief purpose for each]

### Test Cases Required

[Merged list of unit and integration test cases called out by all three agents]

### Cross-Feature Dependencies

[Any dependencies between features in this batch, e.g. F-020 depends on F-054's RunHandle]
```

Then print the following on its own line to signal completion:

```
PLAN_DRAFT_DONE
```
