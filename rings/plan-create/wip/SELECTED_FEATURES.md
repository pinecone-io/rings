## Batch: Resilience, Discovery & Safety Feedback — 2026-03-16

### Vote Tally

| Rank | F-NNN | Feature Name                     | Votes | Voters |
|------|-------|----------------------------------|-------|--------|
| 1    | F-081 | --dry-run                        | 10    | review-cli, review-ai-newcomer, review-gen-z, review-token-opt, review-scripter, review-oss, review-founder, review-prompt-eng, review-agent-ux, review-workflow-author |
| 2    | F-056 | Stale Lock Detection             | 9     | review-cli, review-devops, review-data-eng, review-gen-z, review-security, review-reliability, review-scripter, review-oss, review-enterprise |
| 3    | F-070 | rings list                       | 9     | review-cli, review-devops, review-ai-newcomer, review-gen-z, review-scripter, review-oss, review-founder, review-enterprise |
| 4    | F-050 | Workflow File Change Detection   | 8     | review-data-eng, review-gen-z, review-security, review-reliability, review-scripter, review-oss, review-enterprise, review-workflow-author |
| 5    | F-029 | Unknown Variable Warnings        | 6     | review-cli, review-ai-newcomer, review-gen-z, review-token-opt, review-prompt-eng, review-workflow-author |
| 5    | F-149 | Cost Spike Detection             | 6     | review-devops, review-data-eng, review-token-opt, review-founder, review-prompt-eng, review-agent-ux |
| 7    | F-049 | Resume State Recovery            | 5     | review-devops, review-data-eng, review-reliability, review-oss, review-enterprise |
| 7    | F-053 | Double Ctrl+C                    | 5     | review-cli, review-devops, review-security, review-reliability, review-agent-ux |
| 7    | F-126 | JSONL Output Mode                | 5     | review-devops, review-scripter, review-oss, review-founder, review-agent-ux |
| 7    | F-127 | stderr/stdout Separation         | 5     | review-cli, review-devops, review-scripter, review-oss, review-agent-ux |
| 11   | F-095 | --output-format                  | 4     | review-cli, review-devops, review-gen-z, review-scripter |
| 11   | F-082 | --step                           | 4     | review-ai-newcomer, review-founder, review-agent-ux, review-workflow-author |
| 11   | F-136 | Step-Through Mode                | 4     | review-ai-newcomer, review-agent-ux, review-workflow-author, review-founder |
| 11   | F-037 | Error Classification             | 4     | review-devops, review-ai-newcomer, review-reliability, review-scripter |
| 11   | F-043 | Duration String Parsing          | 4     | review-cli, review-gen-z, review-reliability, review-prompt-eng |
| 11   | F-088 | --budget-cap                     | 4     | review-ai-newcomer, review-token-opt, review-scripter, review-founder |
| 11   | F-150 | No-Files-Changed Streak Warning  | 4     | review-founder, review-prompt-eng, review-agent-ux, review-workflow-author |
| 11   | F-182 | rings init                       | 4     | review-cli, review-ai-newcomer, review-gen-z, review-workflow-author |
| 19   | F-025 | Include Directory                | 3     | review-token-opt, review-prompt-eng, review-agent-ux |
| 19   | F-035 | Parse Warning Summary            | 3     | review-data-eng, review-token-opt, review-founder |
| 19   | F-071 | rings show                       | 3     | review-founder, review-enterprise, review-workflow-author |
| 19   | F-108 | summary.md                       | 3     | review-devops, review-data-eng, review-reliability |
| 19   | F-109 | Directory Permissions            | 3     | review-security, review-oss, review-enterprise |
| 19   | F-110 | Path Traversal Protection        | 3     | review-security, review-oss, review-enterprise |
| 19   | F-013 | Completion Signal Phase Restriction | 3  | review-token-opt, review-prompt-eng, review-workflow-author |
| 19   | F-091 | --force-lock                     | 3     | review-security, review-reliability, review-scripter |
| 19   | F-117 | File Manifest                    | 3     | review-data-eng, review-enterprise, review-agent-ux |

### Selected Batch

| F-NNN | Feature | Spec file |
|-------|---------|-----------|
| F-081 | --dry-run | specs/cli/commands-and-flags.md |
| F-056 | Stale Lock Detection | specs/state/cancellation-resume.md |
| F-070 | rings list | specs/cli/commands-and-flags.md |
| F-050 | Workflow File Change Detection | specs/state/cancellation-resume.md |
| F-029 | Unknown Variable Warnings | specs/execution/prompt-templating.md |
| F-149 | Cost Spike Detection | specs/execution/engine.md |
| F-049 | Resume State Recovery | specs/state/cancellation-resume.md |
| F-053 | Double Ctrl+C | specs/state/cancellation-resume.md |

### Notes

**F-081 (--dry-run)** led the ballot by 1 vote over the next two features, drawing support from every audience that cares about not spending money until they understand what they're running. The "preview before spending" concern was the single most consistent voice across persona types.

**F-056, F-050, F-049, F-053** are all in the same spec file (cancellation-resume.md) and address a coherent surface area: "rings behaves correctly when things go wrong." Implementing them together is efficient and eliminates the "stale lock left by a crashed process" failure mode entirely.

**F-029 (Unknown Variable Warnings)** is small scope and high-value: a single startup pass over prompt files. Six voters noticed it independently — mostly from the "catch bugs before spending money" angle. Easy win to include.

**F-149 (Cost Spike Detection)** is a runtime guardrail that doesn't require any new persistence; it's a check on existing cost data. Six voters (including three high-priority personas: devops, data-eng, agent-ux) called it out specifically for unattended workflows.

**Deferred with rationale:**
- **F-126 + F-127 + F-095 (JSONL triple)**: These 5 votes each form a tightly coupled unit that makes a better self-contained implementation batch. Deferring them keeps their shared interface coherent and avoids shipping F-126 without F-127 or the flag. Recommend as next batch.
- **F-070 + F-071 + F-072 (rings list/show/inspect)**: F-070 is in this batch. F-071 (3 votes) and F-072 (1 vote) can follow once F-070 is established.
- **F-182 (rings init)**: 4 votes but implementation depends on none of the selected features. Could be added to this batch without conflict; deferred for scope control. Strong candidate for the batch after next.
- **F-088 (--budget-cap)**: 4 votes and already PLANNED in the feature inventory (F-112). Excluded as PLANNED work is already in progress.
