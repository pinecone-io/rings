# rings — Product Overview

## What is rings?

`rings` is a command-line tool that orchestrates iterative AI prompt workflows using Claude Code. It automates the "Ralph loop" pattern: running a task many times with fresh context to progressively improve output quality, while tracking cost, progress, and execution state.

The core insight is that good AI-assisted development often requires many iterations — a builder agent drafts, a reviewer agent critiques, and the loop repeats until the output meets a defined completion signal. Without a tool like rings, each of these workflows requires bespoke shell scripts that are hard to share, hard to resume, and provide no cost visibility.

## The Ralph Loop

The "Ralph loop" is the informal name for an iterative AI prompting pattern where:

1. You give an AI agent a task, a working directory, and a goal condition.
2. The agent runs to completion (or context limit) and produces some output.
3. You run it again with fresh context, building on what was written to disk.
4. Repeat until the goal condition is met.

The key insight is that each iteration starts with a fresh context window but operates on the same on-disk state, so progress accumulates across runs even though the model has no memory of prior runs. The model reads what it previously wrote, infers its own progress, and continues.

rings automates the loop mechanics: detecting the goal condition, tracking iteration counts, managing cost accounting, and enabling safe interruption and resumption.

## What rings is NOT

- **Not a task manager.** rings has no concept of tasks, tickets, or work items. Phases may use files in `context_dir` to pass structured information between each other (e.g. a reviewer leaving notes for the next builder iteration), but the format and content of those files is entirely the user's responsibility. rings only tracks which files changed, not what they mean.
- **No direct LLM integration.** rings never calls any AI API directly. All model interaction goes through the `claude` subprocess. rings does not send prompts, outputs, or any workflow content to any remote service.
- **No cloud data transmission.** rings is a local tool. The only outbound network traffic it can generate is OpenTelemetry spans to a collector the user has explicitly configured via `RINGS_OTEL_ENABLED` and `OTEL_EXPORTER_OTLP_ENDPOINT`. If OTel is not enabled (the default), rings makes no network calls whatsoever.
- **Not a general task runner or CI system.** Its scope is iterative AI prompting workflows.

## Key Value Proposition

1. **Removes scaffolding burden** — define a workflow in a TOML file, not a shell script
2. **Cost visibility** — tracks Claude Code token costs per phase and per cycle
3. **Safe cancellation** — Ctrl+C saves state and captures `claude resume` commands so no work is lost
4. **Resumable** — interrupted runs can be continued from the last completed step
5. **Observable** — rich runtime output and structured audit logs
6. **Reproducible** — workflows are defined as code (TOML files), versionable in git, and shareable across a team

## Target User

A developer who has used AI-assisted development and is ready to do it properly.

The ideal rings user has been running Claude Code on real tasks — not toy scripts — and has hit the limits of one-shot prompting. They've written their first janky shell loop to iterate Claude. They know what they're trying to build, and they want the infrastructure to support it: cost visibility, reproducible workflows, safe cancellation, and proper observability.

Specifically:
- Has experience with Claude Code for agentic tasks beyond simple one-shot prompts
- Has felt the pain of multi-step AI workflows without proper tooling (shell scripts, lost progress, unknown cost)
- Cares about knowing what they're spending and where
- Runs long workflows (10–50+ iterations) and needs reliable interruption and resumption
- Wants to version their workflows in git and share them with teammates
- Values explicit, auditable behavior over magic

rings is not for casual one-shot AI usage. It is for developers who want to build AI-assisted workflows that are reliable, cost-visible, and reproducible — the same standards they apply to any other engineering work.

## Design Principles

These principles guide rings's design decisions:

1. **Explicit over magic** — rings never makes decisions on the user's behalf without them being visible. Completion conditions, costs, and state are always shown. Automation should be legible.
2. **Observable over opaque** — every run is auditable: cost per phase, files changed, full output logs. If something went wrong, you can find out what and when.
3. **Resumable over disposable** — work is never lost. Interrupted runs are saved and resumable. rings treats long-running AI workflows as valuable work in progress, not throwaway experiments.
4. **Cost-visible by default** — token spend is tracked and displayed at every level. There are no silent charges. Budget caps are available and recommended.
5. **Composable with the shell** — rings is a well-behaved Unix tool. It respects stdout/stderr separation, emits structured JSONL for automation, uses meaningful exit codes, and plays well in pipelines.

## Relationship to Claude Code

rings treats Claude Code as an execution substrate. Each phase iteration becomes a `claude --dangerously-skip-permissions -p "..."` invocation. Claude Code handles all model interaction; rings handles orchestration, state, cost tracking, and lifecycle management.
