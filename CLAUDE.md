# rings — Development Process

## Testing

- **TDD by default.** Write failing tests first, then implement.
- **No live `claude` invocations in tests.** The process execution layer must sit behind a trait (e.g., `Executor` in `src/executor.rs`) so it can be mocked. Never call an actual `claude` subprocess in any test.
- **A feature is done when:** happy path and key error paths are covered at both unit and integration level.
- `unwrap()` in tests is fine — a panic fails the test, which is the intended behavior.

## Code Quality Gates

A task is not done until **all** of the following are satisfied:

1. **Tests pass** — `cargo test` is clean
2. **Formatting** — run `cargo fmt` to fix, then verify with `cargo fmt --check`
3. **Linting** — `cargo clippy -- -D warnings` produces zero warnings
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

## Security

- Prompts must be passed to `claude` via **stdin**, not as CLI arguments. Passing prompts as CLI arguments exposes them in `ps aux` output, which is readable by any user on the system.
- Every code path that constructs executor command arguments must be covered by a test asserting that no prompt content appears in those arguments (i.e., the `args` passed to `Command::new("claude")` contain no prompt text).
