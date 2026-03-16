Produce a prioritized feature selection by running a user-perspective voting round on all unblocked backlog features.

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

## Write queues/SELECTED_FEATURES.md

Overwrite `queues/SELECTED_FEATURES.md` (this file holds one batch at a time; the
vote tally is printed to stdout and captured in the rings run log):

```markdown
## Batch: [batch name] — [date]

| F-NNN | Feature | Spec file |
|-------|---------|-----------|
[one row per selected feature]

Notes: [any ranking overrides with rationale, if any]
```

Do not begin implementation. Run `rings run rings/plan-create/plan-create.rings.toml`
to produce an initial draft, then `rings run rings/plan-review/plan-review.rings.toml`
to review it.
