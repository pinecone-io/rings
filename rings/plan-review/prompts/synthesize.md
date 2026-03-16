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

Consolidate all reviewer findings. Discard findings that are inapplicable or already
addressed in the draft. When multiple reviewers raise the same issue, consolidate into
one entry.

Produce four sections:

- **Implementation Steps** — an ordered, dependency-aware list of concrete steps to
  execute. Prerequisite work (missing abstractions, new dependencies, data model
  changes) goes first as early steps, then per-feature work. Each step names the
  files to touch, the types/traits/functions to add or change, and the test cases
  required for that step. Cross-step dependencies are noted inline.
- **Open Decisions** — explicit choices with meaningful tradeoffs; include a
  recommended default for each.
- **Test Requirements** — any test cases not already captured in the steps above.
- **Spec Gaps** — ambiguities in the specs that the implementer should note or
  resolve during implementation.

## Write output

### Step 1: Append to READY_TO_IMPLEMENT.md

Append the following entry to `queues/READY_TO_IMPLEMENT.md`:

```markdown
## Batch: <batch name> — <date>

**Features:** [F-NNN list with names]

### Implementation Steps

#### Step N: <short title>

**Files:** <files to create or modify>

<what to implement, including key types/traits/functions>

**Tests:** <test cases required for this step>

[repeat for each step]

---

### Open Decisions

| ID | Decision | Recommendation |
|----|----------|----------------|
[one row per decision]

### Test Requirements

[any test cases not already captured in the steps above]

### Spec Gaps

[ambiguities to note or resolve during implementation]
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
