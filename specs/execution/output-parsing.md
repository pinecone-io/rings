# Executor Output Parsing

## Philosophy

rings extracts structured information from executor stdout/stderr — cost figures, token counts, resume commands, error patterns, and quota signals. An executor's output format is not a stable API; it can change at any time without notice, and rings has no control over that.

The parsing layer must therefore be maximally lenient:

- **Extra fields or lines**: silently ignored
- **Missing optional fields**: logged as a warning, execution continues using whatever is available
- **Complete parse failure** (nothing useful extracted): logged as a warning, execution continues with the affected field recorded as `null`/unknown
- **Unexpected format**: never a hard error; always a degraded-success path

The goal is that an executor update which changes output formatting causes warnings and partial data loss — not a crash or a halted workflow.

## What rings Parses

| Data | Source | Required? | Degraded behavior |
|------|--------|-----------|-------------------|
| Cost in USD | Cost line in output | No | Record `null`, warn |
| Input token count | Cost line in output | No | Record `null`, no warn (tokens are bonus data) |
| Output token count | Cost line in output | No | Record `null`, no warn |
| Resume commands | Any line in output | No | Empty list, no warn |
| Completion signal | Any substring in output | Yes (user-defined) | N/A — exact substring match, never fails |
| Quota/rate limit signals | Pattern scan in output | No | Miss the pattern, treat as unknown error |
| Error classification | Pattern scan in output | No | Fall back to `Unknown` class |

## Cost Parser Profiles

Cost parsing behavior is controlled by the `cost_parser` field in the `[executor]` config.

### `"claude-code"` (default)

Rather than a single regex, rings tries a cascade of patterns from most specific to least specific. The first match wins. Each pattern extracts what it can; missing sub-groups are recorded as `null`.

```
Pattern 1 (full):           Cost: $X.XX (N,NNN input tokens, M,MMM output tokens)
Pattern 2 (no tokens):      Cost: $X.XX
Pattern 3 (alternate label): Total cost: $X.XX
Pattern 4 (generic):        \$(\d+\.\d+)  (last resort — find any dollar amount near "cost")
```

If pattern 4 matches, a `ParseConfidence::Low` flag is set on the result, causing a warning to be emitted. The value is still used. If nothing matches, `cost_usd = None` and a warning is emitted.

### `"none"`

Skips cost extraction entirely. All cost fields are recorded as `null`. No parse warnings are emitted. Use this for executors that do not report cost in their output.

### Custom pattern

A TOML inline table with a `pattern` key containing a regex with named capture groups:

```toml
[executor]
cost_parser = { pattern = 'Cost: \$(?P<cost_usd>[\d.]+) \((?P<input_tokens>[\d,]+) input, (?P<output_tokens>[\d,]+) output\)' }
```

Named captures: `cost_usd` (required), `input_tokens` (optional), `output_tokens` (optional). The custom pattern is tried once; if it matches with all optional groups present, confidence is `Full`; if only `cost_usd` matches, confidence is `Partial`; if it does not match, confidence is `None`.

### ParseResult type

```rust
pub struct CostParseResult {
    pub cost_usd: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub confidence: ParseConfidence,
    pub raw_match: Option<String>,  // the line(s) that produced the match, for diagnostics
}

pub enum ParseConfidence {
    Full,    // pattern 1 matched — all fields present
    Partial, // pattern 2 or 3 — cost present, tokens absent
    Low,     // pattern 4 — dollar amount found but context uncertain
    None,    // no match at all
}
```

## ParseConfidence Exposure

The `ParseConfidence` value for each run is surfaced to users through multiple channels:

- **Human output mode:** A warning summary is printed at workflow completion if any runs had `Low` or `None` confidence (see Warning Surfacing below).
- **JSONL mode:** The `run_end` event includes a `"cost_confidence"` field with the lowercase confidence value (`"full"`, `"partial"`, `"low"`, `"none"`).
- **Audit logs:** Each entry in `costs.jsonl` includes a `"cost_confidence"` field, enabling queries like `jq 'select(.cost_confidence == "low")' costs.jsonl` across a run's history.
- **Strict mode (`--strict-parsing`):** `Low` or `None` confidence causes exit code 2. `Partial` confidence does **not** trigger strict mode — missing token counts are acceptable degraded data.

## Warning Surfacing

Warnings are never shown inline during a run (they would interrupt the spinner). Instead they are:

1. Written to the run's audit log entry for the affected run
2. Accumulated in memory during the workflow
3. Printed in a summary block **after** the final completion/cancellation output:

```
Parse warnings (3 runs affected):
  Run 7:  Cost parsing: partial match (tokens not found). Raw: "Cost: $0.023"
  Run 12: Cost parsing: no match found. Cost recorded as unknown.
           Raw output tail: "...session complete, goodbye..."
  Run 18: Cost parsing: low-confidence match ($0.041). Raw: "total: $0.041 spent"
```

If there are no warnings, the block is omitted entirely.

In `--output-format jsonl`, each warning is emitted as a `parse_warning` event immediately after the `run_end` event it relates to:

```jsonl
{"event":"parse_warning","run":7,"field":"cost_usd","confidence":"partial",
 "message":"tokens not found in cost line","raw_match":"Cost: $0.023","timestamp":"..."}
```

## Warning Deduplication

If the same parse failure pattern occurs on more than 3 consecutive runs, rings upgrades the warning:

```
Parse warnings:
  Runs 7–20 (14 runs): Cost parsing failed consistently. The executor's output format
  may have changed or cost reporting may be disabled. Check: ~/.local/share/rings/runs/<run-id>/runs/007.log
  and report at https://github.com/owner/rings/issues
```

The repeated-failure detection resets if a successful parse occurs.

## Executor Version Tracking

If the executor outputs a version string (e.g. a header line like `Claude Code v1.2.3`), rings captures it and stores it in `run.toml`:

```toml
executor_version = "1.2.3"   # null if not detected
```

This is best-effort. When a parse failure occurs, the audit log records the executor version alongside the raw output snippet, making it much easier to correlate output format changes with specific executor releases when filing issues.

## Raw Snippet Preservation

When any parse confidence is `Low` or `None`, rings saves the last 20 lines of the run's output into the audit log entry as `raw_tail`. This gives enough context to diagnose what changed without requiring the full log to be read.

```json
{
  "run": 12,
  "cost_usd": null,
  "parse_confidence": "none",
  "raw_tail": "...last 20 lines of claude output..."
}
```

## Resume Command Extraction

Resume command extraction uses the regex configured in `executor.resume_pattern`. The named capture group `id` identifies the resumable session. Failure to find any resume commands is not a warning — many runs complete without generating a resumable session. The parser extracts all non-overlapping matches; duplicate IDs are deduplicated.

For executors that do not support resumable sessions, set `resume_pattern = ""` to disable extraction entirely.

## Error Pattern Matching

The quota/auth pattern lists (see `error-handling.md`) are intentionally broad and case-insensitive. False positives (wrongly classifying an unknown error as quota) are acceptable — the behavior is the same either way (pause and save). False negatives (missing a quota error) fall back gracefully to `Unknown` classification with the same pause-and-save behavior.

## Strict Parsing Mode

By default rings treats parse failures as warnings and continues. When `--strict-parsing` is set (or `strict_parsing = true` in the config file), any parse result with confidence `Low` or `None` is treated as a hard error.

**Behavior:**
1. Stop execution immediately after the affected run.
2. Save state (same as cancellation — the run is resumable).
3. Print a clear error explaining what failed to parse and pointing to the raw log.
4. Exit with code `2`.

`Partial` confidence (cost found, token counts absent) does **not** trigger strict mode — missing token counts are considered acceptable degraded data, not an observability failure.

```
Error: Strict parsing mode is enabled and cost parsing failed on run 7 (confidence: none).
  The executor's output format may have changed, or cost output was suppressed.
  Raw log: ~/.local/share/rings/runs/run_20240315_143022_a1b2c3/runs/007.log

  Progress saved. To resume (with strict parsing disabled):
    rings resume run_20240315_143022_a1b2c3
```

Strict mode is intentionally opt-in. Most users want the workflow to complete even with imperfect cost data. Strict mode is for pipelines or monitoring setups where incomplete observability data is worse than a stopped run.

## Testing Strategy

The output parsing tests use a fixture-based approach: a `tests/fixtures/executor_output/` directory contains sample output files organized by executor profile:

```
tests/fixtures/executor_output/
  claude-code/
    cost_full.txt          — standard format with all fields
    cost_no_tokens.txt     — cost only, no token counts
    cost_changed_format.txt — hypothetical future format (for regression testing)
    cost_missing.txt       — no cost line at all
    resume_command.txt     — output containing a resume line
    quota_error.txt        — quota exhaustion message
    auth_error.txt         — authentication failure message
    unknown_error.txt      — unrecognized error
  custom/
    custom_pattern.txt     — sample output for custom cost_parser pattern tests
```

When an executor's output format changes in the wild and is reported, a new fixture is added first, the test is written to assert the correct degraded behavior, and then the parser is updated. This ensures regressions don't re-appear.
