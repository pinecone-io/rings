You are coordinating one review cycle for the plan-review workflow.

## Phase identity

You are the `review` phase of `plan-review`. You run once per cycle. Your job is to
run the next reviewer agent that has not yet produced findings for the current draft.

## Reviewer roster

The authoritative ordered list of reviewers (12 total):

1. impl-testing
2. impl-error-handling
3. impl-cli-framework
4. impl-serialization
5. impl-process-mgmt
6. impl-filesystem
7. impl-cross-platform
8. impl-performance
9. impl-memory
10. impl-regex
11. impl-agent-ux
12. impl-docs

## Setup

### Step 1: Debris cleanup (first cycle only)

Check `rings/plan-review/wip/` for files matching `review-*.md`.

If **zero** such files exist, delete any other files in `rings/plan-review/wip/` before
proceeding. This clears stale state from any previously interrupted run.

### Step 2: Determine which reviewer to run next

Check `rings/plan-review/wip/` for existing `review-{persona}.md` files. Any file
found is treated as complete regardless of how it was produced (supports resume).

Iterate the roster in order. Pick the first reviewer whose
`rings/plan-review/wip/review-{persona}.md` does not exist.

### Step 3: Read the current draft

Find the first entry in `queues/PLAN_DRAFTS.md` whose status line begins `## [DRAFT]`.
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

Write the agent's full output to `rings/plan-review/wip/review-{persona}.md` where
`{persona}` is the reviewer's name (e.g. `review-impl-testing.md`).

### Step 6: Signal

Check whether all 12 reviewers now have a `review-{persona}.md` file in
`rings/plan-review/wip/`.

**If any reviewers remain:**

Print the following on its own line:

```
RINGS_CONTINUE
```

**If all 12 reviewers have run:**

Do NOT print `RINGS_CONTINUE`. Exit normally. The cycle will fall through to `synthesize`.
