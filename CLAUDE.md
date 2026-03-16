# rings — Development Process

## Testing

- **Implement first, then test.** Write the real implementation, then write tests that verify it. Do not write tests first and fill in stubs to make them compile — placeholder implementations are forbidden.
- **No `todo!()`, `unimplemented!()`, or stub bodies in committed code.** Every committed function must do what it says.
- **No live `claude` invocations in tests.** The process execution layer must sit behind a trait (e.g., `Executor` in `src/executor.rs`) so it can be mocked. Never call an actual `claude` subprocess in any test.
- **A feature is done when:** happy path and key error paths are covered at both unit and integration level.
- `unwrap()` in tests is fine — a panic fails the test, which is the intended behavior.

## Code Quality Gates

A task is not done until **all** of the following are satisfied:

1. **Tests pass** — `just validate` is clean (runs fmt-check, lint, and tests)
2. **Formatting** — run `just fmt` to auto-fix, then re-run `just validate`
4. **No `unwrap()` or `expect()` in production code** — all errors propagate via `?` and `anyhow`
5. **Spec consulted** — the relevant spec in `specs/` was read and the implementation is consistent with it. If no directly relevant spec exists, note that in `REVIEW.md` under Open Questions.
6. **REVIEW.md updated** — any decisions, conflicts, or open questions from this task are recorded

## Commits

Conventional commit prefixes are required:

- `feat:` — new functionality
- `fix:` — bug fixes
- `test:` — adding or updating tests
- `refactor:` — code restructuring without behavior change
- `chore:` — build, deps, config
- `docs:` — documentation only

Scopes are optional (e.g., `feat(executor): ...`). No PRs. No branches. Commit directly to `main`.

## Filing Technical Improvements

If you notice an opportunity to improve the codebase internally — a refactor, duplicate
code, a dependency that could be simplified, a performance issue, better test coverage,
or anything else that makes rings easier to work on — add an entry to `queues/TECH_DEBT.md`
under `## Unprocessed`:

```
- [ ] **<short title>**: <what to change and why it's better>
```

Only file items that do not add, remove, or change any product behavior described in
`specs/`. If the change would alter observable behavior, file it in `queues/IDEAS.md` instead.

The `rings/process-improvements/process-improvements.rings.toml` workflow will pick it up in a future run.

## Filing Bug Reports

If you encounter a bug — unexpected behavior, a crash, a spec violation, or a broken test you cannot fix within your task scope — add an entry to `queues/BUG_REPORT.md` under `## Open`:

```
- [ ] **<short title>**: <what happened> — <what was expected instead>
```

Be specific: include the file path, function name, or test name where the bug manifests. Do not leave a bug silently unresolved; if you can't fix it, file it.

The `rings/process-bugs/process-bugs.rings.toml` workflow will pick it up in a future run.

## Rings Workflow File Organization

When writing or modifying rings workflows, follow these conventions:

- **`queues/`** — files intended to be consumed by other workflows. Each queue file holds an ordered list of entries that workflows process and produce. Examples: `queues/IDEAS.md`, `queues/PLAN_DRAFTS.md`. Contains only unprocessed and in-flight items — never completed ones.
- **`activities/`** — permanent records of completed work. Workflows append here when closing an item; nothing reads these files as input. Examples: `activities/BUGS_RESOLVED.md`, `activities/IDEAS_PROCESSED.md`, `activities/TECH_DEBT_RESOLVED.md`.
- **`rings/<workflow-name>/wip/`** — ephemeral state internal to a workflow's cycles. These files are scratch space for intermediate outputs within a run and must never be treated as durable. They should be cleaned up by the workflow itself (typically in the final synthesizing phase).
- **First-phase cleanup** — the first phase of a cycle should delete any stale debris in its `wip/` directory before beginning new work. This prevents leftover files from an interrupted prior run from corrupting the current run's state detection logic.

Never write ephemeral state to the repository root or to `queues/`. If a file is only meaningful within a single workflow run, it belongs in `rings/<workflow-name>/wip/`. If a file records completed work for audit purposes, it belongs in `activities/`.

## Agent Behavior

- **Specs are the source of truth.** When code and spec conflict, fix the code to match the spec, then record the conflict in `REVIEW.md` under Conflicts.
- **When uncertain**, pick the approach most consistent with `specs/`, then record the decision in `REVIEW.md` under Decisions.
- **Never edit files in `specs/`.** Suggest refinements or corrections in `REVIEW.md` under Open Questions.

### Cross-File Spec References

`specs/feature_inventory.md` is a one-line-per-feature index of everything defined across all spec files. It is the primary navigation aid for token-efficient spec lookup — load it first to find the right spec file, then load only that file.

**Whenever you add, rename, or substantially change a feature in any `specs/` file, also update `specs/feature_inventory.md`** so the index stays accurate. This applies to:
- New features or flags added to any spec
- Features renamed or split across files
- Features removed or marked deprecated
- New spec files added to `specs/`

### REVIEW.md structure

Three fixed sections. Append entries within each section — never overwrite or delete prior entries. Each entry should begin with a date or task reference so entries can be distinguished.

If `REVIEW.md` does not exist, create it with the three section headers before appending the first entry.

```markdown
## Decisions
<!-- Architectural or design choices made during implementation. -->
<!-- Format: `[YYYY-MM-DD / task name] description` -->

## Conflicts
<!-- Cases where code and spec disagreed; what was changed and why. -->
<!-- Format: `[YYYY-MM-DD / task name] description` -->

## Open Questions
<!-- Ambiguities, spec gaps, or missing specs that need human review. -->
<!-- Format: `[YYYY-MM-DD / task name] description` -->
```

When no relevant spec exists for a decision being made, record the decision under **Decisions** *and* note the missing spec under **Open Questions**.

## Pre-Commit Checklist

Before every commit, verify and record the following. Copy this block into your working notes and check each item:

```
Pre-commit checklist
--------------------
[ ] just validate — all gates pass (fmt, lint, tests)
[ ] No unwrap()/expect() added to production code
[ ] Relevant spec in specs/ consulted; implementation is consistent
[ ] REVIEW.md updated with decisions, conflicts, or open questions from this task
[ ] Commit message uses a conventional commit prefix (feat/fix/test/refactor/chore/docs)
```

Do not commit until every item is checked. If an item cannot be satisfied, record why in `REVIEW.md` under Open Questions before proceeding.

## Security

- Prompts must be passed to `claude` via **stdin**, not as CLI arguments. Passing prompts as CLI arguments exposes them in `ps aux` output, which is readable by any user on the system.
- Every code path that constructs executor command arguments must be covered by a test asserting that no prompt content appears in those arguments (i.e., the `args` passed to `Command::new("claude")` contain no prompt text).
