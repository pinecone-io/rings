Review the feature inventory, run a multi-persona voting round to select the highest-value batch, then produce a detailed implementation-hardened plan.

## Steps

### 1. Orient yourself

Read the following to understand current state:
- `specs/feature_inventory.md` — full feature list with statuses
- `specs/mvp.md` — original scope priorities
- `specs/index.md` — spec tree overview

Identify all `BACKLOG` features whose prerequisites are all `COMPLETE`. These are the candidates eligible for voting. Read the spec file for each candidate so you can summarize them accurately for the agents.

### 2. Voting round — dispatch all personas in parallel

Using the Agent tool, launch ALL of the following agents simultaneously. Give each the same task:

"You are being asked to vote on which features to implement next in the rings project. Below is the list of unblocked BACKLOG features — features whose prerequisites are already complete. Review the list from your area of expertise and cast your votes.

For each feature you vote for, explain in 1–2 sentences why it matters from your perspective. You may vote for as many features as you genuinely care about, but focus on the ones that would make the biggest difference from your area of concern.

Unblocked features:
[list each F-NNN, name, one-line summary, and spec file]

Return your votes as a simple list: F-NNN — reason."

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

### 3. Tally votes

Read all 15 voting responses. For each feature, record:
- Total vote count
- Which personas voted for it
- A condensed summary of their reasons

Produce a ranked vote table:

```
F-NNN  Feature Name                  Votes  Voters
-----  ----------------------------  -----  ----------------------------------
F-046  State Persistence             11     impl-rust, impl-testing, ...
...
```

Note any features that received strong consensus (many voters) vs. niche advocacy (one or two specialists). A feature championed by `impl-agent-ux` alone still deserves consideration if agent experience is a priority.

### 4. Select the batch

Choose 5–10 features for the implementation batch using the vote tally as the primary signal. Prefer:
- High vote counts (broad consensus across personas)
- Logical grouping (features that share a spec file or implementation surface)
- Coherent scope (features that can be reviewed and implemented together)

Note any manual overrides to the vote ranking and why (e.g. "F-055 ranked 3rd but grouped with F-056 which ranked 8th, so both included").

### 5. Implementation review round — dispatch all personas in parallel

Using the Agent tool, launch ALL of the same agents simultaneously again. Give each the same task:

"The following features have been selected for the next implementation batch based on a voting round. Please now do a full implementation review from your area of expertise. For each feature, identify concerns, risks, design decisions, and anything that should be resolved before coding begins.

Selected features: [list F-NNN numbers, names, and spec file links]

Focus on your area of expertise and produce numbered findings with severity ratings."

### 6. Synthesize findings

Read all 15 review outputs. Group findings by theme across features. Identify:
- **Blockers** — must be resolved before implementation begins
- **Implementation decisions** — explicit choices to make, each with a recommended default
- **Test requirements** — specific test cases called out across reviewers
- **Spec clarifications** — ambiguities that would affect implementation
- **Discarded concerns** — out of scope or inapplicable, with brief rationale

### 7. Produce PLAN.md

Write `PLAN.md` at the project root:

```markdown
# Implementation Plan — [date]

## Vote Tally
[Full ranked table from step 3]

## Selected Features
[F-NNN list with one-line summaries and rationale for any ranking overrides]

## Implementation Review Findings
[Synthesized findings grouped by theme, with reviewer attribution]

## Open Decisions
[Explicit choices with recommended options]

## Spec Clarifications Needed
[Ambiguities to resolve before or during implementation]

## Implementation Steps
[For each feature: source files to touch, types/structs to add, test cases required]
```

Do not mark any feature as `PLANNED` in the inventory or begin implementation until the user has reviewed and approved `PLAN.md`.
