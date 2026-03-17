# Prioritized Feature Queue

Features are listed in implementation priority order. The top of this list is what the next
`/replan` batch should draw from.

**Status values in this file:**
- Features listed here have status `PRIORITIZED` in `specs/feature_inventory.md`
- When a feature moves into a `/replan` batch it becomes `PLANNED`
- When implemented and tested it becomes `COMPLETE`

**How to use:**
1. Run `rings run rings/plan-prioritize/plan-prioritize.rings.toml` to populate this queue (or extend it)
2. Run `rings run rings/plan-create/plan-create.rings.toml` to draft implementation plans from the top entries
3. After implementation, mark features `COMPLETE` in `specs/feature_inventory.md`

---

<!-- Election cycles append entries below this line -->

### Priority 1: F-037 — Error Classification

- **Summary:** rings categorizes executor failures as Quota, Auth, or Unknown so I know whether the problem is recoverable
- **Spec:** [error-handling.md](specs/execution/error-handling.md)
- **Unblocks:** F-038 (Quota Error Detection), F-039 (Auth Error Detection), F-040 (Custom Error Profiles), and transitively F-044 (Quota Backoff), F-085, F-086, F-087

---

### Priority 2: F-117 — File Manifest

- **Summary:** rings records SHA256 fingerprints of every file in context_dir before and after each run
- **Spec:** [file-lineage.md](specs/observability/file-lineage.md)
- **Unblocks:** F-118 (File Diff Detection), F-119 (File Manifest Ignore Patterns), F-120 (Credential File Protection), F-121 (mtime Optimization), F-122 (Cycle Snapshots), F-124 (Manifest Compression), F-154 (Large Context Directory Warning)

---

### Priority 3: F-072 — `rings inspect`

- **Summary:** I can deeply inspect any run with multiple views: summary, cycles, files, costs, and raw output
- **Spec:** [inspect-command.md](specs/cli/inspect-command.md)
- **Unblocks:** F-097 (Summary View), F-098 (Cycles View), F-099 (Files Changed View), F-100 (Data Flow View), F-101 (Costs View), F-102 (Claude Output View)

---

### Priority 4: F-038 — Quota Error Detection

- **Summary:** rings automatically recognizes quota exhaustion messages from the executor output
- **Spec:** [error-handling.md](specs/execution/error-handling.md)
- **Unblocks:** F-044 (Quota Backoff), and transitively F-045 (Quota Backoff Max Retries), F-085 (`--quota-backoff`), F-086 (`--quota-backoff-delay`), F-087 (`--quota-backoff-max-retries`)

---

### Priority 5: F-097 — Summary View

- **Summary:** I can see a one-screen overview of a run: total cycles, cost, and files changed
- **Spec:** [inspect-command.md](specs/cli/inspect-command.md)
- **Unblocks:** none

---

### Priority 6: F-118 — File Diff Detection

- **Summary:** rings computes exactly which files were added, modified, or deleted by each run
- **Spec:** [file-lineage.md](specs/observability/file-lineage.md)
- **Unblocks:** none directly listed; contributes data for F-099 (Files Changed View), F-138 (Step Summary Display), F-150 (No-Files-Changed Streak Warning)

---

### Priority 7: F-044 — Quota Backoff

- **Summary:** I can tell rings to automatically wait and retry when it hits a quota error instead of stopping
- **Spec:** [rate-limiting.md](specs/execution/rate-limiting.md)
- **Unblocks:** F-045 (Quota Backoff Max Retries), F-085 (`--quota-backoff`), F-086 (`--quota-backoff-delay`), F-087 (`--quota-backoff-max-retries`)

---

### Priority 8: F-058 — Parent Run ID

- **Summary:** rings records which run was the parent when I resume or use `--parent-run`, building an ancestry chain
- **Spec:** [run-ancestry.md](specs/state/run-ancestry.md)
- **Unblocks:** F-059 (Ancestry Depth Tracking), F-060 (Continuation Linking), F-073 (`rings lineage`), F-090 (`--parent-run`), F-166 (Span Links)

---

### Priority 9: F-039 — Auth Error Detection

- **Summary:** rings automatically recognizes authentication failure messages so I know to check my credentials
- **Spec:** [error-handling.md](specs/execution/error-handling.md)
- **Unblocks:** F-040 (Custom Error Profiles)

---

### Priority 10: F-014 — Consumes Declaration

- **Summary:** I can declare which files a phase reads so rings can warn me if those files don't exist yet
- **Spec:** [phase-contracts.md](specs/workflow/phase-contracts.md)
- **Unblocks:** F-152 (Consumes File Validation); co-unlocks F-017 (Advisory Contract Warnings), F-018 (Data Flow Documentation), F-093 (`--no-contract-check`), F-100 (Data Flow View) alongside F-015

---

### Priority 11: F-015 — Produces Declaration

- **Summary:** I can declare which files a phase should write so rings warns me when nothing was actually created or changed
- **Spec:** [phase-contracts.md](specs/workflow/phase-contracts.md)
- **Unblocks:** F-016 (Produces Required Flag), F-153 (Produces File Validation); co-unlocks F-017 (Advisory Contract Warnings), F-018 (Data Flow Documentation), F-093 (`--no-contract-check`), F-100 (Data Flow View) alongside F-014

---

### Priority 12: F-126 — JSONL Output Mode

- **Summary:** I can switch to newline-delimited JSON events for scripting, CI, or piping into other tools
- **Spec:** [runtime-output.md](specs/observability/runtime-output.md)
- **Unblocks:** F-127 (stderr/stdout Separation), F-139 (JSONL Event Envelope), F-140 (JSONL Event Types)

---

### Priority 13: F-139 — JSONL Event Envelope

- **Summary:** Every JSONL event includes `run_id` and `timestamp` so I can correlate events across tools
- **Spec:** [runtime-output.md](specs/observability/runtime-output.md)
- **Unblocks:** none

---

### Priority 14: F-140 — JSONL Event Types

- **Summary:** rings emits structured events for start, run_start, run_end, completion_signal, executor_error, delays, budget_cap, and summary
- **Spec:** [runtime-output.md](specs/observability/runtime-output.md)
- **Unblocks:** none

---

### Priority 15: F-136 — Step-Through Mode

- **Summary:** With `--step`, I'm prompted after each run and can continue, skip a cycle, view output, or quit
- **Spec:** [runtime-output.md](specs/observability/runtime-output.md)
- **Unblocks:** F-137 (Step-Cycles Mode), F-138 (Step Summary Display)

---

### Priority 16: F-162 — OTel Opt-In

- **Summary:** OpenTelemetry tracing is off by default; I enable it by setting `RINGS_OTEL_ENABLED=1`
- **Spec:** [opentelemetry.md](specs/observability/opentelemetry.md)
- **Unblocks:** F-163 (OTel Trace Structure), F-167 (OTel Metrics), F-168 (OTel Path Stripping), F-169 (OTel Init Failure Handling), F-170 (OTel Endpoint Configuration); transitively F-164, F-165, F-166

---

### Priority 17: F-071 — `rings show`

- **Summary:** I can get a single-screen summary of any past run by its ID
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 18: F-042 — Cycle-to-Cycle Delay

- **Summary:** I can add a fixed pause between full cycles to spread out API usage over time
- **Spec:** [rate-limiting.md](specs/execution/rate-limiting.md)
- **Unblocks:** F-080 (`--cycle-delay`)

---

### Priority 19: F-163 — OTel Trace Structure

- **Summary:** I get one trace per workflow run with a clean hierarchy: root span → cycle spans → phase-run spans
- **Spec:** [opentelemetry.md](specs/observability/opentelemetry.md)
- **Unblocks:** F-164 (Span Attributes), F-165 (Span Status), F-166 (Span Links)

---

### Priority 20: F-098 — Cycles View

- **Summary:** I can drill into a per-cycle breakdown showing each run's status and whether the completion signal fired
- **Spec:** [inspect-command.md](specs/cli/inspect-command.md)
- **Unblocks:** none

---

### Priority 21: F-082 — `--step`

- **Summary:** I can pause after every individual run to inspect output before letting rings continue
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** F-083 (`--step-cycles`)

---

### Priority 22: F-099 — Files Changed View

- **Summary:** I can see exactly which files changed in each run, attributed by phase and cycle
- **Spec:** [inspect-command.md](specs/cli/inspect-command.md)
- **Unblocks:** none

---

### Priority 23: F-101 — Costs View

- **Summary:** I can see a detailed cost and token breakdown for every individual run
- **Spec:** [inspect-command.md](specs/cli/inspect-command.md)
- **Unblocks:** none

---

### Priority 24: F-040 — Custom Error Profiles

- **Summary:** I can define my own patterns for what counts as a quota or auth error when using a custom executor
- **Spec:** [error-handling.md](specs/execution/error-handling.md)
- **Unblocks:** none

---

### Priority 25: F-045 — Quota Backoff Max Retries

- **Summary:** I can set a cap on how many times rings will retry after quota errors before giving up
- **Spec:** [rate-limiting.md](specs/execution/rate-limiting.md)
- **Unblocks:** F-087 (`--quota-backoff-max-retries`)

---

### Priority 26: F-085 — `--quota-backoff`

- **Summary:** I can enable automatic quota-error retry at the command line without changing the workflow file
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 27: F-075 — `rings completions`

- **Summary:** I can generate shell completion scripts for bash, zsh, or fish with `rings completions <shell>`
- **Spec:** [completion-and-manpage.md](specs/cli/completion-and-manpage.md)
- **Unblocks:** F-178 (Shell Completions), transitively F-179 (Completion Behavior)

---

### Priority 28: F-178 — Shell Completions

- **Summary:** I can get tab-completion for all commands and flags in bash, zsh, or fish (requires F-075)
- **Spec:** [completion-and-manpage.md](specs/cli/completion-and-manpage.md)
- **Unblocks:** F-179 (Completion Behavior)

---

### Priority 29: F-152 — Consumes File Validation

- **Summary:** rings warns me before a run if a phase's declared input files don't exist yet (requires F-014)
- **Spec:** [phase-contracts.md](specs/workflow/phase-contracts.md)
- **Unblocks:** none

---

### Priority 30: F-153 — Produces File Validation

- **Summary:** rings warns me after a run if a phase's declared output files weren't actually written (requires F-015)
- **Spec:** [phase-contracts.md](specs/workflow/phase-contracts.md)
- **Unblocks:** none

---

### Priority 31: F-070 — `rings list`

- **Summary:** I can see all recent runs with their status and total cost in a summary table
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 32: F-080 — `--cycle-delay`

- **Summary:** I can set or override the between-cycle delay for this run without editing the workflow file (requires F-042)
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 33: F-043 — Duration String Parsing

- **Summary:** I can write delays as human-readable strings like "30s", "5m", or "1h" instead of raw milliseconds
- **Spec:** [rate-limiting.md](specs/execution/rate-limiting.md)
- **Unblocks:** none

---

### Priority 34: F-053 — Double Ctrl+C

- **Summary:** A second Ctrl+C while rings is waiting skips the graceful shutdown and force-kills the subprocess immediately (requires F-051)
- **Spec:** [cancellation-resume.md](specs/state/cancellation-resume.md)
- **Unblocks:** none

---

### Priority 35: F-012 — Completion Signal Modes

- **Summary:** I can match the completion signal by exact substring, line anchor, or full regex
- **Spec:** [completion-detection.md](specs/execution/completion-detection.md)
- **Unblocks:** none

---

### Priority 36: F-029 — Unknown Variable Warnings

- **Summary:** rings warns me at startup if my prompts reference variables it doesn't recognize, before any Claude calls happen
- **Spec:** [prompt-templating.md](specs/execution/prompt-templating.md)
- **Unblocks:** none

---

### Priority 37: F-119 — File Manifest Ignore Patterns

- **Summary:** I can tell rings to skip certain directories (e.g. `.git/`, `target/`) from file tracking (requires F-117)
- **Spec:** [file-lineage.md](specs/observability/file-lineage.md)
- **Unblocks:** none

---

### Priority 38: F-113 — Budget Warning Thresholds

- **Summary:** rings warns me when I've reached 80% and 90% of my budget cap so I'm not surprised by a stop (requires F-112)
- **Spec:** [cost-tracking.md](specs/observability/cost-tracking.md)
- **Unblocks:** none

---

### Priority 39: F-127 — stderr/stdout Separation

- **Summary:** Human-readable output goes to stderr; JSONL events go to stdout so I can pipe them cleanly
- **Spec:** [runtime-output.md](specs/observability/runtime-output.md)
- **Unblocks:** none

---

### Priority 40: F-049 — Resume State Recovery

- **Summary:** If my state file is corrupted, rings can reconstruct my position from audit logs (requires F-046, F-068)
- **Spec:** [cancellation-resume.md](specs/state/cancellation-resume.md)
- **Unblocks:** none

---

### Priority 41: F-050 — Workflow File Change Detection

- **Summary:** rings refuses to resume if I've made structural changes to the workflow since the last run, protecting me from mismatched state
- **Spec:** [cancellation-resume.md](specs/state/cancellation-resume.md)
- **Unblocks:** none

---

### Priority 42: F-022 — Executor Abstraction

- **Summary:** I can swap out the default `claude` binary for any other tool that accepts prompts over stdin
- **Spec:** [executor-integration.md](specs/execution/executor-integration.md)
- **Unblocks:** F-023 (Per-Phase Executors)

---

### Priority 43: F-121 — mtime Optimization

- **Summary:** rings skips re-hashing files whose modification time hasn't changed, keeping large repos fast (requires F-117)
- **Spec:** [file-lineage.md](specs/observability/file-lineage.md)
- **Unblocks:** none

---

### Priority 44: F-120 — Credential File Protection

- **Summary:** rings always excludes `.env`, `*.key`, `*.pem`, and similar files from manifests regardless of my ignore patterns (requires F-117)
- **Spec:** [file-lineage.md](specs/observability/file-lineage.md)
- **Unblocks:** none

---

### Priority 45: F-182 — `rings init`

- **Summary:** I can scaffold a new, immediately runnable workflow TOML file with `rings init [NAME]` so I don't have to write boilerplate by hand
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 46: F-095 — `--output-format`

- **Summary:** I can switch between human-readable and JSONL output for the same run
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 47: F-056 — Stale Lock Detection

- **Summary:** rings automatically removes a lock from a process that is no longer running (requires F-055)
- **Spec:** [cancellation-resume.md](specs/state/cancellation-resume.md)
- **Unblocks:** none

---

### Priority 48: F-164 — Span Attributes

- **Summary:** Each span carries run metadata, phase name, cost, and file change counts so I can filter in my observability platform (requires F-163)
- **Spec:** [opentelemetry.md](specs/observability/opentelemetry.md)
- **Unblocks:** none

---

### Priority 49: F-081 — `--dry-run`

- **Summary:** I can preview the full execution plan — phases, prompts, delays — without any Claude calls
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 50: F-094 — `--no-color`

- **Summary:** I can disable colored terminal output
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 51: F-109 — Directory Permissions

- **Summary:** rings creates my output directory with mode 0700 so only I can read run logs and cost data
- **Spec:** [audit-logs.md](specs/observability/audit-logs.md)
- **Unblocks:** none

---

### Priority 52: F-016 — Produces Required Flag

- **Summary:** I can mark a phase's output as mandatory so rings halts with an error if it produces nothing
- **Spec:** [phase-contracts.md](specs/workflow/phase-contracts.md)
- **Unblocks:** none

---

### Priority 53: F-017 — Advisory Contract Warnings

- **Summary:** rings tells me when a phase didn't meet its declared contract but keeps running so I can observe the issue
- **Spec:** [phase-contracts.md](specs/workflow/phase-contracts.md)
- **Unblocks:** none

---

### Priority 54: F-035 — Parse Warning Summary

- **Summary:** At the end of a run, I see a consolidated summary of any cost parsing failures with raw output snippets
- **Spec:** [output-parsing.md](specs/execution/output-parsing.md)
- **Unblocks:** F-036 (Warning Deduplication)

---

### Priority 55: F-023 — Per-Phase Executors

- **Summary:** I can use a different executor binary for individual phases within the same workflow
- **Spec:** [executor-integration.md](specs/execution/executor-integration.md)
- **Unblocks:** none

---

### Priority 56: F-181 — Per-Phase Model Selection via `executor.extra_args`

- **Summary:** I can set `executor.extra_args` on any phase to append flags (e.g. `--model claude-haiku-4-5`) to the inherited executor args, so I can route cheap phases to smaller models without re-specifying all base flags
- **Spec:** [executor-integration.md](specs/execution/executor-integration.md)
- **Unblocks:** none

---

### Priority 57: F-025 — Include Directory

- **Summary:** I can pass additional context directories whose file listings get prepended to each prompt
- **Spec:** [executor-integration.md](specs/execution/executor-integration.md)
- **Unblocks:** F-078 (`--include-dir`)

---

### Priority 58: F-061 — User Config File

- **Summary:** I can set personal defaults in `~/.config/rings/config.toml` that apply to all my workflows
- **Spec:** [configuration.md](specs/state/configuration.md)
- **Unblocks:** F-065 (Default Output Directory)

---

### Priority 59: F-062 — Project Config File

- **Summary:** I can check a `.rings-config.toml` into my project to share team-level defaults
- **Spec:** [configuration.md](specs/state/configuration.md)
- **Unblocks:** F-067 (Config File Trust Warning)

---

### Priority 60: F-088 — `--budget-cap`

- **Summary:** I can set a spending limit for this run so rings stops and saves state if cost exceeds it (requires F-112)
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 61: F-159 — Exit Code 4

- **Summary:** rings exits 4 when the budget cap is hit and saves state so I can resume after reviewing spend (requires F-112)
- **Spec:** [exit-codes.md](specs/cli/exit-codes.md)
- **Unblocks:** none

---

### Priority 62: F-102 — Claude Output View

- **Summary:** I can read the raw stdout/stderr from any executor invocation inside `rings inspect` (requires F-072, F-106)
- **Spec:** [inspect-command.md](specs/cli/inspect-command.md)
- **Unblocks:** none

---

### Priority 63: F-100 — Data Flow View

- **Summary:** I can see declared vs. actual file inputs and outputs for each phase (requires F-072, F-014, F-015, F-117)
- **Spec:** [inspect-command.md](specs/cli/inspect-command.md)
- **Unblocks:** none

---

### Priority 64: F-026 — Environment Variable Pass-Through

- **Summary:** My shell environment variables are automatically available to the executor subprocess
- **Spec:** [executor-integration.md](specs/execution/executor-integration.md)
- **Unblocks:** none

---

### Priority 65: F-063 — Config Precedence

- **Summary:** I always know which value wins: CLI flags beat env vars beat workflow TOML beat project config beat user config
- **Spec:** [configuration.md](specs/state/configuration.md)
- **Unblocks:** none

---

### Priority 66: F-074 — `rings cleanup`

- **Summary:** I can remove old run data to free disk space
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 67: F-013 — Completion Signal Phase Restriction

- **Summary:** I can limit which phases are allowed to trigger workflow completion so early phases can't accidentally end the run
- **Spec:** [completion-detection.md](specs/execution/completion-detection.md)
- **Unblocks:** none

---

### Priority 68: F-036 — Warning Deduplication

- **Summary:** Repeated parse failures for the same pattern are collapsed into one warning instead of flooding my terminal (requires F-035)
- **Spec:** [output-parsing.md](specs/execution/output-parsing.md)
- **Unblocks:** none

---

### Priority 69: F-108 — summary.md

- **Summary:** rings generates a human-readable markdown summary of the completed run automatically
- **Spec:** [audit-logs.md](specs/observability/audit-logs.md)
- **Unblocks:** none

---

### Priority 70: F-122 — Cycle Snapshots

- **Summary:** rings can copy my entire context_dir at each cycle boundary so I can roll back to any prior cycle (requires F-117)
- **Spec:** [file-lineage.md](specs/observability/file-lineage.md)
- **Unblocks:** F-123 (Snapshot Storage Warning)

---

### Priority 71: F-110 — Path Traversal Protection

- **Summary:** rings rejects any output_dir value containing `..` so a malicious workflow can't write outside the intended directory
- **Spec:** [audit-logs.md](specs/observability/audit-logs.md)
- **Unblocks:** none

---

### Priority 72: F-145 — Sensitive Files Warning

- **Summary:** rings warns me if context_dir contains credentials (`.env`, `*.key`, `*.pem`) that could be exposed to the model
- **Spec:** [engine.md](specs/execution/engine.md)
- **Unblocks:** none

---

### Priority 73: F-066 — Default Executor Config

- **Summary:** I can define executor defaults in my workflow TOML that apply to all phases unless overridden
- **Spec:** [configuration.md](specs/state/configuration.md)
- **Unblocks:** none

---

### Priority 74: F-064 — XDG Base Directory

- **Summary:** rings follows XDG so my config and data land in standard locations alongside my other tools
- **Spec:** [configuration.md](specs/state/configuration.md)
- **Unblocks:** none

---

### Priority 75: F-031 — Custom Cost Parser

- **Summary:** I can provide a custom regex to extract cost from non-standard executor output formats
- **Spec:** [output-parsing.md](specs/execution/output-parsing.md)
- **Unblocks:** none

---

### Priority 76: F-114 — Per-Phase Budget Caps

- **Summary:** I can set independent spending limits on individual phases to protect against a runaway single phase (requires F-112)
- **Spec:** [cost-tracking.md](specs/observability/cost-tracking.md)
- **Unblocks:** none

---

### Priority 77: F-089 — `--strict-parsing`

- **Summary:** I can make rings treat any cost parsing failure as a hard error that stops the run (requires F-033)
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 78: F-144 — Empty Context Directory Warning

- **Summary:** rings warns me if context_dir has no files in case I pointed it at the wrong directory
- **Spec:** [engine.md](specs/execution/engine.md)
- **Unblocks:** none

---

### Priority 79: F-018 — Data Flow Documentation

- **Summary:** I can see the declared and actual data flow for each phase when inspecting a run
- **Spec:** [phase-contracts.md](specs/workflow/phase-contracts.md)
- **Unblocks:** none

---

### Priority 80: F-146 — Output Directory Inside Repo Warning

- **Summary:** rings warns me if my output_dir is inside a git repo and would get committed accidentally
- **Spec:** [engine.md](specs/execution/engine.md)
- **Unblocks:** none

---

### Priority 81: F-147 — Disk Space Check

- **Summary:** rings warns at < 100 MB free and aborts at < 10 MB so I don't silently lose run data
- **Spec:** [engine.md](specs/execution/engine.md)
- **Unblocks:** none

---

### Priority 82: F-148 — Delay Sanity Warning

- **Summary:** rings warns me if `delay_between_runs` exceeds 600 seconds, since that's likely a units mistake
- **Spec:** [engine.md](specs/execution/engine.md)
- **Unblocks:** none

---

### Priority 83: F-057 — Cross-Machine Resume Limitation

- **Summary:** rings documents that resume requires the workflow file at the same absolute path; `--parent-run` is available for cross-machine linking
- **Spec:** [cancellation-resume.md](specs/state/cancellation-resume.md)
- **Unblocks:** none

---

### Priority 84: F-096 — `--no-sensitive-files-check`

- **Summary:** I can suppress the warning about credential files in context_dir when I know they're intentionally there
- **Spec:** [engine.md](specs/execution/engine.md)
- **Unblocks:** none

---

### Priority 85: F-149 — Cost Spike Detection

- **Summary:** rings warns me mid-run when a single run costs 5× more than the rolling 5-run average
- **Spec:** [engine.md](specs/execution/engine.md)
- **Unblocks:** none

---

### Priority 86: F-169 — OTel Init Failure Handling

- **Summary:** If the OTel exporter fails to initialize, rings continues with a no-op tracer instead of aborting
- **Spec:** [opentelemetry.md](specs/observability/opentelemetry.md)
- **Unblocks:** none

---

### Priority 87: F-091 — `--force-lock`

- **Summary:** I can override the context_dir lock check when I know the previous process is truly gone
- **Spec:** [cancellation-resume.md](specs/state/cancellation-resume.md)
- **Unblocks:** none

---

### Priority 88: F-073 — `rings lineage`

- **Summary:** I can see the full chain of parent/child runs that led to any given run ID (requires F-058)
- **Spec:** [inspect-command.md](specs/cli/inspect-command.md)
- **Unblocks:** none

---

### Priority 89: F-150 — No-Files-Changed Streak Warning

- **Summary:** rings warns me after 3 consecutive runs where the declared produces files weren't changed, suggesting the workflow is stuck
- **Spec:** [engine.md](specs/execution/engine.md)
- **Unblocks:** none

---

### Priority 90: F-060 — Continuation Linking

- **Summary:** I can use `--parent-run` to link a fresh run to a prior one without resuming its saved state (requires F-058)
- **Spec:** [run-ancestry.md](specs/state/run-ancestry.md)
- **Unblocks:** none

---

### Priority 91: F-059 — Ancestry Depth Tracking

- **Summary:** rings tracks how many resumptions deep a run is (requires F-058)
- **Spec:** [run-ancestry.md](specs/state/run-ancestry.md)
- **Unblocks:** none

---

### Priority 92: F-065 — Default Output Directory

- **Summary:** I can set a global default for where all run output is written instead of specifying it every time (requires F-061)
- **Spec:** [configuration.md](specs/state/configuration.md)
- **Unblocks:** none

---

### Priority 93: F-090 — `--parent-run`

- **Summary:** I can explicitly link this run to a prior one for ancestry tracking without resuming its state (requires F-058)
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 94: F-067 — Config File Trust Warning

- **Summary:** rings warns me when it loads a `.rings-config.toml` from the current directory in case I ran rings somewhere unexpected (requires F-062)
- **Spec:** [configuration.md](specs/state/configuration.md)
- **Unblocks:** none

---

### Priority 95: F-093 — `--no-contract-check`

- **Summary:** I can suppress phase contract violation warnings for a run (requires F-014, F-015)
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 96: F-137 — Step-Cycles Mode

- **Summary:** With `--step-cycles`, I'm only prompted at cycle boundaries rather than after every run (requires F-136)
- **Spec:** [runtime-output.md](specs/observability/runtime-output.md)
- **Unblocks:** none

---

### Priority 97: F-078 — `--include-dir`

- **Summary:** I can inject additional file-listing context into prompts for this run (requires F-025)
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 98: F-086 — `--quota-backoff-delay`

- **Summary:** I can set how long rings waits before retrying after a quota error (requires F-044)
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 99: F-087 — `--quota-backoff-max-retries`

- **Summary:** I can cap how many quota retries rings will attempt before giving up (requires F-044, F-045)
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 100: F-170 — OTel Endpoint Configuration

- **Summary:** I configure my collector endpoint via the standard `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable
- **Spec:** [opentelemetry.md](specs/observability/opentelemetry.md)
- **Unblocks:** none

---

### Priority 101: F-165 — Span Status

- **Summary:** Spans are marked ERROR on non-zero executor exit so I can alert on failures in my tracing tool
- **Spec:** [opentelemetry.md](specs/observability/opentelemetry.md)
- **Unblocks:** none

---

### Priority 102: F-083 — `--step-cycles`

- **Summary:** I can pause only at cycle boundaries for a less granular step-through experience
- **Spec:** [commands-and-flags.md](specs/cli/commands-and-flags.md)
- **Unblocks:** none

---

### Priority 103: F-171 — Static Binary

- **Summary:** I can download a single binary with no system library dependencies and run it immediately
- **Spec:** [distribution.md](specs/cli/distribution.md)
- **Unblocks:** F-172 (Multi-Platform Release), F-173 (macOS Universal Binary), F-174 (Binary Size Optimization), F-175 (Cargo Install Support), F-176 (SHA256 Checksums), F-177 (Reproducible Builds)

---

### Priority 104: F-167 — OTel Metrics

- **Summary:** rings emits counters and histograms for cost, duration, and token counts so I can dashboard and alert on them
- **Spec:** [opentelemetry.md](specs/observability/opentelemetry.md)
- **Unblocks:** none

---

### Priority 105: F-138 — Step Summary Display

- **Summary:** At each step-through pause, rings shows cost so far, files changed, and whether the completion signal was detected
- **Spec:** [runtime-output.md](specs/observability/runtime-output.md)
- **Unblocks:** none

---

### Priority 106: F-172 — Multi-Platform Release

- **Summary:** I can get native binaries for x86_64 and aarch64 on both Linux and macOS
- **Spec:** [distribution.md](specs/cli/distribution.md)
- **Unblocks:** none

---

### Priority 107: F-175 — Cargo Install Support

- **Summary:** Rust users can install rings with `cargo install rings` without needing pre-built binaries
- **Spec:** [distribution.md](specs/cli/distribution.md)
- **Unblocks:** none

---

### Priority 108: F-176 — SHA256 Checksums

- **Summary:** Every release includes checksums I can verify to confirm binary integrity
- **Spec:** [distribution.md](specs/cli/distribution.md)
- **Unblocks:** none

---

### Priority 109: F-154 — Large Context Directory Warning

- **Summary:** rings warns me if context_dir has > 10,000 files because manifest scanning will be slow (requires F-117)
- **Spec:** [file-lineage.md](specs/observability/file-lineage.md)
- **Unblocks:** none

---

### Priority 110: F-124 — Manifest Compression

- **Summary:** rings stores file manifests as gzip-compressed JSON to keep disk usage low (requires F-117)
- **Spec:** [file-lineage.md](specs/observability/file-lineage.md)
- **Unblocks:** none

---

### Priority 111: F-179 — Completion Behavior

- **Summary:** Tab-completion offers `.toml` files for workflow arguments, run IDs for run arguments, and flag names everywhere (requires F-178)
- **Spec:** [completion-and-manpage.md](specs/cli/completion-and-manpage.md)
- **Unblocks:** none

---

### Priority 112: F-177 — Reproducible Builds

- **Summary:** The Rust toolchain is pinned and Cargo.lock is committed so I can reproduce any release binary myself
- **Spec:** [distribution.md](specs/cli/distribution.md)
- **Unblocks:** none

---

### Priority 113: F-173 — macOS Universal Binary

- **Summary:** On macOS, I get a single universal binary that runs natively on both Intel and Apple Silicon
- **Spec:** [distribution.md](specs/cli/distribution.md)
- **Unblocks:** none

---

### Priority 114: F-180 — Man Page

- **Summary:** I can read `man rings` for offline documentation generated from the same source as `--help`
- **Spec:** [completion-and-manpage.md](specs/cli/completion-and-manpage.md)
- **Unblocks:** none

---

### Priority 115: F-174 — Binary Size Optimization

- **Summary:** The rings binary targets < 5 MB so downloads and distributions stay lightweight
- **Spec:** [distribution.md](specs/cli/distribution.md)
- **Unblocks:** none

---

### Priority 116: F-123 — Snapshot Storage Warning

- **Summary:** rings estimates snapshot storage usage at startup and warns me if it will be unexpectedly large (requires F-122)
- **Spec:** [file-lineage.md](specs/observability/file-lineage.md)
- **Unblocks:** none

---

### Priority 117: F-168 — OTel Path Stripping

- **Summary:** I can set `RINGS_OTEL_STRIP_PATHS=1` to redact filesystem paths from telemetry for privacy (requires F-162)
- **Spec:** [opentelemetry.md](specs/observability/opentelemetry.md)
- **Unblocks:** none

---

### Priority 118: F-166 — Span Links

- **Summary:** When I resume a run, the new trace is linked to the parent run's trace so I can navigate the full history (requires F-163, F-058)
- **Spec:** [run-ancestry.md](specs/state/run-ancestry.md)
- **Unblocks:** none

---
