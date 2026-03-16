# Autonomous Pipeline Design

**Date:** 2026-03-16
**Command:** `/pipeline`

## Overview

A single slash command that runs the full idea-to-plan pipeline unattended. The user writes ideas to `queues/IDEAS.md`, triggers `/pipeline`, and walks away. The pipeline classifies ideas, vets them through a user-perspective review panel, writes approved specs, drafts an implementation plan, and hardens that plan through an impl review panel — all without any approval gates or human checkpoints.

All newly accepted ideas are planned. There is no feature election or voting step — the user has already curated the ideas by writing them down.

The pipeline produces two artifacts: updated spec files (with new features added to `specs/feature_inventory.md`) and a `queues/PLAN.md` with hardened implementation steps ready for execution.

---

## Stage 1: Orientation

Load the full project context:
- `queues/IDEAS.md` — all `## Unprocessed` entries. Each entry is a bullet point or paragraph under the `## Unprocessed` section header. If `queues/IDEAS.md` does not exist or has no `## Unprocessed` section, exit with a message asking the user to add ideas first.
- `specs/index.md`, `specs/feature_inventory.md`, `specs/overview.md`, `specs/mvp.md`

If there are no unprocessed ideas, exit immediately with a message. Otherwise proceed without confirmation.

---

## Stage 2: Idea Classification and Context Hydration

For each unprocessed idea, classify it as one of:
- **Already covered** — essentially described by an existing spec (note the F-NNN); queue for rejection logging with reason "already covered: F-NNN"
- **Extension** — adds nuance or a new option to an existing feature (identify the F-NNN being extended)
- **New feature** — genuinely not covered by any existing spec
- **Out of scope / conflict** — conflicts with a design principle; queue for rejection logging with reason

Do not write any files during Stage 2. All `queues/IDEAS.md` moves and `REJECTED.md` writes happen in Stage 4.

**Context hydration:** For ideas classified as **extension** or **new feature** that reference existing feature areas, dispatch an exploration agent to read:
- The relevant spec files for those feature areas
- The relevant `src/` files to understand actual implementation state

This hydrated context (spec excerpts + relevant `src/` file content) travels with the idea into the review panel and forward to Stage 5.

**Rejection criteria at this stage:**
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

Ideas that pass proceed to spec writing. Ideas that fail are queued for rejection logging.

**If all ideas are rejected or already covered:** proceed to Stage 4 to flush all rejection logs and `queues/IDEAS.md` moves, make a single commit covering all Stage 2 and Stage 3 rejections, then exit with the message: "All ideas were rejected or already covered. See REJECTED.md."

---

## Stage 4: Spec Writing and Rejection Logging

All `queues/IDEAS.md` moves and `REJECTED.md` writes from Stages 2 and 3 are flushed here in a single pass.

**Target spec file:** For each accepted idea, determine the appropriate spec file by consulting `specs/index.md` for the existing file list and matching by topic area. If no existing spec file fits, create a new one under the appropriate subdirectory and add it to `specs/index.md`.

**F-NNN assignment:** Read `specs/feature_inventory.md` once to find the current highest F-NNN. Assign all new feature numbers sequentially in-memory before writing anything. There is no need to re-read between writes — the pipeline is single-agent at this stage with no concurrent writers.

**Accepted ideas** are written into the appropriate spec files, one at a time:
- New F-NNN number
- One-line user-perspective summary in inventory style
- Status: `BACKLOG`
- Short prose description (3–6 sentences)
- Dependencies on existing features (cite F-NNN)

`specs/feature_inventory.md` is updated with each new entry.

**All processed ideas** (accepted, rejected, and already-covered) are moved from `## Unprocessed` to `## Processed` in `queues/IDEAS.md`.

**All rejected ideas** (from Stages 2 and 3) are appended to `REJECTED.md` (created if missing) using this format:

```markdown
## [YYYY-MM-DD] <first line of idea text>
- **Stage rejected:** Stage 2 / Stage 3
- **Reason:** <condensed reasoning>
```

A git commit is made at the end of this stage as a clean checkpoint before planning begins.

---

## Stage 5: Draft Implementation Plan

A single agent reads the spec entries written in Stage 4 for ideas that were accepted (not already-covered or rejected), plus the hydrated `src/` context carried forward from Stage 2, and produces a draft `queues/PLAN.md`. If `queues/PLAN.md` already exists, overwrite it — the new plan supersedes the previous one.

Stage 6 findings will be incorporated inline into each feature's section rather than surfaced as a separate `## Implementation Review Findings` block. This keeps the plan self-contained per feature and avoids a separate synthesis layer.

`queues/PLAN.md` structure:

```markdown
# Implementation Plan — [date]

## Features in This Batch
[F-NNN list with one-line summaries]

## Implementation Steps

### F-NNN: Feature Name

**Source files to create or modify:**
- `src/foo.rs` — description of changes

**Key types, structs, and traits:**
- `FooBar` — purpose

**Test cases required:**
- Unit: [specific cases]
- Integration: [specific cases]

**Cross-feature dependencies:**
- Depends on F-NNN (Feature Name) — reason

**Completion criteria:**
- [ ] `just validate` passes (fmt, lint, tests)
- [ ] No `unwrap()` or `expect()` in production code
- [ ] Happy path and key error paths covered at unit and integration level
- [ ] Relevant spec in `specs/` consulted; implementation is consistent
- [ ] `REVIEW.md` updated with decisions, conflicts, or open questions
- [ ] [Any feature-specific criteria identified during planning]

## Open Decisions
[Explicit choices to make, each with a recommended default and tradeoffs]

## Spec Gaps
[Ambiguities in the spec that would affect implementation]
```

---

## Stage 6: Plan Hardening

All 15 impl domain expert agents receive the draft `queues/PLAN.md` simultaneously:

`impl-rust`, `impl-architecture`, `impl-deps`, `impl-testing`, `impl-error-handling`, `impl-cli-framework`, `impl-serialization`, `impl-process-mgmt`, `impl-filesystem`, `impl-cross-platform`, `impl-performance`, `impl-memory`, `impl-regex`, `impl-agent-ux`, `impl-docs`

Each reviews the plan from their area of expertise. Findings are incorporated with judgment:

- **Simple, unambiguous improvements** (e.g. "add a test for the empty input case") → added directly as checklist items in the relevant feature's completion criteria
- **Substantive changes** (e.g. "this trait boundary needs rethinking") → incorporated as revised prose in the relevant section
- **Conflicting feedback** → surfaced as an **Open Decision** in `queues/PLAN.md` with both positions and a recommended default
- **Feature cancellation:** A single `blocker`-severity finding from `impl-architecture` is sufficient to cancel a feature. A `blocker`-severity finding from any other impl reviewer triggers cancellation only if at least one other reviewer independently raises a `blocker`-severity concern about the same issue. When cancelled: set status to `CANCELLED` and wrap the feature name in `~~strikethrough~~` in `specs/feature_inventory.md`; also update the `Status:` field in the feature's spec file to `CANCELLED`; remove the feature from `queues/PLAN.md`; append to `REJECTED.md` using this format:

```markdown
## [YYYY-MM-DD] F-NNN: <Feature Name>
- **Stage rejected:** Stage 6 (plan hardening)
- **Reason:** <reviewer name and condensed concern>
```

- **Low-signal or redundant findings** → discarded with a note

**Additional cancellation triggers:**
- Blocks on unimplemented features not in scope for this batch
- Unacceptable performance implications
- The written spec contradicts another spec

**If all features are cancelled during hardening:** do not commit an empty `queues/PLAN.md`. The Stage 6 commit includes: all spec files with `Status: CANCELLED` updates, `specs/feature_inventory.md` with `~~strikethrough~~` entries, and `REJECTED.md` additions. Then exit with the message: "All features cancelled during plan hardening. See REJECTED.md for details."

**Normal exit commit** (when at least one feature survives): commit `queues/PLAN.md`, `specs/feature_inventory.md`, any spec files updated with `Status: CANCELLED`, and `REJECTED.md`. The pipeline exits.

---

## Artifacts Produced

| Artifact | Description |
|----------|-------------|
| Updated spec files | New features written into appropriate `specs/` files; cancelled features have `Status: CANCELLED` |
| `specs/feature_inventory.md` | New `BACKLOG` entries added; cancelled entries have status `CANCELLED` and `~~name~~` |
| `queues/IDEAS.md` | All processed ideas moved from `## Unprocessed` to `## Processed` |
| `REJECTED.md` | All rejected and cancelled ideas with stage, date, and reasoning |
| `queues/PLAN.md` | Hardened implementation plan with completion criteria per feature (overwritten each run) |

## Git Checkpoints

1. After Stage 4 (spec writing complete, before planning)
2. After Stage 6 (plan hardening complete — this is also the commit made on the all-cancelled early-exit path)

---

## What This Replaces

The individual commands (`/process-ideas`, `/replan`) remain available for interactive use — exploring ideas one at a time, running a standalone planning pass, etc. `/pipeline` supersedes the manual sequence of `/process-ideas` → `/replan` for autonomous use. `/replan` itself incorporates feature election (Wave 1 voting) and impl review (Wave 2); `/pipeline` replaces this entire sequence without the voting step, since the user has already curated the ideas.
