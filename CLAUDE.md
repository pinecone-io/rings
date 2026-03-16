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
