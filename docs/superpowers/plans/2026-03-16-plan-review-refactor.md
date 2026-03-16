# Plan Review Refactor Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `plan-impl` workflow with two focused workflows (`plan-create` and `plan-review`) that use rings' cycle mechanism to iterate through reviewers one at a time, eliminating context exhaustion and prompt duplication.

**Architecture:** `plan-create` is a single-phase workflow that dispatches 3 core agents in parallel to draft an initial plan into `queues/PLAN_DRAFTS.md`. `plan-review` is a 2-phase cycle workflow that runs one reviewer per cycle (12 total), writing per-reviewer findings to `rings/plan-review/wip/`, then synthesizes all findings into `queues/READY_TO_IMPLEMENT.md`. The feature-election command is updated to write `queues/SELECTED_FEATURES.md` (the input to `plan-create`) instead of `queues/PLAN.md`.

**Tech Stack:** TOML (rings workflow config), Markdown (prompts and queue files)

**Spec:** `docs/superpowers/specs/2026-03-16-plan-review-refactor-design.md`

---

## Chunk 1: plan-create workflow

### Task 1: Create the plan-create TOML

**Files:**
- Create: `rings/plan-create/plan-create.rings.toml`
- Create: `rings/plan-create/prompts/draft.md`

- [ ] **Step 1: Create the directory and TOML**

```bash
mkdir -p rings/plan-create/prompts
```

Create `rings/plan-create/plan-create.rings.toml`:

```toml
[workflow]
# Reads queues/SELECTED_FEATURES.md, dispatches 3 core technical agents in parallel,
# and appends a [DRAFT] plan entry to queues/PLAN_DRAFTS.md.
#
# Run with: rings run rings/plan-create/plan-create.rings.toml
# Resume with: rings resume <run-id>

completion_signal = "PLAN_DRAFT_DONE"
completion_signal_mode = "line"
completion_signal_phases = ["draft"]
context_dir = "."
max_cycles = 1

[executor]
binary = "claude"
args = ["--dangerously-skip-permissions", "-p", "-"]

[[phases]]
name = "draft"
prompt = "rings/plan-create/prompts/draft.md"
```

- [ ] **Step 2: Verify the TOML is valid**

```bash
cat rings/plan-create/plan-create.rings.toml
```

Expected: file contents print without error.

### Task 2: Write the draft prompt

- [ ] **Step 1: Create the draft prompt**

Create `rings/plan-create/prompts/draft.md`:

```markdown
You are coordinating the initial technical draft for the plan-create workflow.

## Phase identity

You are the `draft` phase of `plan-create`.

## Setup

Read `queues/SELECTED_FEATURES.md`. Find the batch header (`## Batch: ...`) and extract the feature table (F-NNN, feature name, spec file path).

For each selected feature, read its spec file in full.

If `queues/SELECTED_FEATURES.md` is empty or contains no batch header, stop immediately and print:

```
PLAN_DRAFT_DONE
```

## Dispatch

Using the Agent tool, launch the following 3 agents **in a single message with 3 parallel tool calls**. Give each the prompt below, substituting the actual feature list and spec content.

Agents: `impl-rust`, `impl-architecture`, `impl-deps`

---
*"You are a member of the rings initial planning panel. Before reviewing, orient yourself:*
- *Read `specs/index.md` and `specs/overview.md` to understand what rings is*
- *Read the relevant source files in `src/` that relate to your area of expertise*
- *Read the spec files listed below for the selected features*

*The following features have been selected for the next implementation batch. Produce an initial technical plan from your area of expertise. For each feature, identify: source files to create or modify, key types/traits/structs to introduce, test cases required (unit and integration), and any cross-feature dependencies.*

*Selected features and their specs:*
*[list F-NNN · Name · spec file for each]*

*Return your findings as structured markdown."*

---

## Record draft

Collect the three agents' outputs. Append the following to `queues/PLAN_DRAFTS.md`:

```markdown
## [DRAFT] Batch: <batch name from SELECTED_FEATURES.md> — <today's date>

### Selected Features

| F-NNN | Feature | Spec file |
|-------|---------|-----------|
[feature table rows]

### Source Files

[Merged list of source files to create or modify, deduplicated across all three agents]

### Key Types, Traits, and Structs

[Merged list of types/traits/structs to introduce, with brief purpose for each]

### Test Cases Required

[Merged list of unit and integration test cases called out by all three agents]

### Cross-Feature Dependencies

[Any dependencies between features in this batch, e.g. F-020 depends on F-054's RunHandle]
```

Then print the following on its own line to signal completion:

```
PLAN_DRAFT_DONE
```
```

- [ ] **Step 2: Verify the prompt file exists and reads cleanly**

```bash
wc -l rings/plan-create/prompts/draft.md
```

Expected: line count prints without error.

- [ ] **Step 3: Commit**

```bash
git add rings/plan-create/
git commit -m "feat: add plan-create workflow"
```

---

## Chunk 2: plan-review workflow

### Task 3: Create the plan-review TOML

**Files:**
- Create: `rings/plan-review/plan-review.rings.toml`
- Create: `rings/plan-review/prompts/review.md`
- Create: `rings/plan-review/prompts/synthesize.md`

- [ ] **Step 1: Create the directory and TOML**

```bash
mkdir -p rings/plan-review/prompts rings/plan-review/wip
```

Create `rings/plan-review/plan-review.rings.toml`:

```toml
[workflow]
# Iterates through 12 reviewer agents (one per cycle), writing per-reviewer findings
# to rings/plan-review/wip/. When all reviewers are done, synthesizes findings into
# queues/READY_TO_IMPLEMENT.md and marks the draft [REVIEWED] in PLAN_DRAFTS.md.
#
# max_cycles MUST equal the number of reviewers in the roster (currently 12).
# If reviewers are added or removed from the roster in prompts/review.md,
# update max_cycles to match.
#
# Run with: rings run rings/plan-review/plan-review.rings.toml
# Resume with: rings resume <run-id>

completion_signal = "PLAN_REVIEW_DONE"
completion_signal_mode = "line"
completion_signal_phases = ["synthesize"]
continue_signal = "RINGS_CONTINUE"
context_dir = "."
max_cycles = 12

[executor]
binary = "claude"
args = ["--dangerously-skip-permissions", "-p", "-"]

[[phases]]
name = "review"
prompt = "rings/plan-review/prompts/review.md"

[[phases]]
name = "synthesize"
prompt = "rings/plan-review/prompts/synthesize.md"
```

- [ ] **Step 2: Verify the TOML**

```bash
cat rings/plan-review/plan-review.rings.toml
```

Expected: file contents print without error.

### Task 4: Write the review prompt

- [ ] **Step 1: Create the review prompt**

Create `rings/plan-review/prompts/review.md`:

```markdown
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
area of expertise. For each feature, identify concerns, risks, design decisions, and
anything that must be resolved before coding begins.*

*Selected features and their specs:*
*[list F-NNN · Name · spec file for each]*

*Draft plan:*
*[full draft plan content]*

*Return numbered findings with severity (nit / concern / blocker) and concrete suggestions."*

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
```

- [ ] **Step 2: Verify the review prompt**

```bash
wc -l rings/plan-review/prompts/review.md
```

Expected: line count prints without error.

### Task 5: Write the synthesize prompt

- [ ] **Step 1: Create the synthesize prompt**

Create `rings/plan-review/prompts/synthesize.md`:

```markdown
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
```

- [ ] **Step 2: Verify the synthesize prompt**

```bash
wc -l rings/plan-review/prompts/synthesize.md
```

Expected: line count prints without error.

- [ ] **Step 3: Add a .gitkeep to preserve the wip directory**

```bash
touch rings/plan-review/wip/.gitkeep
```

- [ ] **Step 4: Commit**

```bash
git add rings/plan-review/
git commit -m "feat: add plan-review workflow"
```

---

## Chunk 3: Update feature-election and remove plan-impl

### Task 6: Update the feature-election command

**Files:**
- Modify: `.claude/commands/feature-election.md`

The feature-election command currently writes `queues/PLAN.md`. It must be updated to write `queues/SELECTED_FEATURES.md` instead, and its trailing instruction must point to `plan-create` rather than `plan-impl`.

- [ ] **Step 1: Update the output section**

In `.claude/commands/feature-election.md`, replace the entire `## Write queues/PLAN.md` section (from line `## Write queues/PLAN.md` through the end of the file) with:

```markdown
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
```

- [ ] **Step 2: Verify the edit**

```bash
tail -20 .claude/commands/feature-election.md
```

Expected: the new `SELECTED_FEATURES.md` write instruction appears; no mention of `PLAN.md`.

- [ ] **Step 3: Commit**

```bash
git add .claude/commands/feature-election.md
git commit -m "feat: update feature-election to write SELECTED_FEATURES.md"
```

### Task 7: Delete plan-impl and any remaining PLAN.md

**Files:**
- Delete: `rings/plan-impl/` (entire directory)
- Delete: `queues/PLAN.md` (if it still exists; may have been removed in a prior migration step)

- [ ] **Step 1: Remove plan-impl**

```bash
git rm -r rings/plan-impl/
```

- [ ] **Step 2: Remove queues/PLAN.md if still present**

```bash
git rm --ignore-unmatch queues/PLAN.md
```

Expected: either `rm 'queues/PLAN.md'` (removed) or no output (already gone). Both are correct.

- [ ] **Step 3: Verify no remaining references to PLAN.md or plan-impl**

```bash
grep -r "PLAN\.md\|plan-impl" .claude/ rings/ --include="*.md" --include="*.toml" -l
```

Expected: no output (no files reference the deprecated names).

- [ ] **Step 4: Commit**

```bash
git commit -m "chore: remove plan-impl workflow (replaced by plan-create + plan-review)"
```
