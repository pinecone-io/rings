## Batch: Composability, Validation & Resume Safety — 2026-03-16

| F-NNN | Feature | Spec file |
|-------|---------|-----------|
| F-081 | `--dry-run` | specs/cli/commands-and-flags.md |
| F-070 | `rings list` | specs/cli/commands-and-flags.md |
| F-126 | JSONL Output Mode | specs/observability/runtime-output.md |
| F-127 | stderr/stdout Separation | specs/observability/runtime-output.md |
| F-095 | `--output-format` | specs/cli/commands-and-flags.md |
| F-049 | Resume State Recovery | specs/state/cancellation-resume.md |
| F-050 | Workflow File Change Detection | specs/state/cancellation-resume.md |
| F-029 | Unknown Variable Warnings | specs/execution/prompt-templating.md |
| F-013 | Completion Signal Phase Restriction | specs/execution/completion-detection.md |

Notes:
- **F-126 + F-127 + F-095 shipped as a unit**: JSONL Output Mode (9/15 votes) is the top-3 feature but only works when stderr/stdout separation is in place and the `--output-format` flag exists. All three are the same implementation surface and must land together.
- **F-081 and F-070 tied at top**: Both received 9/15 votes across every persona type — developer, operator, scripter, newcomer. They address the single biggest ergonomic gap: no way to validate a workflow before spending money, and no way to find runs after they complete.
- **F-049 and F-050 grouped**: Both protect the resume contract — F-049 (state recovery from audit logs when state.json is corrupted) and F-050 (refuse resume after structural workflow changes). They share an implementation surface in `src/state.rs` and the spec file.
- **F-029 included at rank 5 (6 votes)**: Unknown variable warnings are cheap to implement (template system already exists, variable set is small and closed) and prevent silent prompt corruption across entire multi-cycle runs.
- **F-013 override**: Rank 11 by raw vote count (5/15), but the `review-workflow-author` and `review-prompt-eng` personas rate it as critical for multi-phase workflow correctness. The engine already parses `completion_signal_phases` — implementation cost is low. The batch would be incomplete without it since a reviewer's phase-restricted completion is the canonical rings workflow pattern used in every dogfood workflow.
- **F-053 (Double Ctrl+C, 5 votes) excluded**: Deferred to a process-management batch alongside SIGTERM handling (F-052, PLANNED) and subprocess graceful shutdown (F-054, PLANNED) which are closer to completion.
- **F-037 (Error Classification, 5 votes) excluded**: Natural fit for a future error-handling batch with F-038/F-039 (quota/auth detection) once F-037 is done, rather than splitting the error handling cluster.
- **F-109 (Directory Permissions, 5 votes) excluded**: Security hardening item that belongs with F-110 (Path Traversal Protection) and F-145 (Sensitive Files Warning) in a focused security batch.
