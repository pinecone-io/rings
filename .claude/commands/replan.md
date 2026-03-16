Produce a high-quality, evidence-based implementation plan by running a user-perspective voting round to select the right features, then an implementation review round to plan them correctly.

## Steps

### 1. Orient yourself

Read the following to build a complete picture of the project:
- `specs/index.md` — what rings is and core concepts
- `specs/overview.md` — design principles and target user
- `specs/mvp.md` — original scope and what was built first
- `specs/feature_inventory.md` — full feature list with statuses

Identify all `BACKLOG` features whose prerequisites are all `COMPLETE`. These are the candidates eligible for voting. For each candidate, note its F-NNN, name, one-line summary, spec file, and any dependency notes.

---

## Wave 1: Feature Selection (User Perspective Voting)

### 2. Dispatch review panel for voting — in parallel

Using the Agent tool, launch ALL of the following agents simultaneously. Give each the same task prompt:

---
*"You are a member of the rings project review panel. Before voting, orient yourself by reading these files:*
- *`specs/index.md` — what rings is*
- *`specs/overview.md` — design principles and target user*
- *`specs/mvp.md` — what was built first and why*

*You are being asked to vote on which features to implement next. Below are all unblocked BACKLOG features — features whose prerequisites are already complete:*

*[list each candidate: F-NNN · Name · one-line summary · spec file]*

*Vote for the features that would deliver the most value from your specific perspective. For each vote, write 1–2 sentences explaining why it matters to the people you represent. Vote for as many as you genuinely care about — but be selective. A vote means 'this would make a real difference to me.'*

*Return your votes as:*
*F-NNN — Name — reason"*

---

Agents to dispatch:
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
- `review-agent-ux`
- `review-workflow-author`

### 3. Tally votes and select the batch

Read all 15 voting responses. Build a ranked table:

```
Rank  F-NNN  Feature Name                  Votes  Voters
----  -----  ----------------------------  -----  ---------------------------------
1     F-046  State Persistence             12     review-cli, review-founder, ...
2     F-055  Context Directory Lock        9      review-reliability, ...
...
```

For each feature, note the condensed reasons across voters — patterns in why people voted for something are as important as the count.

Then select a batch of 5–10 features using votes as the primary signal, also considering:
- **Logical grouping** — features that share a spec file or implementation surface
- **Coherent scope** — a batch that can be reviewed and implemented together
- **Niche but critical** — a feature with few votes from a high-priority persona (e.g. `review-workflow-author`) may outrank one with many votes from lower-priority personas

Document any overrides to the raw vote ranking with explicit rationale.

---

## Wave 2: Implementation Planning (Technical Review)

### 4. Load spec context for selected features

For each selected feature, read its linked spec file in full. This context will be passed to the impl agents.

### 5. Dispatch implementation review panel — in parallel

Using the Agent tool, launch ALL of the following agents simultaneously. Give each the same task prompt:

---
*"You are a member of the rings implementation review panel. Before reviewing, orient yourself:*
- *Read `specs/index.md` and `specs/overview.md` to understand what rings is*
- *Read the relevant source files in `src/` that relate to your area of expertise*
- *Read the spec files listed below for the selected features*

*The following features have been selected for the next implementation batch based on a user-perspective voting round. Do a full implementation review from your area of expertise. For each feature, identify concerns, risks, design decisions, and anything that must be resolved before coding begins.*

*Selected features and their specs:*
*[list F-NNN · Name · spec file for each]*

*Return numbered findings with severity (nit / concern / blocker) and concrete suggestions."*

---

Agents to dispatch:
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

### 6. Synthesize findings

Read all 15 implementation review outputs. Group by theme across features:
- **Blockers** — must be resolved before implementation begins
- **Open decisions** — explicit choices to make, each with a recommended default
- **Test requirements** — specific cases called out by reviewers
- **Spec gaps** — ambiguities that would affect implementation
- **Discarded concerns** — inapplicable findings, with rationale

---

## Produce PLAN.md

Write `PLAN.md` at the project root:

```markdown
# Implementation Plan — [date]

## Vote Tally
[Full ranked table with voter names and condensed reasons per feature]

## Selected Features
[F-NNN list with one-line summaries; note any ranking overrides with rationale]

## Implementation Review Findings
[Synthesized findings grouped by theme, with reviewer attribution]

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

Do not mark any feature as `PLANNED` in the inventory or begin implementation until the user has reviewed and approved `PLAN.md`.
