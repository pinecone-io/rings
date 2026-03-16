Review the feature inventory and produce a detailed, review-hardened implementation plan for the next batch of features.

## Steps

### 1. Orient yourself

Read the following to understand current state:
- `specs/feature_inventory.md` — full feature list with statuses; identify all `BACKLOG` features whose prerequisites are all `COMPLETE`
- `specs/mvp.md` — original scope priorities
- `specs/index.md` — spec tree overview

### 2. Select candidate features

From the unblocked `BACKLOG` features, identify a candidate batch of 5–10 that are:
- High user value relative to implementation complexity
- Logically grouped (same spec file or implementation surface)
- Well-specified enough to implement now

For each candidate, read its linked spec file in full.

### 3. Dispatch review panel in parallel

Using the Agent tool, launch ALL of the following agents simultaneously, giving each the same task description: "Review the following candidate features for the next implementation batch and provide your perspective. Candidate features: [list the F-NNN numbers and names]. Relevant spec files: [list the spec files]. Focus on your area of expertise and identify concerns, gaps, or improvements before we commit to building these."

Agents to dispatch in parallel:
- `review-cli`
- `review-devops`
- `review-data-eng`
- `review-ai-newcomer`
- `review-gen-z`
- `review-security`
- `review-token-opt`
- `review-reliability`
- `review-scripter`
- `review-oss`
- `review-founder`
- `review-prompt-eng`
- `review-enterprise`
- `review-docs`
- `review-agent-ux`

### 4. Synthesize findings

Read all 13 review outputs. Group findings by theme. Identify:
- **Blockers** — things that should change before building (spec gaps, design issues)
- **Spec amendments** — clarifications or additions needed in spec files before implementing
- **Implementation notes** — concerns to carry forward into code (not spec changes)
- **Discarded concerns** — findings that don't apply or are out of scope, with brief rationale

### 5. Produce PLAN.md

Write a `PLAN.md` at the project root with the following structure:

```markdown
# Implementation Plan — [date]

## Selected Features
[Numbered list of F-NNN features being planned, with one-line summaries]

## Review Panel Findings
[Synthesized findings grouped by theme, with reviewer attribution]

## Spec Amendments Required
[Any spec changes that should happen before implementation begins]

## Implementation Steps
[For each feature: source files to touch, types/structs to add, test cases required]

## Open Questions
[Anything requiring human decision before work begins]
```

Do not mark any feature as `PLANNED` in the inventory or begin implementation until the user has reviewed and approved `PLAN.md`.
