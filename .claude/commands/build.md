Implement the next task from the TODO list.

## Steps

### 1. Find the next task

Read `TODO.md` at the repo root.

Find the first task (`### Task N: ...`) that has unchecked steps (`- [ ]`). Tasks are ordered by dependency — do not skip to a later task if an earlier one is incomplete.

If there are no unchecked tasks, tell the user the TODO list is complete and stop.

Tell the user which task you are starting before proceeding.

### 2. Check if the task is already done

Explore the code to see if the task has already been implemented. If it has, mark all its steps done (`- [x]`) in `TODO.md` and stop.

### 3. Implement

Work through **all steps** of the chosen task. Follow the `CLAUDE.md` quality gates:

- Read the relevant spec in `specs/` before implementing
- Implement first, then write tests — no stubs or placeholders
- `just validate` must pass (runs fmt-check, lint, and tests)
- No `unwrap()` or `expect()` in production code
- Update `REVIEW.md` with decisions, conflicts, or open questions

When all steps are complete, mark each finished step in `TODO.md` (`- [ ]` → `- [x]`).

Commit following the conventional commit rules in `CLAUDE.md`. Do not push unless asked.

Report: task name, what was implemented, and whether any REVIEW.md entries were added.
