## Decisions
<!-- Architectural or design choices made during implementation. -->
<!-- Format: `[YYYY-MM-DD / task name] description` -->

[2026-03-15 / Task 3: Workflow TOML parsing] Implemented `std::str::FromStr` for `Workflow` instead of an inherent `from_str` method, to satisfy `clippy::should_implement_trait`. Tests import `std::str::FromStr` to call `Workflow::from_str`. Added `src/lib.rs` to expose modules to integration tests (binary crate integration tests require a lib target). Consulted `specs/mvp.md` as the primary spec for workflow field requirements and validation rules.

## Conflicts
<!-- Cases where code and spec disagreed; what was changed and why. -->
<!-- Format: `[YYYY-MM-DD / task name] description` -->

## Open Questions
<!-- Ambiguities, spec gaps, or missing specs that need human review. -->
<!-- Format: `[YYYY-MM-DD / task name] description` -->

[2026-03-15 / Task 1: Initialize Cargo project] No spec directly governs dependency versions. Versions chosen match the plan header; pin to exact versions in Cargo.lock.

[2026-03-15 / Task 3: Workflow TOML parsing] `context_dir` existence and prompt file readability checks are deferred from `Workflow::validate` — per `specs/mvp.md`, these checks belong in the engine startup sequence (Task 10), not at parse time. Missing spec for a separate `specs/workflow.md` — the validation rules are sourced from `specs/mvp.md` directly.
