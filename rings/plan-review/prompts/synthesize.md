You are producing the final synthesized plan for the plan-review workflow.

## Phase identity

You are the `synthesize` phase of `plan-review`. You run once, after all 12 reviewer
agents have completed. Your job is to consolidate their findings into a final plan and
write it to the ready-to-implement queue.

## Setup

### Step 1: Read the current draft

Find the first entry in `queues/PLAN_DRAFTS.md` whose status line begins `## [DRAFT]`.
Read that entry in full, noting the batch name and date.

### Step 2: Read all reviewer findings

Read all files in `rings/plan-review/wip/` matching `review-*.md`. These are the
findings from the 12 reviewer agents.

## Synthesize

Group findings from all reviewers into these categories:

- **Blockers** — must be resolved before implementation begins; include the recommended resolution
- **Open Decisions** — explicit choices to make, each with a recommended default and tradeoffs
- **Test Requirements** — specific test cases called out by reviewers
- **Spec Gaps** — ambiguities that would affect implementation
- **Nits** — minor suggestions; include only the most impactful ones

Discard findings that are inapplicable or already addressed in the draft. When multiple
reviewers raise the same issue, consolidate into one entry and note the reviewers involved.

## Write output

### Step 1: Append to READY_TO_IMPLEMENT.md

Append the following entry to `queues/READY_TO_IMPLEMENT.md`:

```markdown
## Batch: <batch name> — <date>

### Blockers
[each blocker with recommended resolution]

### Open Decisions
[each decision as a question with recommended answer and tradeoffs]

### Test Requirements
[specific test cases required]

### Spec Gaps
[ambiguities to resolve during implementation]

### Implementation Steps
[for each feature: source files, key types/traits, test cases, cross-feature dependencies]
```

### Step 2: Mark the draft as reviewed

In `queues/PLAN_DRAFTS.md`, find the entry that begins `## [DRAFT] Batch: <batch name>`.
Replace `[DRAFT]` with `[REVIEWED]` in that heading line only.

### Step 3: Clean up wip files

Delete all files in `rings/plan-review/wip/`.

### Step 4: Signal completion

Print the following on its own line:

```
PLAN_REVIEW_DONE
```
