# Review Panel

Your job is to run the 15-persona review panel against the idea in `idea-working.md` and
synthesize their findings back into that file.

---

## Step 1: Load the idea

Read `idea-working.md` in full. Read the spec file listed under `## Spec File`. Read
`specs/overview.md` and `specs/feature_inventory.md` for broader context.

---

## Step 2: Dispatch the review panel

Using the Agent tool, launch ALL of the following agents **in a single message with 15 parallel
tool calls**. Give each the same prompt below, substituting the actual idea and context.

Agents: `review-cli`, `review-devops`, `review-data-eng`, `review-ai-newcomer`, `review-gen-z`,
`review-security`, `review-token-opt`, `review-reliability`, `review-scripter`, `review-oss`,
`review-founder`, `review-prompt-eng`, `review-enterprise`, `review-agent-ux`, `review-workflow-author`

---

*"You are a member of the rings project review panel. Before reviewing, orient yourself:*

- *Read `specs/index.md` — what rings is and core concepts*
- *Read `specs/overview.md` — design principles and target user*
- *Read `specs/mvp.md` — what was built first and why*
- *Read `specs/feature_inventory.md` — what is already specified*

*The following idea has been proposed for addition to the rings specification. Review it from
your area of expertise. Identify: concerns or risks before we commit it to the spec, suggested
refinements to scope or design, dependencies or interactions with existing features, and anything
your area of expertise requires that the idea doesn't mention.*

*Proposed idea and classification:*
*[copy the ## Raw Idea and ## Classification sections from idea-working.md]*

*Related existing features:*
*[copy the ## Related Features section from idea-working.md]*

*Return your findings concisely. Lead with the most important concern if you have one. If the
idea looks solid from your perspective, say so briefly."*

---

## Step 3: Synthesize findings

Read all 15 responses. Write a synthesis under `## Review Synthesis` in `idea-working.md`.

The synthesis must cover:

- **Concerns to resolve before writing the spec** — anything a reviewer flagged as a blocker or
  significant gap, especially from `review-workflow-author` (whose findings carry the highest weight)
- **Refinements** — suggested changes to scope or design that emerged from the panel
- **Dependencies** — interactions with other features not obvious from the original idea
- **Verdict** — one of: `Proceed as stated` / `Proceed with refinements` / `Needs significant rework`

If the verdict is `Needs significant rework`, describe what would need to change before the write
phase can proceed. The write phase will use this to shape the spec entry.
