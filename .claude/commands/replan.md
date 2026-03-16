Review the feature inventory and produce a detailed, implementation-hardened plan for the next batch of features.

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

### 3. Dispatch implementation review panel in parallel

Using the Agent tool, launch ALL of the following agents simultaneously, giving each the same task description: "Review the following candidate features for implementation planning. For each feature, identify implementation concerns, risks, design decisions, and anything that should be resolved before coding begins. Candidate features: [list the F-NNN numbers and names]. Relevant spec files: [list the spec files]. Focus on your area of expertise."

Agents to dispatch in parallel:
- `impl-rust`
- `impl-architecture`
- `impl-deps`
- `impl-testing`
- `impl-error-handling`
- `impl-cli-framework`
- `impl-serialization`
- `impl-process-mgmt`
- `impl-filesystem`
- `impl-cross-platform`
- `impl-performance`
- `impl-memory`
- `impl-regex`
- `impl-agent-ux`
- `impl-docs`

### 4. Synthesize findings

Read all 15 review outputs. Group findings by theme. Identify:
- **Blockers** — design issues that must be resolved before implementation begins
- **Implementation decisions** — choices that need to be made explicitly (with a recommended default)
- **Test requirements** — specific test cases called out by the testing reviewer
- **Spec clarifications needed** — ambiguities in the spec that would affect implementation
- **Discarded concerns** — findings that don't apply or are out of scope, with brief rationale

### 5. Produce PLAN.md

Write a `PLAN.md` at the project root with the following structure:

```markdown
# Implementation Plan — [date]

## Selected Features
[Numbered list of F-NNN features being planned, with one-line summaries]

## Implementation Review Findings
[Synthesized findings grouped by theme, with reviewer attribution]

## Open Decisions
[Explicit choices that must be made before coding, with a recommended option]

## Spec Clarifications Needed
[Ambiguities to resolve, or confirm as implementation decisions]

## Implementation Steps
[For each feature: source files to touch, types/structs to add, test cases required]
```

Do not mark any feature as `PLANNED` in the inventory or begin implementation until the user has reviewed and approved `PLAN.md`.
