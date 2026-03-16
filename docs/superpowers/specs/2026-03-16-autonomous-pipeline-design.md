# Autonomous Pipeline Design

**Date:** 2026-03-16
**Command:** `/pipeline`

## Overview

A single slash command that runs the full idea-to-plan pipeline unattended. The user writes ideas to `IDEAS.md`, triggers `/pipeline`, and walks away. The pipeline classifies ideas, vets them through a user-perspective review panel, writes approved specs, drafts an implementation plan, and hardens that plan through an impl review panel — all without any approval gates or human checkpoints.

The pipeline produces two artifacts: updated spec files (with new features added to `specs/feature_inventory.md`) and a `PLAN.md` with hardened implementation steps ready for execution.

---

## Stage 1: Orientation

Load the full project context:
- `IDEAS.md` — all `## Unprocessed` entries
- `specs/feature_inventory.md`, `specs/index.md`, `specs/overview.md`, `specs/mvp.md`

If there are no unprocessed ideas, exit immediately with a message. Otherwise proceed without confirmation.

---

## Stage 2: Idea Classification and Context Hydration

For each unprocessed idea, classify it as one of:
- **Already covered** — essentially described by an existing spec (note the F-NNN); skip, no further action
- **Extension** — adds nuance or a new option to an existing feature (identify the F-NNN being extended)
- **New feature** — genuinely not covered by any existing spec
- **Out of scope / conflict** — conflicts with a design principle; log to `REJECTED.md` immediately, do not send to review panel

**Context hydration:** For ideas classified as **extension** or **new feature** that reference existing feature areas, dispatch an exploration agent to read:
- The relevant spec files for those feature areas
- The relevant `src/` files to understand actual implementation state

This hydrated context travels with the idea into the review panel.

**Rejection criteria at this stage:**
- Already covered by an existing spec
- Conflicts with a core design principle in `specs/overview.md` or `specs/mvp.md`
- Previously rejected — already appears in `REJECTED.md`
- Depends on a `CANCELLED` or `BLOCKED` feature

---

## Stage 3: Review Panel

All **extension** and **new feature** ideas (with their hydrated context) are dispatched simultaneously to all 15 user-perspective review agents:

`review-cli`, `review-devops`, `review-data-eng`, `review-ai-newcomer`, `review-gen-z`, `review-security`, `review-token-opt`, `review-reliability`, `review-scripter`, `review-oss`, `review-founder`, `review-prompt-eng`, `review-enterprise`, `review-agent-ux`, `review-workflow-author`

Each agent receives: the idea, its classification, relevant spec excerpts, and relevant code context.

**Synthesis:** After all 15 agents return, findings are synthesized per idea. `review-workflow-author` carries the highest weight — a feature that makes workflows hard to author or debug is rejected even if other reviewers approve it.

**Rejection criteria at this stage:**
- Overwhelming concern from user-perspective personas (especially `review-workflow-author`)
- Security issue with no viable mitigation
- Idea is too underspecified to write a coherent spec
- Contradicts a decision recorded in `REVIEW.md`

Ideas that pass proceed to spec writing. Ideas that fail are logged to `REJECTED.md`.

---

## Stage 4: Spec Writing and Rejection Logging

**Accepted ideas** are written into the appropriate spec files:
- New F-NNN number (continuing from the highest existing number)
- One-line user-perspective summary in inventory style
- Status: `BACKLOG`
- Short prose description (3–6 sentences)
- Dependencies on existing features (cite F-NNN)

`specs/feature_inventory.md` is updated with each new entry. The idea is moved from `## Unprocessed` to `## Processed` in `IDEAS.md`.

**Rejected ideas** are appended to `REJECTED.md` (created if missing):
- Original idea text
- Stage at which it was rejected
- Condensed summary of rejection reasoning

A git commit is made at the end of this stage as a clean checkpoint before planning begins.

---

## Stage 5: Draft Implementation Plan

A single agent reads all newly written feature specs plus the relevant `src/` files and produces a draft `PLAN.md`. For each feature:

- **Source files** to create or modify
- **Key types, structs, and traits** to add
- **Test cases required** (unit and integration)
- **Cross-feature dependencies** within this batch
- **Completion criteria checklist** — specific, verifiable conditions that must all be true before the feature is considered done:
  - `just validate` passes (fmt, lint, tests)
  - No `unwrap()` or `expect()` in production code
  - Happy path and key error paths covered at unit and integration level
  - Relevant spec in `specs/` consulted; implementation is consistent
  - `REVIEW.md` updated with decisions, conflicts, or open questions
  - Any feature-specific criteria identified during planning

---

## Stage 6: Plan Hardening

All 15 impl domain expert agents receive the draft `PLAN.md` simultaneously:

`impl-rust`, `impl-architecture`, `impl-deps`, `impl-testing`, `impl-error-handling`, `impl-cli-framework`, `impl-serialization`, `impl-process-mgmt`, `impl-filesystem`, `impl-cross-platform`, `impl-performance`, `impl-memory`, `impl-regex`, `impl-agent-ux`, `impl-docs`

Each reviews the plan from their area of expertise. Findings are incorporated with judgment:

- **Simple, unambiguous improvements** (e.g. "add a test for the empty input case") → added directly as checklist items
- **Substantive changes** (e.g. "this trait boundary needs rethinking") → incorporated as revised prose in the relevant section
- **Conflicting feedback** → surfaced as an **Open Decision** with both positions and a recommended default
- **Overwhelming concern from a reviewer** (e.g. `impl-architecture` finds a fundamental incompatibility) → feature is **cancelled**: marked `CANCELLED` with strikethrough in `specs/feature_inventory.md`, removed from `PLAN.md`, logged to `REJECTED.md` with reviewer reasoning
- **Low-signal or redundant findings** → discarded with a note

**Additional cancellation triggers:**
- Blocks on unimplemented features not in scope for this batch
- Unacceptable performance implications
- The spec written contradicts another spec

The final `PLAN.md` and updated `specs/feature_inventory.md` are committed. The pipeline exits.

---

## Artifacts Produced

| Artifact | Description |
|----------|-------------|
| Updated spec files | New features written into appropriate `specs/` files |
| `specs/feature_inventory.md` | New `BACKLOG` entries added; `CANCELLED` entries struck through |
| `IDEAS.md` | Processed ideas moved from `## Unprocessed` to `## Processed` |
| `REJECTED.md` | All rejected ideas with stage and reasoning |
| `PLAN.md` | Hardened implementation plan with completion criteria per feature |

## Git Checkpoints

1. After Stage 4 (spec writing complete)
2. After Stage 6 (plan hardening complete)

---

## What This Replaces

The `/pipeline` command supersedes the manual sequence of `/process-ideas` → `/feature-election` → `/replan` for autonomous use. Those commands remain available for interactive use (exploring individual ideas, running just a planning pass, etc.).
