## Decisions
<!-- Architectural or design choices made during implementation. -->
<!-- Format: `[YYYY-MM-DD / task name] description` -->

[2026-03-15 / Task 2: GitHub Actions CI] CI targets `x86_64-unknown-linux-musl` only for now (static Linux binary). macOS targets deferred until there is a macOS runner need. `nightly` tag is force-updated on every push to main — only one nightly exists at any time. `install.sh` defaults install destination to `/usr/local/bin/rings` but accepts a path argument.

[2026-03-15 / Task 3: Workflow TOML parsing] Implemented `std::str::FromStr` for `Workflow` instead of an inherent `from_str` method, to satisfy `clippy::should_implement_trait`. Tests import `std::str::FromStr` to call `Workflow::from_str`. Added `src/lib.rs` to expose modules to integration tests (binary crate integration tests require a lib target). Consulted `specs/mvp.md` as the primary spec for workflow field requirements and validation rules.

## Conflicts
<!-- Cases where code and spec disagreed; what was changed and why. -->
<!-- Format: `[YYYY-MM-DD / task name] description` -->

## Open Questions
<!-- Ambiguities, spec gaps, or missing specs that need human review. -->
<!-- Format: `[YYYY-MM-DD / task name] description` -->

[2026-03-15 / Task 4: Prompt template rendering] `specs/execution/prompt-templating.md` requires that unknown template variables produce a startup advisory warning. The current `render_prompt` implementation silently passes unknown variables through unchanged. This behavior is correct for render_prompt itself (it is not responsible for warnings), but the engine startup sequence must call a validator that scans prompt text for unrecognized `{{...}}` patterns and emits a warning. Track this in the engine task.

[2026-03-15 / Task 2: GitHub Actions CI] Steps 5 and 6 (verify workflow ran, test one-line install) require a GitHub remote configured and the code pushed to GitHub. The local repository has no remote configured. The infrastructure files (`.github/workflows/ci.yml` and `install.sh`) are correctly implemented and ready. To fully complete Task 2: (1) configure a GitHub remote, (2) push the commits, (3) wait for the workflow to complete via `gh run watch`, (4) verify the nightly release was created, (5) test the one-line install. These steps are deferred pending GitHub repository setup.

[2026-03-15 / Task 1: Initialize Cargo project] No spec directly governs dependency versions. Versions chosen match the plan header; pin to exact versions in Cargo.lock.

[2026-03-15 / Task 3: Workflow TOML parsing] `context_dir` existence and prompt file readability checks are deferred from `Workflow::validate` — per `specs/mvp.md`, these checks belong in the engine startup sequence (Task 10), not at parse time. Missing spec for a separate `specs/workflow.md` — the validation rules are sourced from `specs/mvp.md` directly.

[2026-03-15 / Code Review] Review of current implementation state: Tasks 1 and 3 are correctly implemented and all quality gates pass. Tasks 2 (CI/CD), 4 (template), 5 (cost), 6 (completion), 7 (executor), 8 (scheduling), 9 (state), 10 (audit), 11 (engine), 12 (CLI), 13 (display), and 14 (main.rs wiring) are entirely unimplemented — all existing unchecked steps in the plan are correct. Two additional critical gaps were identified and added to the plan as Tasks 15 and 16: (1) Task 14's plan spec uses `.expect()` on `ctrlc::set_handler`, violating CLAUDE.md production code quality gate — must use `?` propagation. (2) Task 14's Ctrl+C handler sets an AtomicBool flag but the engine loop never checks it, meaning SIGINT does not save state or exit 130 as required by `specs/mvp.md`. Both issues are now tracked as Tasks 15 and 16 in the plan under "Chunk: Review Fixes".
