# rings inspect and rings lineage Commands

## Overview

`rings inspect` and `rings lineage` are read-only commands for exploring and understanding run history. They make the rich audit data collected during execution navigable without manually reading JSON files.

---

## `rings inspect <RUN_ID>`

Detailed view of a single run: phases, cycles, cost, file changes, and data flow.

```
USAGE:
    rings inspect [OPTIONS] <RUN_ID>

ARGS:
    <RUN_ID>    Run ID to inspect

OPTIONS:
    --show <VIEW>       What to display. May be specified multiple times.
                        Values: summary (default), cycles, files-changed,
                                data-flow, costs, claude-output
    --cycle <N>         Filter output to a specific cycle
    --phase <NAME>      Filter output to a specific phase
    --format <FORMAT>   Output format: human (default) or jsonl
    -h, --help          Print help
```

### Views

**`--show summary`** (default)

```
Run: run_20240315_143022_a1b2c3
Workflow: my-task.rings.toml
Status: completed (signal detected on run 12)
Started: 2024-03-15 14:30:22 UTC   Duration: 8m 14s
Ancestry: root run (no parent)

Cycles: 2 completed
Total runs: 12
Total cost: $1.10

Phase breakdown:
  builder   10 runs   $0.89   avg $0.089/run
  reviewer   2 runs   $0.21   avg $0.105/run

Files changed (total unique): 7
  Most active: src/engine.rs (modified 4 times)

Output: ~/.local/share/rings/runs/run_20240315_143022_a1b2c3/
```

---

**`--show cycles`**

```
Cycle 1:
  Run  1  builder  iter 1/3   $0.092   3 files changed   1.2s
  Run  2  builder  iter 2/3   $0.088   2 files changed   1.1s
  Run  3  builder  iter 3/3   $0.091   1 file changed    1.0s
  Run  4  reviewer iter 1/1   $0.104   1 file changed    1.3s

Cycle 2:
  Run  5  builder  iter 1/3   $0.087   4 files changed   1.1s
  ...
  Run 12  builder  iter 2/3   $0.090   2 files changed ✓ SIGNAL
```

---

**`--show files-changed`**

```
File change history:
  src/main.rs
    ├─ modified  run 1   cycle 1  builder  iter 1
    ├─ modified  run 5   cycle 2  builder  iter 1
    └─ modified  run 7   cycle 2  builder  iter 3

  src/engine.rs
    ├─ modified  run 2   cycle 1  builder  iter 2
    ├─ modified  run 3   cycle 1  builder  iter 3
    ├─ modified  run 6   cycle 2  builder  iter 2
    └─ modified  run 7   cycle 2  builder  iter 3

  review-notes.md
    ├─ created   run 4   cycle 1  reviewer  iter 1
    └─ modified  run 8   cycle 2  reviewer  iter 1
```

---

**`--show data-flow`**

```
Declared data flow (from phase contracts):
  specs/**/*.md  ──→  [builder]  ──→  src/**/*.rs
                                      tests/**/*.rs
  src/**/*.rs   ──→  [reviewer] ──→  review-notes.md

Actual file attribution (this run):
  src/main.rs       builder  (cycles 1, 2)
  src/engine.rs     builder  (cycles 1, 2)
  src/workflow.rs   builder  (cycle 2)
  tests/engine.rs   builder  (cycle 1)
  review-notes.md   reviewer (cycles 1, 2)
```

---

**`--show costs`**

```
Cost breakdown:
  Run   Cycle  Phase     Iter   Input Tok   Output Tok   Cost
  ────────────────────────────────────────────────────────────
    1     1    builder   1/3     1,234         567       $0.092
    2     1    builder   2/3     1,198         534       $0.088
  ...
  ────────────────────────────────────────────────────────────
  Total                         12,341       5,234       $1.10
```

---

**`--show claude-output`**

Prints the captured stdout/stderr from each `claude` invocation, with run headers. Equivalent to `cat output_dir/runs/*.log` but formatted.

With `--cycle N` or `--phase NAME`, filters to matching runs.

---

## `rings lineage <RUN_ID>`

Traverses the ancestry chain for a run and displays the full history of related runs.

```
USAGE:
    rings lineage [OPTIONS] <RUN_ID>

ARGS:
    <RUN_ID>    Any run ID in the chain (will find root and traverse forward)

OPTIONS:
    --format <FORMAT>   human (default) or jsonl
    -h, --help
```

### Output (human)

```
Lineage chain for: run_20240315_150012_x9y8z7

 #  RUN ID                          DATE              STATUS      CYCLES  COST
─────────────────────────────────────────────────────────────────────────────
 1  run_20240315_120000_p1q2r3  2024-03-15 12:00  canceled        3/50   $2.14
    └─ Canceled at cycle 3, builder run 2. Captured 1 claude resume.
 2  run_20240315_130000_s4t5u6  2024-03-15 13:00  canceled        5/50   $3.21  ← resumed from #1
    └─ Canceled at cycle 5, reviewer run 1. Captured 2 claude resumes.
 3  run_20240315_150012_x9y8z7  2024-03-15 15:00  completed       2/50   $1.10  ← resumed from #2
    └─ Completed on cycle 7 (cumulative), builder run 12.

Chain totals:
  Total wall time:   47m 22s
  Total cycles:      10  (spread across 3 runs)
  Total runs:        38
  Total cost:        $6.45
```

### Output (jsonl)

Each run in the chain is emitted as one JSON object, followed by a `chain_summary` object.

```jsonl
{"type":"run","position":1,"run_id":"run_20240315_120000_p1q2r3","status":"canceled","cycles":3,"cost_usd":2.14,"parent_run_id":null}
{"type":"run","position":2,"run_id":"run_20240315_130000_s4t5u6","status":"canceled","cycles":5,"cost_usd":3.21,"parent_run_id":"run_20240315_120000_p1q2r3"}
{"type":"run","position":3,"run_id":"run_20240315_150012_x9y8z7","status":"completed","cycles":2,"cost_usd":1.10,"parent_run_id":"run_20240315_130000_s4t5u6"}
{"type":"chain_summary","depth":3,"total_cycles":10,"total_runs":38,"total_cost_usd":6.45,"final_status":"completed"}
```

---

## `rings show <RUN_ID>` (existing command, updated)

`rings show` is a shorthand for `rings inspect --show summary`. Updated to show ancestry info if present:

```
Ancestry: resumed from run_20240315_143022_a1b2c3
          (run 2 of 3 in chain — see: rings lineage run_20240315_150012_x9y8z7)
```
