You are coordinating one review cycle for the plan-review workflow.

## Phase identity

You are the `review` phase of `plan-review`. You run once per cycle. Your job is to
run the next reviewer agent that has not yet produced findings for the current draft.

## Reviewer roster

The authoritative ordered list of reviewers (7 total):

1. impl-testing
2. impl-error-handling
3. impl-cli-framework
4. impl-serialization
5. impl-process-mgmt
6. impl-filesystem
7. impl-agent-ux

## Setup

### Step 1: Debris cleanup

**This is cycle {{cycle}} of {{max_cycles}}.**

Assert that `{{max_cycles}}` equals 7 (the number of reviewers in the roster). If it
does not, stop and note the mismatch in `REVIEW.md` — misconfigured `max_cycles` causes
synthesize to run too early or too late.

If `{{cycle}}` == 1, unconditionally delete **all** files in `rings/{{workflow_name}}/wip/`
before proceeding. This clears stale state from any previously interrupted run,
regardless of what files are present. A prior synthesize interrupted before wip cleanup
would leave stale review files that look like current-draft findings — unconditional
cleanup on cycle 1 prevents this.

### Step 2: Determine which reviewer to run next

Check `rings/{{workflow_name}}/wip/` for existing `review-{persona}.md` files. Any file
found is treated as complete regardless of how it was produced (supports resume).

Iterate the roster in order. Pick the first reviewer whose
`rings/{{workflow_name}}/wip/review-{persona}.md` does not exist.

### Step 3: Read the current draft

Find the first entry in `rings/{{workflow_name}}/queue/PLAN_DRAFTS.md` whose status line begins `## [DRAFT]`.
Read that entry in full.

For each spec file referenced in the draft's feature table, read the spec file.

### Step 4: Run the reviewer

Using the Agent tool, launch **one** agent (the next reviewer from the roster).

Give it this prompt, substituting actual feature and draft content:

---
*"You are a member of the rings implementation review panel. Before reviewing, orient yourself:*
- *Read `specs/index.md` and `specs/overview.md` to understand what rings is*
- *Read the relevant source files in `src/` that relate to your area of expertise*
- *Read the spec files listed below for the selected features*

*The following features have been selected for the next implementation batch. A draft
technical plan has already been produced. Do a full implementation review from your
area of expertise. For each feature, identify:*

*1. **Prerequisite work** — things that must be implemented before other steps can proceed (missing abstractions, missing dependencies, data model changes, etc.). These will become early steps in the implementation plan.*
*2. **Design decisions** — explicit choices where there are meaningful tradeoffs. Include a recommended default.*
*3. **Test cases** — specific cases that must be covered.*
*4. **Spec gaps** — ambiguities in the spec that would affect implementation.*

*Selected features and their specs:*
*[list F-NNN · Name · spec file for each]*

*Draft plan:*
*[full draft plan content]*

*Return numbered findings grouped by the four categories above. Skip categories with no findings."*

---

### Step 5: Write findings

Write the agent's full output to `rings/{{workflow_name}}/wip/review-{persona}.md` where
`{persona}` is the reviewer's name (e.g. `review-impl-testing.md`).

### Step 6: Signal

Check whether all 7 reviewers now have a `review-{persona}.md` file in
`rings/{{workflow_name}}/wip/`.

**If any reviewers remain:**

Print the following on its own line:

```
RINGS_CONTINUE
```

**If all 7 reviewers have run:**

Do NOT print `RINGS_CONTINUE`. Exit normally. The cycle will fall through to `synthesize`.
