# Cycle Model

## Definitions

- **Run**: A single invocation of `claude` for one phase. The atomic unit of execution.
- **Cycle**: One complete pass through all phases, in declaration order. Each phase executes `runs_per_cycle` times consecutively before the next phase begins.
- **Workflow**: The full execution, consisting of repeated cycles until completion or termination.

## Execution Order

Given phases `[A (runs=3), B (runs=1)]` and `max_cycles=2`, the execution sequence is:

```
Cycle 1:  A  A  A  B
Cycle 2:  A  A  A  B
```

Phases within a cycle always execute in declaration order. The `runs_per_cycle` value determines how many consecutive times a phase runs before the next phase begins.

## Completion Check

After **every individual run** (not just after a full cycle), rings scans the run's output for the `completion_signal` string. If found:

1. The workflow is marked as completed successfully.
2. No further runs are started, even mid-cycle.
3. The completion summary is printed.

This means completion can be triggered by any phase, not just the last one in a cycle.

## Termination Conditions

A workflow terminates when any of the following occur (checked in priority order):

1. **Completion signal detected** in any run's output — success exit
2. **max_cycles reached** — exits with a non-zero status code and a clear message
3. **User cancellation** (Ctrl+C) — saves state, prints resume instructions, exits
4. **Phase error** (Claude Code exits non-zero) — exits with error, logs details

## Run Numbering

Each run is assigned a monotonically increasing global run number for logging:

```
Run 1:  cycle=1 phase=builder iteration=1/3
Run 2:  cycle=1 phase=builder iteration=2/3
Run 3:  cycle=1 phase=builder iteration=3/3
Run 4:  cycle=1 phase=reviewer iteration=1/1
Run 5:  cycle=2 phase=builder iteration=1/3
...
```

## Context / State Between Runs

rings does not pass output from one phase to another explicitly. State is shared implicitly through the filesystem: all phases are invoked in the same `context_dir`. It is the responsibility of prompts to instruct Claude Code to read and write files in that directory.

This is intentional. rings remains a simple orchestrator and does not need to understand prompt content or output semantics.
