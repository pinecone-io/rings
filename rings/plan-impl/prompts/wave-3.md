You are coordinating implementation review wave 3 of 5 for the plan-impl workflow.

## Setup

Read `queues/PLAN.md`. Find the `## Selected Features` section and extract the list of features (F-NNN, name, spec file).

For each selected feature, read its spec file in full. This context will be passed to each reviewer.

## Dispatch

Using the Agent tool, launch the following 3 agents **in a single message with 3 parallel tool calls**. Give each the prompt below, substituting the actual feature list.

Agents: `impl-serialization`, `impl-process-mgmt`, `impl-filesystem`

---
*"You are a member of the rings implementation review panel. Before reviewing, orient yourself:*
- *Read `specs/index.md` and `specs/overview.md` to understand what rings is*
- *Read the relevant source files in `src/` that relate to your area of expertise*
- *Read the spec files listed below for the selected features*

*The following features have been selected for the next implementation batch. Do a full implementation review from your area of expertise. For each feature, identify concerns, risks, design decisions, and anything that must be resolved before coding begins.*

*Selected features and their specs:*
*[list F-NNN · Name · spec file for each]*

*Return numbered findings with severity (nit / concern / blocker) and concrete suggestions."*

---

## Record findings

Append to `queues/PLAN.md` under `## Implementation Review Findings`:

```markdown
### Wave 3 — Serialization · Process Mgmt · Filesystem

#### impl-serialization
[findings]

#### impl-process-mgmt
[findings]

#### impl-filesystem
[findings]
```
