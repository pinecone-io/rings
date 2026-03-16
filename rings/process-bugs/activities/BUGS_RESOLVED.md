# Bugs Resolved

- [x] [2026-03-16] **costs.jsonl records null cost and "none" confidence**: `cost_usd`, `input_tokens`, and `output_tokens` are all `null` in `costs.jsonl` entries (and `cost_confidence` is `"none"`), indicating `parse_cost_from_output` in `src/cost.rs` is not matching any cost pattern in the claude subprocess output.
  → Fixed: Changed `parse_cost_from_output` from a single `serde_json::from_str(output.trim())` call to a line-by-line scan so that stderr content appended after the JSON line no longer breaks JSON parsing.

- [x] [2026-03-16] **Cost always displays as $0.000 in run output**: Per-phase and cycle cost lines always show `$0.000` during `rings run` even when real Claude API calls are being made — costs should reflect actual token usage billed by the API.
  → Fixed: Added `--output-format json` to `ClaudeExecutor::build_args()` and taught `parse_cost_from_output()` to parse `total_cost_usd` from the resulting JSON blob; added `extract_response_text()` to `executor.rs` so signal matching and resume command extraction still operate on the plain-text `result` field.
