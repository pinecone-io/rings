# Feature Inventory

Quick-reference lookup table of all rings features. Each row links to the spec file with full details.

Summaries are written from the user's perspective. Features with dependencies note them so you can plan implementation order.

**Status values:** `COMPLETE` — implemented and tested · `PLANNED` — scoped for current implementation batch · `PRIORITIZED` — elected to the work queue in `queues/PRIORITIZED_FEATURES.md` · `BACKLOG` — specified but not yet scheduled

## Workflow Definition

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-001 | Workflow File Format | I can describe a full iterative workflow in a single TOML file with phases, cycle limits, and a completion signal | COMPLETE | [workflow-file-format.md](workflow/workflow-file-format.md) |
| F-002 | Phase Configuration | I can define named phases, each with its own prompt source and how many times it runs per cycle | COMPLETE | [workflow-file-format.md](workflow/workflow-file-format.md) |
| F-003 | Inline Prompts | I can write a phase's prompt directly inside the TOML file without a separate prompt file | COMPLETE | [workflow-file-format.md](workflow/workflow-file-format.md) |
| F-004 | File-Based Prompts | I can keep prompts in separate files and reference them by path relative to the workflow file | COMPLETE | [workflow-file-format.md](workflow/workflow-file-format.md) |
| F-005 | Runs Per Cycle | I can configure each phase to run more than once per cycle | COMPLETE | [workflow-file-format.md](workflow/workflow-file-format.md) |
| F-006 | Max Cycles | I can set a hard cap on how many cycles rings will run before stopping, so it never loops forever | COMPLETE | [workflow-file-format.md](workflow/workflow-file-format.md) |
| F-007 | Context Directory | I can point all phases at a shared working directory so Claude's file changes are visible across the entire workflow | COMPLETE | [workflow-file-format.md](workflow/workflow-file-format.md) |
| F-008 | Output Directory Override | I can redirect where rings writes run logs and state, either globally or per-run | COMPLETE | [workflow-file-format.md](workflow/workflow-file-format.md) |
| F-009 | Phase Name Uniqueness | rings rejects my workflow at startup if two phases share a name, so I catch naming mistakes before any Claude calls | COMPLETE | [workflow-file-format.md](workflow/workflow-file-format.md) |
| F-010 | Cycle Model | rings runs all my phases in declaration order, then repeats from the top as a full cycle | COMPLETE | [cycle-model.md](workflow/cycle-model.md) |
| F-011 | Completion Signal | I can declare a string that, when detected in phase output, tells rings the workflow is done | COMPLETE | [cycle-model.md](workflow/cycle-model.md) |
| F-012 | Completion Signal Modes | I can match the completion signal by exact substring, line anchor, or full regex | PLANNED | [completion-detection.md](execution/completion-detection.md) |
| F-013 | Completion Signal Phase Restriction | I can limit which phases are allowed to trigger workflow completion so early phases can't accidentally end the run | PLANNED | [completion-detection.md](execution/completion-detection.md) |

## Phase Contracts

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-014 | Consumes Declaration | I can declare which files a phase reads so rings can warn me if those files don't exist yet | PLANNED | [phase-contracts.md](workflow/phase-contracts.md) |
| F-015 | Produces Declaration | I can declare which files a phase should write so rings warns me when nothing was actually created or changed | PLANNED | [phase-contracts.md](workflow/phase-contracts.md) |
| F-016 | Produces Required Flag | I can mark a phase's output as mandatory so rings halts with an error if it produces nothing | PLANNED | [phase-contracts.md](workflow/phase-contracts.md) |
| F-017 | Advisory Contract Warnings | rings tells me when a phase didn't meet its declared contract but keeps running so I can observe the issue | PLANNED | [phase-contracts.md](workflow/phase-contracts.md) |
| F-018 | Data Flow Documentation | I can see the declared and actual data flow for each phase when inspecting a run | PLANNED | [phase-contracts.md](workflow/phase-contracts.md) |

## Execution Engine

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-019 | Sequential Phase Execution | I can write phases that depend on each other's file output because rings never runs two phases simultaneously | COMPLETE | [engine.md](execution/engine.md) |
| F-020 | Timeout Per Run | I can set a per-run timeout so a hung Claude invocation doesn't stall my workflow indefinitely | PLANNED | [engine.md](execution/engine.md) |
| F-021 | Claude Code Integration | rings invokes Claude Code as a subprocess and passes my prompt securely over stdin | COMPLETE | [executor-integration.md](execution/executor-integration.md) |
| F-022 | Executor Abstraction | I can swap out the default `claude` binary for any other tool that accepts prompts over stdin | PRIORITIZED | [executor-integration.md](execution/executor-integration.md) |
| F-023 | Per-Phase Executors | I can use a different executor binary for individual phases within the same workflow | PRIORITIZED | [executor-integration.md](execution/executor-integration.md) |
| F-024 | Executor Binary Check | rings tells me before starting if the configured executor binary can't be found on PATH | COMPLETE | [executor-integration.md](execution/executor-integration.md) |
| F-025 | Include Directory | I can pass additional context directories whose file listings get prepended to each prompt | PRIORITIZED | [executor-integration.md](execution/executor-integration.md) |
| F-026 | Environment Variable Pass-Through | My shell environment variables are automatically available to the executor subprocess | PRIORITIZED | [executor-integration.md](execution/executor-integration.md) |

## Prompt Templating

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-027 | Prompt Templating | I can embed `{{variables}}` in my prompts that rings substitutes with live run context before each invocation | COMPLETE | [prompt-templating.md](execution/prompt-templating.md) |
| F-028 | Template Variables | I have access to `{{phase_name}}`, `{{cycle}}`, `{{max_cycles}}`, `{{iteration}}`, `{{run}}`, and `{{cost_so_far_usd}}` in prompts | COMPLETE | [prompt-templating.md](execution/prompt-templating.md) |
| F-029 | Unknown Variable Warnings | rings warns me at startup if my prompts reference variables it doesn't recognize, before any Claude calls happen | PRIORITIZED | [prompt-templating.md](execution/prompt-templating.md) |

## Output Parsing

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-030 | Cost Extraction | rings automatically parses cost information from Claude's output so I can track spend without manual inspection | COMPLETE | [output-parsing.md](execution/output-parsing.md) |
| F-031 | Custom Cost Parser | I can provide a custom regex to extract cost from non-standard executor output formats | PRIORITIZED | [output-parsing.md](execution/output-parsing.md) |
| F-032 | Token Counting | rings extracts input and output token counts from each run so I can see detailed usage | COMPLETE | [output-parsing.md](execution/output-parsing.md) |
| F-033 | Parse Confidence Levels | rings tells me how confident it is in each parsed cost value (Full, Partial, Low, or None) so I know when to distrust numbers | COMPLETE | [output-parsing.md](execution/output-parsing.md) |
| F-034 | Resume Command Extraction | rings captures any `claude resume` command from executor output so I can manually restart a Claude session if needed | COMPLETE | [output-parsing.md](execution/output-parsing.md) |
| F-035 | Parse Warning Summary | At the end of a run, I see a consolidated summary of any cost parsing failures with raw output snippets | PRIORITIZED | [output-parsing.md](execution/output-parsing.md) |
| F-036 | Warning Deduplication | Repeated parse failures for the same pattern are collapsed into one warning instead of flooding my terminal (requires F-035) | PRIORITIZED | [output-parsing.md](execution/output-parsing.md) |

## Error Handling

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-037 | Error Classification | rings categorizes executor failures as Quota, Auth, or Unknown so I know whether the problem is recoverable | PRIORITIZED | [error-handling.md](execution/error-handling.md) |
| F-038 | Quota Error Detection | rings automatically recognizes quota exhaustion messages from the executor output (requires F-037) | PRIORITIZED | [error-handling.md](execution/error-handling.md) |
| F-039 | Auth Error Detection | rings automatically recognizes authentication failure messages so I know to check my credentials (requires F-037) | PRIORITIZED | [error-handling.md](execution/error-handling.md) |
| F-040 | Custom Error Profiles | I can define my own patterns for what counts as a quota or auth error when using a custom executor (requires F-037) | PRIORITIZED | [error-handling.md](execution/error-handling.md) |

## Rate Limiting

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-041 | Run-to-Run Delay | I can add a fixed pause between individual phase runs to stay within API rate limits | COMPLETE | [rate-limiting.md](execution/rate-limiting.md) |
| F-042 | Cycle-to-Cycle Delay | I can add a fixed pause between full cycles to spread out API usage over time | PRIORITIZED | [rate-limiting.md](execution/rate-limiting.md) |
| F-043 | Duration String Parsing | I can write delays as human-readable strings like "30s", "5m", or "1h" instead of raw milliseconds | PRIORITIZED | [rate-limiting.md](execution/rate-limiting.md) |
| F-044 | Quota Backoff | I can tell rings to automatically wait and retry when it hits a quota error instead of stopping (requires F-038) | PRIORITIZED | [rate-limiting.md](execution/rate-limiting.md) |
| F-045 | Quota Backoff Max Retries | I can set a cap on how many times rings will retry after quota errors before giving up (requires F-044) | PRIORITIZED | [rate-limiting.md](execution/rate-limiting.md) |

## State & Resumability

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-046 | State Persistence | rings saves my exact execution position after every completed run so nothing is lost on interruption | COMPLETE | [cancellation-resume.md](state/cancellation-resume.md) |
| F-047 | Atomic State Writes | rings writes state atomically so a crash mid-write can't leave me with a corrupted position file | COMPLETE | [cancellation-resume.md](state/cancellation-resume.md) |
| F-048 | Resumable Runs | I can press Ctrl+C to stop a workflow and later resume from exactly where it left off (requires F-046) | COMPLETE | [cancellation-resume.md](state/cancellation-resume.md) |
| F-049 | Resume State Recovery | If my state file is corrupted, rings can reconstruct my position from audit logs (requires F-046, F-068) | PRIORITIZED | [cancellation-resume.md](state/cancellation-resume.md) |
| F-050 | Workflow File Change Detection | rings refuses to resume if I've made structural changes to the workflow since the last run, protecting me from mismatched state | PRIORITIZED | [cancellation-resume.md](state/cancellation-resume.md) |
| F-051 | SIGINT Handling | Pressing Ctrl+C gracefully saves state and prints a resume command before exiting (requires F-046) | COMPLETE | [cancellation-resume.md](state/cancellation-resume.md) |
| F-052 | SIGTERM Handling | rings treats SIGTERM like Ctrl+C so process managers can stop it cleanly (requires F-051) | PLANNED | [cancellation-resume.md](state/cancellation-resume.md) |
| F-053 | Double Ctrl+C | A second Ctrl+C while rings is waiting skips the graceful shutdown and force-kills the subprocess immediately (requires F-051) | PRIORITIZED | [cancellation-resume.md](state/cancellation-resume.md) |
| F-054 | Subprocess Graceful Shutdown | rings sends SIGTERM to the executor and waits 5 seconds before escalating to SIGKILL | PLANNED | [cancellation-resume.md](state/cancellation-resume.md) |
| F-055 | Context Directory Lock | rings prevents two instances from running against the same context_dir at the same time | PLANNED | [cancellation-resume.md](state/cancellation-resume.md) |
| F-056 | Stale Lock Detection | rings automatically removes a lock from a process that is no longer running (requires F-055) | PRIORITIZED | [cancellation-resume.md](state/cancellation-resume.md) |
| F-057 | Cross-Machine Resume Limitation | rings documents that resume requires the workflow file at the same absolute path; `--parent-run` is available for cross-machine linking | PRIORITIZED | [cancellation-resume.md](state/cancellation-resume.md) |
| F-058 | Parent Run ID | rings records which run was the parent when I resume or use `--parent-run`, building an ancestry chain | PRIORITIZED | [run-ancestry.md](state/run-ancestry.md) |
| F-059 | Ancestry Depth Tracking | rings tracks how many resumptions deep a run is (requires F-058) | PRIORITIZED | [run-ancestry.md](state/run-ancestry.md) |
| F-060 | Continuation Linking | I can use `--parent-run` to link a fresh run to a prior one without resuming its saved state (requires F-058) | PRIORITIZED | [run-ancestry.md](state/run-ancestry.md) |

## Configuration

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-061 | User Config File | I can set personal defaults in `~/.config/rings/config.toml` that apply to all my workflows | PRIORITIZED | [configuration.md](state/configuration.md) |
| F-062 | Project Config File | I can check a `.rings-config.toml` into my project to share team-level defaults | PRIORITIZED | [configuration.md](state/configuration.md) |
| F-063 | Config Precedence | I always know which value wins: CLI flags beat env vars beat workflow TOML beat project config beat user config | PRIORITIZED | [configuration.md](state/configuration.md) |
| F-064 | XDG Base Directory | rings follows XDG so my config and data land in standard locations alongside my other tools | PRIORITIZED | [configuration.md](state/configuration.md) |
| F-065 | Default Output Directory | I can set a global default for where all run output is written instead of specifying it every time (requires F-061) | PRIORITIZED | [configuration.md](state/configuration.md) |
| F-066 | Default Executor Config | I can define executor defaults in my workflow TOML that apply to all phases unless overridden | PRIORITIZED | [configuration.md](state/configuration.md) |
| F-067 | Config File Trust Warning | rings warns me when it loads a `.rings-config.toml` from the current directory in case I ran rings somewhere unexpected (requires F-062) | PRIORITIZED | [configuration.md](state/configuration.md) |

## CLI Commands

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-068 | `rings run` | I can start a new workflow execution with `rings run <workflow.toml>` | COMPLETE | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-069 | `rings resume` | I can resume an interrupted workflow from its last completed step with `rings resume <run-id>` (requires F-048) | COMPLETE | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-070 | `rings list` | I can see all recent runs with their status and total cost in a summary table | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-071 | `rings show` | I can get a single-screen summary of any past run by its ID | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-072 | `rings inspect` | I can deeply inspect any run with multiple views: summary, cycles, files, costs, and raw output | PRIORITIZED | [inspect-command.md](cli/inspect-command.md) |
| F-073 | `rings lineage` | I can see the full chain of parent/child runs that led to any given run ID (requires F-058) | PRIORITIZED | [inspect-command.md](cli/inspect-command.md) |
| F-074 | `rings cleanup` | I can remove old run data to free disk space | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-075 | `rings completions` | I can generate shell completion scripts for bash, zsh, or fish with `rings completions <shell>` | PRIORITIZED | [completion-and-manpage.md](cli/completion-and-manpage.md) |

## CLI Flags

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-076 | `--max-cycles` | I can override the workflow's max_cycles for a single run without editing the file | COMPLETE | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-077 | `--output-dir` | I can redirect this run's output to a specific directory | COMPLETE | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-078 | `--include-dir` | I can inject additional file-listing context into prompts for this run (requires F-025) | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-079 | `--delay` | I can set or override the between-run delay for this run without editing the workflow file (requires F-041) | COMPLETE | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-080 | `--cycle-delay` | I can set or override the between-cycle delay for this run without editing the workflow file (requires F-042) | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-081 | `--dry-run` | I can preview the full execution plan — phases, prompts, delays — without any Claude calls | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-082 | `--step` | I can pause after every individual run to inspect output before letting rings continue | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-083 | `--step-cycles` | I can pause only at cycle boundaries for a less granular step-through experience (requires F-082) | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-084 | `--verbose` | I can stream the executor's live output to my terminal alongside rings' status display | COMPLETE | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-085 | `--quota-backoff` | I can enable automatic quota-error retry at the command line without changing the workflow file (requires F-044) | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-086 | `--quota-backoff-delay` | I can set how long rings waits before retrying after a quota error (requires F-044) | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-087 | `--quota-backoff-max-retries` | I can cap how many quota retries rings will attempt before giving up (requires F-044, F-045) | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-088 | `--budget-cap` | I can set a spending limit for this run so rings stops and saves state if cost exceeds it (requires F-112) | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-089 | `--strict-parsing` | I can make rings treat any cost parsing failure as a hard error that stops the run (requires F-033) | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-090 | `--parent-run` | I can explicitly link this run to a prior one for ancestry tracking without resuming its state (requires F-058) | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-091 | `--force-lock` | I can override the context_dir lock check when I know the previous process is truly gone (requires F-055) | PRIORITIZED | [cancellation-resume.md](state/cancellation-resume.md) |
| F-092 | `--no-completion-check` | I can suppress the startup warning about missing completion signals in prompts (requires F-011) | COMPLETE | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-093 | `--no-contract-check` | I can suppress phase contract violation warnings for a run (requires F-014, F-015) | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-094 | `--no-color` | I can disable colored terminal output | PLANNED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-095 | `--output-format` | I can switch between human-readable and JSONL output for the same run | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
| F-096 | `--no-sensitive-files-check` | I can suppress the warning about credential files in context_dir when I know they're intentionally there | PRIORITIZED | [engine.md](execution/engine.md) |

## Inspect Command Views

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-097 | Summary View | I can see a one-screen overview of a run: total cycles, cost, and files changed (requires F-072) | PRIORITIZED | [inspect-command.md](cli/inspect-command.md) |
| F-098 | Cycles View | I can drill into a per-cycle breakdown showing each run's status and whether the completion signal fired (requires F-072) | PRIORITIZED | [inspect-command.md](cli/inspect-command.md) |
| F-099 | Files Changed View | I can see exactly which files changed in each run, attributed by phase and cycle (requires F-072, F-117) | PRIORITIZED | [inspect-command.md](cli/inspect-command.md) |
| F-100 | Data Flow View | I can see declared vs. actual file inputs and outputs for each phase (requires F-072, F-014, F-015, F-117) | PRIORITIZED | [inspect-command.md](cli/inspect-command.md) |
| F-101 | Costs View | I can see a detailed cost and token breakdown for every individual run (requires F-072, F-030) | PRIORITIZED | [inspect-command.md](cli/inspect-command.md) |
| F-102 | Claude Output View | I can read the raw stdout/stderr from any executor invocation inside `rings inspect` (requires F-072, F-106) | PRIORITIZED | [inspect-command.md](cli/inspect-command.md) |

## Observability & Audit Logs

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-103 | Run ID Generation | Every run gets a unique ID combining a timestamp and random suffix so I can always identify it | COMPLETE | [audit-logs.md](observability/audit-logs.md) |
| F-104 | Output Directory Structure | rings organizes all run data in a predictable hierarchy: run.toml, state.json, runs/, costs.jsonl | COMPLETE | [audit-logs.md](observability/audit-logs.md) |
| F-105 | run.toml Metadata | rings writes a machine-readable record of my workflow path, start time, rings version, and final status for each run | COMPLETE | [audit-logs.md](observability/audit-logs.md) |
| F-106 | Per-Run Log Files | rings captures the full stdout/stderr of every executor invocation to individual log files I can read later | COMPLETE | [audit-logs.md](observability/audit-logs.md) |
| F-107 | costs.jsonl | rings appends a cost record for each run to a newline-delimited JSON file I can stream-process with standard tools | COMPLETE | [audit-logs.md](observability/audit-logs.md) |
| F-108 | summary.md | rings generates a human-readable markdown summary of the completed run automatically | PRIORITIZED | [audit-logs.md](observability/audit-logs.md) |
| F-109 | Directory Permissions | rings creates my output directory with mode 0700 so only I can read run logs and cost data | PRIORITIZED | [audit-logs.md](observability/audit-logs.md) |
| F-110 | Path Traversal Protection | rings rejects any output_dir value containing `..` so a malicious workflow can't write outside the intended directory | PRIORITIZED | [audit-logs.md](observability/audit-logs.md) |

## Cost Tracking

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-111 | Real-Time Cost Accumulation | I can see cumulative cost grow in real time as each run completes, both per-phase and globally | COMPLETE | [cost-tracking.md](observability/cost-tracking.md) |
| F-112 | Budget Cap | I can set a spending ceiling so rings automatically stops and saves state when the cost limit is hit (requires F-046) | PLANNED | [cost-tracking.md](observability/cost-tracking.md) |
| F-113 | Budget Warning Thresholds | rings warns me when I've reached 80% and 90% of my budget cap so I'm not surprised by a stop (requires F-112) | PRIORITIZED | [cost-tracking.md](observability/cost-tracking.md) |
| F-114 | Per-Phase Budget Caps | I can set independent spending limits on individual phases to protect against a runaway single phase (requires F-112) | PRIORITIZED | [cost-tracking.md](observability/cost-tracking.md) |
| F-115 | Low-Confidence Cost Warning | rings warns me any time it can only partially or not at all parse cost from executor output (requires F-033) | PLANNED | [cost-tracking.md](observability/cost-tracking.md) |
| F-116 | No Budget Cap Warning | rings warns me at startup if I haven't set any budget cap, so I don't accidentally run an unbounded workflow | PLANNED | [cost-tracking.md](observability/cost-tracking.md) |

## File Lineage

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-117 | File Manifest | rings records SHA256 fingerprints of every file in context_dir before and after each run | PRIORITIZED | [file-lineage.md](observability/file-lineage.md) |
| F-118 | File Diff Detection | rings computes exactly which files were added, modified, or deleted by each run (requires F-117) | PRIORITIZED | [file-lineage.md](observability/file-lineage.md) |
| F-119 | File Manifest Ignore Patterns | I can tell rings to skip certain directories (e.g. `.git/`, `target/`) from file tracking (requires F-117) | PRIORITIZED | [file-lineage.md](observability/file-lineage.md) |
| F-120 | Credential File Protection | rings always excludes `.env`, `*.key`, `*.pem`, and similar files from manifests regardless of my ignore patterns (requires F-117) | PRIORITIZED | [file-lineage.md](observability/file-lineage.md) |
| F-121 | mtime Optimization | rings skips re-hashing files whose modification time hasn't changed, keeping large repos fast (requires F-117) | PRIORITIZED | [file-lineage.md](observability/file-lineage.md) |
| F-122 | Cycle Snapshots | rings can copy my entire context_dir at each cycle boundary so I can roll back to any prior cycle (requires F-117) | PRIORITIZED | [file-lineage.md](observability/file-lineage.md) |
| F-123 | Snapshot Storage Warning | rings estimates snapshot storage usage at startup and warns me if it will be unexpectedly large (requires F-122) | PRIORITIZED | [file-lineage.md](observability/file-lineage.md) |
| F-124 | Manifest Compression | rings stores file manifests as gzip-compressed JSON to keep disk usage low (requires F-117) | PRIORITIZED | [file-lineage.md](observability/file-lineage.md) |

## Runtime Output

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-125 | Human Output Mode | I see colored, spinner-animated terminal output that shows what rings is doing at a glance | PLANNED | [runtime-output.md](observability/runtime-output.md) |
| F-126 | JSONL Output Mode | I can switch to newline-delimited JSON events for scripting, CI, or piping into other tools | PRIORITIZED | [runtime-output.md](observability/runtime-output.md) |
| F-127 | stderr/stdout Separation | Human-readable output goes to stderr; JSONL events go to stdout so I can pipe them cleanly | PRIORITIZED | [runtime-output.md](observability/runtime-output.md) |
| F-128 | Status Line Display | I see a single updating line showing current cycle, phase name, and running cost | COMPLETE | [runtime-output.md](observability/runtime-output.md) |
| F-129 | Animated Spinner | A spinner next to the status line confirms rings is alive even during long Claude invocations | PLANNED | [runtime-output.md](observability/runtime-output.md) |
| F-130 | Phase Transition Lines | rings prints a clear line whenever it moves to a new phase or cycle, including per-cycle cost | COMPLETE | [runtime-output.md](observability/runtime-output.md) |
| F-131 | Startup Header | rings prints my workflow's phases, max cycles, and key settings when it starts so I can confirm before any calls happen | COMPLETE | [runtime-output.md](observability/runtime-output.md) |
| F-132 | Completion Summary | rings prints a final breakdown of total cost, token counts, and cycle statistics when the workflow finishes | COMPLETE | [runtime-output.md](observability/runtime-output.md) |
| F-133 | Cancellation Summary | When I press Ctrl+C, rings prints what completed and the exact command to resume (requires F-051) | COMPLETE | [runtime-output.md](observability/runtime-output.md) |
| F-134 | Claude Resume Command Display | rings surfaces any captured `claude resume` command in the terminal so I can recover an interactive session (requires F-034) | COMPLETE | [runtime-output.md](observability/runtime-output.md) |
| F-135 | Verbose Mode Output | With `--verbose`, I can see the executor's raw output interleaved with rings' own status | COMPLETE | [runtime-output.md](observability/runtime-output.md) |
| F-136 | Step-Through Mode | With `--step`, I'm prompted after each run and can continue, skip a cycle, view output, or quit | PRIORITIZED | [runtime-output.md](observability/runtime-output.md) |
| F-137 | Step-Cycles Mode | With `--step-cycles`, I'm only prompted at cycle boundaries rather than after every run (requires F-136) | PRIORITIZED | [runtime-output.md](observability/runtime-output.md) |
| F-138 | Step Summary Display | At each step-through pause, rings shows cost so far, files changed, and whether the completion signal was detected (requires F-136) | PRIORITIZED | [runtime-output.md](observability/runtime-output.md) |
| F-139 | JSONL Event Envelope | Every JSONL event includes `run_id` and `timestamp` so I can correlate events across tools (requires F-126) | PRIORITIZED | [runtime-output.md](observability/runtime-output.md) |
| F-140 | JSONL Event Types | rings emits structured events for start, run_start, run_end, completion_signal, executor_error, delays, budget_cap, and summary (requires F-126) | PRIORITIZED | [runtime-output.md](observability/runtime-output.md) |

## Startup Validation & Advisory Checks

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-141 | Startup Validation | rings validates my workflow file's syntax and required fields before making any Claude calls | COMPLETE | [engine.md](execution/engine.md) |
| F-142 | Prompt File Existence Check | rings fails fast with a clear error if any referenced prompt file is missing or unreadable | COMPLETE | [engine.md](execution/engine.md) |
| F-143 | Context Directory Validation | rings verifies my context_dir exists and is readable before starting | COMPLETE | [engine.md](execution/engine.md) |
| F-144 | Empty Context Directory Warning | rings warns me if context_dir has no files in case I pointed it at the wrong directory | PRIORITIZED | [engine.md](execution/engine.md) |
| F-145 | Sensitive Files Warning | rings warns me if context_dir contains credentials (`.env`, `*.key`, `*.pem`) that could be exposed to the model | PRIORITIZED | [engine.md](execution/engine.md) |
| F-146 | Output Directory Inside Repo Warning | rings warns me if my output_dir is inside a git repo and would get committed accidentally | PRIORITIZED | [engine.md](execution/engine.md) |
| F-147 | Disk Space Check | rings warns at < 100 MB free and aborts at < 10 MB so I don't silently lose run data | PRIORITIZED | [engine.md](execution/engine.md) |
| F-148 | Delay Sanity Warning | rings warns me if `delay_between_runs` exceeds 600 seconds, since that's likely a units mistake | PRIORITIZED | [engine.md](execution/engine.md) |
| F-149 | Cost Spike Detection | rings warns me mid-run when a single run costs 5× more than the rolling 5-run average | PRIORITIZED | [engine.md](execution/engine.md) |
| F-150 | No-Files-Changed Streak Warning | rings warns me after 3 consecutive runs where the declared produces files weren't changed, suggesting the workflow is stuck | PRIORITIZED | [engine.md](execution/engine.md) |
| F-151 | Completion Signal Presence Check | rings warns at startup if my completion signal string doesn't appear in any prompt, so I catch typos before spending money (requires F-011) | COMPLETE | [completion-detection.md](execution/completion-detection.md) |
| F-152 | Consumes File Validation | rings warns me before a run if a phase's declared input files don't exist yet (requires F-014) | PRIORITIZED | [phase-contracts.md](workflow/phase-contracts.md) |
| F-153 | Produces File Validation | rings warns me after a run if a phase's declared output files weren't actually written (requires F-015) | PRIORITIZED | [phase-contracts.md](workflow/phase-contracts.md) |
| F-154 | Large Context Directory Warning | rings warns me if context_dir has > 10,000 files because manifest scanning will be slow (requires F-117) | PRIORITIZED | [file-lineage.md](observability/file-lineage.md) |

## Exit Codes

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-155 | Exit Code 0 | rings exits 0 when the completion signal is detected so my scripts can reliably detect success | COMPLETE | [exit-codes.md](cli/exit-codes.md) |
| F-156 | Exit Code 1 | rings exits 1 when max_cycles completes without a signal, distinguishing "ran out of cycles" from errors | COMPLETE | [exit-codes.md](cli/exit-codes.md) |
| F-157 | Exit Code 2 | rings exits 2 for configuration errors (bad TOML, missing files, executor not found) that need my attention | COMPLETE | [exit-codes.md](cli/exit-codes.md) |
| F-158 | Exit Code 3 | rings exits 3 for executor errors (quota, auth, unknown) and saves state so I can resume after fixing the issue | COMPLETE | [exit-codes.md](cli/exit-codes.md) |
| F-159 | Exit Code 4 | rings exits 4 when the budget cap is hit and saves state so I can resume after reviewing spend (requires F-112) | PRIORITIZED | [exit-codes.md](cli/exit-codes.md) |
| F-160 | Exit Code 130 | rings exits 130 on Ctrl+C or SIGTERM, matching the standard Unix convention for signal termination | COMPLETE | [exit-codes.md](cli/exit-codes.md) |
| F-161 | Error Output to stderr | All error messages go to stderr so stdout remains clean for JSONL piping | COMPLETE | [exit-codes.md](cli/exit-codes.md) |

## OpenTelemetry Integration

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-162 | OTel Opt-In | OpenTelemetry tracing is off by default; I enable it by setting `RINGS_OTEL_ENABLED=1` | PRIORITIZED | [opentelemetry.md](observability/opentelemetry.md) |
| F-163 | OTel Trace Structure | I get one trace per workflow run with a clean hierarchy: root span → cycle spans → phase-run spans (requires F-162) | PRIORITIZED | [opentelemetry.md](observability/opentelemetry.md) |
| F-164 | Span Attributes | Each span carries run metadata, phase name, cost, and file change counts so I can filter in my observability platform (requires F-163) | PRIORITIZED | [opentelemetry.md](observability/opentelemetry.md) |
| F-165 | Span Status | Spans are marked ERROR on non-zero executor exit so I can alert on failures in my tracing tool (requires F-163) | PRIORITIZED | [opentelemetry.md](observability/opentelemetry.md) |
| F-166 | Span Links | When I resume a run, the new trace is linked to the parent run's trace so I can navigate the full history (requires F-163, F-058) | PRIORITIZED | [run-ancestry.md](state/run-ancestry.md) |
| F-167 | OTel Metrics | rings emits counters and histograms for cost, duration, and token counts so I can dashboard and alert on them (requires F-162) | PRIORITIZED | [opentelemetry.md](observability/opentelemetry.md) |
| F-168 | OTel Path Stripping | I can set `RINGS_OTEL_STRIP_PATHS=1` to redact filesystem paths from telemetry for privacy (requires F-162) | PRIORITIZED | [opentelemetry.md](observability/opentelemetry.md) |
| F-169 | OTel Init Failure Handling | If the OTel exporter fails to initialize, rings continues with a no-op tracer instead of aborting (requires F-162) | PRIORITIZED | [opentelemetry.md](observability/opentelemetry.md) |
| F-170 | OTel Endpoint Configuration | I configure my collector endpoint via the standard `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable (requires F-162) | PRIORITIZED | [opentelemetry.md](observability/opentelemetry.md) |

## Distribution & Shell Integration

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-171 | Static Binary | I can download a single binary with no system library dependencies and run it immediately | PRIORITIZED | [distribution.md](cli/distribution.md) |
| F-172 | Multi-Platform Release | I can get native binaries for x86_64 and aarch64 on both Linux and macOS | PRIORITIZED | [distribution.md](cli/distribution.md) |
| F-173 | macOS Universal Binary | On macOS, I get a single universal binary that runs natively on both Intel and Apple Silicon | PRIORITIZED | [distribution.md](cli/distribution.md) |
| F-174 | Binary Size Optimization | The rings binary targets < 5 MB so downloads and distributions stay lightweight | PRIORITIZED | [distribution.md](cli/distribution.md) |
| F-175 | Cargo Install Support | Rust users can install rings with `cargo install rings` without needing pre-built binaries | PRIORITIZED | [distribution.md](cli/distribution.md) |
| F-176 | SHA256 Checksums | Every release includes checksums I can verify to confirm binary integrity | PRIORITIZED | [distribution.md](cli/distribution.md) |
| F-177 | Reproducible Builds | The Rust toolchain is pinned and Cargo.lock is committed so I can reproduce any release binary myself | PRIORITIZED | [distribution.md](cli/distribution.md) |
| F-178 | Shell Completions | I can get tab-completion for all commands and flags in bash, zsh, or fish (requires F-075) | PRIORITIZED | [completion-and-manpage.md](cli/completion-and-manpage.md) |
| F-179 | Completion Behavior | Tab-completion offers `.toml` files for workflow arguments, run IDs for run arguments, and flag names everywhere (requires F-178) | PRIORITIZED | [completion-and-manpage.md](cli/completion-and-manpage.md) |
| F-180 | Man Page | I can read `man rings` for offline documentation generated from the same source as `--help` | PRIORITIZED | [completion-and-manpage.md](cli/completion-and-manpage.md) |

## Runtime Output — Visual Enhancement

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-183 | ANSI Color System | rings uses a semantic color palette (green success, red errors, cyan costs, dim chrome) gated behind NO_COLOR and TTY detection | PLANNED | [runtime-output.md](observability/runtime-output.md) |
| F-184 | Phase Cost Bar Chart | Completion and cancellation summaries show a proportional bar chart of cost distribution across phases | PLANNED | [runtime-output.md](observability/runtime-output.md) |
| F-185 | Budget Gauge | When a budget cap is configured, summaries show a visual gauge of budget consumption with color-coded thresholds | PLANNED | [runtime-output.md](observability/runtime-output.md) |
| F-186 | Styled Startup Header | The startup header shows workflow details in a clean, labeled layout with semantic coloring | PLANNED | [runtime-output.md](observability/runtime-output.md) |
| F-187 | Styled Cycle Transitions | Cycle boundaries show a horizontal rule with the cycle number and previous cycle cost embedded | PLANNED | [runtime-output.md](observability/runtime-output.md) |
| F-188 | Styled List Table | `rings list` output uses color-coded status, bold headers, and accent cost figures | PLANNED | [runtime-output.md](observability/runtime-output.md) |
| F-189 | Styled Dry Run Output | `rings run --dry-run` uses the same color system as live runs for visual consistency | PLANNED | [runtime-output.md](observability/runtime-output.md) |

## Executor Args Ergonomics

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-181 | Per-Phase Model Selection via `executor.extra_args` | I can set `executor.extra_args` on any phase to append flags (e.g. `--model claude-haiku-4-5`) to the inherited executor args, so I can route cheap phases to smaller models without re-specifying all base flags | PRIORITIZED | [executor-integration.md](execution/executor-integration.md) |

## Workflow Scaffolding

| # | Feature | Summary | Status | Spec |
|---|---------|---------|--------|------|
| F-182 | `rings init` | I can scaffold a new, immediately runnable workflow TOML file with `rings init [NAME]` so I don't have to write boilerplate by hand | PRIORITIZED | [commands-and-flags.md](cli/commands-and-flags.md) |
