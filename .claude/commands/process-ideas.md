Read rough ideas from IDEAS.md, evaluate them against the existing specs, run them through the full review panel in parallel, then produce concrete spec proposals or new feature entries.

## Steps

### 1. Load context

- If `IDEAS.md` does not exist at the project root, tell the user to create one with their rough notes and stop.
- Read `IDEAS.md` in full.
- Read `specs/feature_inventory.md` to understand what is already specified.
- Read `specs/index.md` and any spec files directly relevant to the ideas.
- Read `specs/overview.md` and `specs/mvp.md` to understand design principles and scope.

### 2. Classify each idea

For each idea in `IDEAS.md`, classify it as:
- **Already covered** — essentially described by an existing spec; note the F-NNN number(s) and explain how.
- **Extension** — adds nuance or a new option to an existing feature; identify the F-NNN being extended.
- **New feature** — genuinely not covered; note which spec file it would belong in.
- **Out of scope / conflict** — conflicts with a design principle; explain why and suggest an alternative if one exists.

Set aside anything classified as "already covered" or "out of scope" — do not send those to the review panel.

### 3. Dispatch review panel in parallel

For all ideas classified as **extension** or **new feature**, launch ALL of the following agents simultaneously using the Agent tool. Give each the same task:

---
*"You are a member of the rings project review panel. Before reviewing, orient yourself by reading:*
- *`specs/index.md` — what rings is and core concepts*
- *`specs/overview.md` — design principles and target user*
- *`specs/mvp.md` — what was built first and why*
- *`specs/feature_inventory.md` — what is already specified*

*Review the following proposed ideas and provide your perspective from your area of expertise. For each idea, identify concerns, risks, gaps, or improvements before we commit to writing them into the spec. Also note anything the idea is missing that your area of expertise would require.*

*Proposed ideas: [summarize each idea with its classification]*
*Relevant existing features for context: [list related F-NNN entries and their spec files]*"*

---

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
- `review-agent-ux`
- `review-workflow-author`

### 4. Synthesize review findings

Read all 15 review outputs. For each proposed idea, compile:

> **Priority note:** findings from `review-workflow-author` carry the highest weight. If that persona identifies a concern that conflicts with another reviewer's preference, the workflow author wins. A feature that's architecturally clean but makes workflows hard to author or debug is not a good feature.
- Concerns that should be resolved before writing the spec
- Suggested refinements to the idea's scope or design
- Dependencies or interactions with other features that weren't obvious
- Anything a reviewer flagged as a blocker

### 5. Produce spec proposals

For each idea that survives review (or has been refined by it), produce:
- A proposed feature number (continuing from the highest existing F-NNN)
- A one-line user-perspective summary in the style of the inventory
- Status: `BACKLOG`
- The spec file it belongs in
- A short prose description (3–6 sentences) suitable for dropping into that spec file
- Dependencies on existing features (cite F-NNN)
- A note summarizing any significant concerns raised by the review panel

### 6. Present for approval

Output all proposals in a single reviewable block. Do not modify any spec files, `IDEAS.md`, or `specs/feature_inventory.md` until the user approves. After approval, ask whether to move approved ideas from `## Unprocessed` to a `## Processed` section in `IDEAS.md`.
