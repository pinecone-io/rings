You are coordinating implementation review wave 5 of 5 for the plan-impl workflow.

## Setup

Read `queues/PLAN.md`. Find the `## Selected Features` section and extract the list of features (F-NNN, name, spec file).

For each selected feature, read its spec file in full. This context will be passed to each reviewer.

## Dispatch

Using the Agent tool, launch the following 3 agents **in a single message with 3 parallel tool calls**. Give each the prompt below, substituting the actual feature list.

Agents: `impl-regex`, `impl-agent-ux`, `impl-docs`

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
### Wave 5 — Regex · Agent UX · Docs

#### impl-regex
[findings]

#### impl-agent-ux
[findings]

#### impl-docs
[findings]
```

## Synthesize

Read the complete `## Implementation Review Findings` section in `queues/PLAN.md` — all five waves. Group findings across all reviewers into:

- **Blockers** — must be resolved before implementation begins
- **Open Decisions** — explicit choices to make, each with a recommended default
- **Test Requirements** — specific cases called out by reviewers
- **Spec Gaps** — ambiguities that would affect implementation
- **Discarded Concerns** — inapplicable findings, with brief rationale

Append to `queues/PLAN.md`:

```markdown
## Open Decisions
[Each decision as a question, with recommended answer and tradeoffs]

## Spec Gaps
[Ambiguities to resolve before or during implementation]

## Implementation Steps
[For each feature:
  - Source files to create or modify
  - Key types, structs, or traits to add
  - Test cases required (unit and integration)
  - Any cross-feature dependencies in this batch]
```

Then print the following on its own line to signal completion:

PLAN_IMPL_DONE
