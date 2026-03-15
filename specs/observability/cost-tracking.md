# Cost Tracking

## Overview

rings parses token cost information from Claude Code's output after each run and accumulates totals per phase and globally. Cost data is included in real-time display, per-run audit logs, and the final completion summary.

## Parsing Claude Code Cost Output

Claude Code outputs a cost line at the end of each session. rings uses a multi-pattern cascade to extract cost data leniently — see `specs/execution/output-parsing.md` for the full parsing strategy, confidence levels, and warning behavior.

The primary pattern matched:
```
Cost: $0.0234 (1,234 input tokens, 567 output tokens)
```

If no cost line is found, cost is recorded as `null` and a parse warning is accumulated for display after the run completes. This is never a hard error.

## Cost Data Model

Per run:
```
RunCost {
  run_number: u32,
  phase_name: String,
  cycle: u32,
  iteration: u32,
  cost_usd: Option<f64>,
  input_tokens: Option<u64>,
  output_tokens: Option<u64>,
}
```

Accumulated per phase:
```
PhaseCostSummary {
  phase_name: String,
  total_runs: u32,
  total_cost_usd: f64,
  total_input_tokens: u64,
  total_output_tokens: u64,
}
```

## Real-Time Display

The status line shows running cost totals:

```
Cycle 3/10 | builder (run 2/3) | Total cost: $0.47
```

## Cost in Audit Log

Each run's audit log entry includes cost. See audit-logs.md.

## Final Summary

On workflow completion or cancellation:

```
Cost Summary
─────────────────────────────────────────
Phase       Runs   Input Tok   Output Tok   Cost
─────────────────────────────────────────
builder       30    245,123      89,432     $3.12
reviewer      10     82,341      21,089     $1.11
─────────────────────────────────────────
TOTAL         40    327,464     110,521    ≥$4.23  ← if any runs had null cost
TOTAL         40    327,464     110,521     $4.23  ← if all runs parsed successfully
```

When any runs have `cost_usd: null` (parse failure), the TOTAL row is prefixed with `≥` to indicate the figure is a floor, not an exact amount. A footnote is added:

```
* 3 runs had unparseable cost output and are excluded from this total.
  Actual total may be higher. See parse warnings above.
```

This ensures users never silently underestimate spend due to parse failures.

## Storage

Cost data is written to `output_dir/<run-id>/costs.jsonl` as newline-delimited JSON, one entry per run. This allows partial cost recovery even if rings is killed without a clean shutdown.

```jsonl
{"run":1,"phase":"builder","cycle":1,"iteration":1,"cost_usd":0.0234,"input_tokens":1234,"output_tokens":567,"cost_confidence":"full"}
{"run":2,"phase":"builder","cycle":1,"iteration":2,"cost_usd":0.0198,"input_tokens":1050,"output_tokens":489,"cost_confidence":"full"}
```

## Budget Warning Thresholds

When a `budget_cap_usd` is configured, rings emits a `budget_warning` advisory event (JSONL) and prints a warning to the status line when cumulative cost reaches **80%** and **90%** of the cap:

```jsonl
{"event":"budget_warning","run_id":"run_...","cost_usd":4.00,"budget_cap_usd":5.00,"pct":80,"timestamp":"..."}
```

The hard stop at 100% is unchanged (exit code 4, state saved). The 80%/90% warnings are advisory only — execution continues.

## No Budget Cap Warning

At startup, if no `budget_cap_usd` is configured (neither in the workflow file nor via `--budget-cap`), rings emits an advisory warning:

```
Warning: No budget cap configured. Use --budget-cap or budget_cap_usd to prevent unbounded spend.
```

This is a warning only — rings proceeds normally. Users running rings in non-interactive or CI environments are strongly encouraged to set a budget cap.

## Per-Phase Budget Caps

A `budget_cap_usd` can also be set on individual `[[phases]]` entries (see `workflow-file-format.md`). A per-phase cap applies to the cumulative cost of that phase across all cycles, independently of the global cap. Whichever limit triggers first stops execution. The `budget_cap` JSONL event includes a `scope` field: `"global"` or `"phase:<name>"`.
