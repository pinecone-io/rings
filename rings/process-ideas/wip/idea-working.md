# Idea Working File

## Raw Idea
RINGS_CONTINUE signal: a per-cycle short-circuit analogous to `continue` in a loop. when a phase emits a line containing RINGS_CONTINUE, rings skips all remaining phases in the current cycle and immediately begins the next cycle. useful when an early phase determines the current cycle has nothing to do (e.g. an idea is out-of-scope and needs no review or write phases). configured as continue_signal in the workflow TOML, similar to completion_signal.

## Classification
Extension — extends the cycle model (F-010) and completion signal system (F-011) with a new short-circuit mechanism

## Related Features
- F-010 — Cycle Model — rings runs all phases in declaration order, repeating as full cycles
- F-011 — Completion Signal — detects a string in output to terminate the workflow
- F-013 — Completion Signal Phase Restriction — limits which phases can trigger completion (analogous mechanism)
- F-027 — Prompt Templating — `continue_signal` is a TOML field like `completion_signal`

## Spec File
specs/workflow/cycle-model.md

## Review Synthesis
(to be filled by the review phase)
