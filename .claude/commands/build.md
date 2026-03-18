Implement the next task from the ready-to-implement queue.

## Steps

### 1. Check for active work

Read `rings/build/queue/NEXT.md`.

If it has any unchecked tasks (`- [ ]`), skip to **Step 3**.

### 2. Refill NEXT.md from the queue

NEXT.md has no unchecked tasks. Archive and refill:

1. If NEXT.md has content (completed tasks):
   - Append its entire content to `rings/build/activities/BATCHES_COMPLETED.md` (create if absent).
   - Then overwrite `NEXT.md` with empty content.

2. Read `rings/build/queue/READY_TO_IMPLEMENT.md`. Find the first batch (starts with `## Batch:`).

3. If there are no batches, tell the user the queue is empty and stop.

4. Copy the first batch from `READY_TO_IMPLEMENT.md` into `NEXT.md`, then remove it from `READY_TO_IMPLEMENT.md`. Then stop — do not continue to Step 3.

### 3. Choose a task

Read `specs/mvp.md` to orient yourself on the large-scale goal.

In `NEXT.md`, find the first task (`### Task N: ...`) that has unchecked steps (`- [ ]`). Tasks are ordered by dependency — do not skip to a later task if an earlier one is incomplete.

Tell the user which task you are starting before proceeding.

### 4. Check if the task is already done

Explore the code to see if the task has already been implemented. If it has, mark all its steps done (`- [x]`) in `NEXT.md` and stop.

### 5. Implement

Work through **all steps** of the chosen task. Follow the `CLAUDE.md` quality gates:

- Read the relevant spec in `specs/` before implementing
- Implement first, then write tests — no stubs or placeholders
- `just validate` must pass (runs fmt-check, lint, and tests)
- No `unwrap()` or `expect()` in production code
- Update `REVIEW.md` with decisions, conflicts, or open questions

When all steps are complete, mark each finished step in `NEXT.md` (`- [ ]` → `- [x]`).

Commit following the conventional commit rules in `CLAUDE.md`. Do not push unless asked.

Report: task name, what was implemented, and whether any REVIEW.md entries were added.
