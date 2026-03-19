## Batch: Token Display + Model Indicator — 2026-03-19

**Features:** F-190 (Cumulative Token Display), F-191 (Model Name Display)

**Context:** Token counts are already parsed per-run (`input_tokens`, `output_tokens` in `RunCost` / `CostEntry`) but not accumulated or shown in the status line or summaries. Model name is not yet captured from executor args (F-181 prerequisite for full support), but we can extract it from existing args today for the common case.

### Task 1: Cumulative Token Tracking in Engine

**Files:** `src/engine.rs`

**Steps:**
1. - [x] Add `cumulative_input_tokens: u64` and `cumulative_output_tokens: u64` fields to `BudgetTracker`
2. - [x] After each successful run's cost is parsed, accumulate token counts: `if let Some(t) = cost.input_tokens { ctx.budget.cumulative_input_tokens += t; }` (same for output)
3. - [x] On resume, reconstruct cumulative tokens from `costs.jsonl` in the existing `reconstruct_from_costs()` pass (same loop that rebuilds `cumulative_cost` and rolling windows)
4. - [x] Add `total_input_tokens: u64` and `total_output_tokens: u64` to `EngineResult` so summaries can display them

**Tests:**
- [x] Cumulative tokens increment correctly across multiple runs
- [x] Resume reconstructs token totals from costs.jsonl
- [x] Runs with `None` tokens don't affect totals

---

### Task 2: Token Display in Status Line

**Files:** `src/display.rs`

**Steps:**
1. - [x] Update `format_status_line` / `print_run_elapsed` signatures to accept `cumulative_input_tokens: u64` and `cumulative_output_tokens: u64`
2. - [x] Add `format_token_count(n: u64) -> String` helper: below 1000 → plain integer (e.g., `842`), 1000+ → one decimal `k` (e.g., `1.2k`, `18.2k`), 1M+ → one decimal `M` (e.g., `1.1M`)
3. - [x] Append token segment to status line: `│  18.2k in · 4.1k out` — rendered **dim**, separator **dim**
4. - [x] Omit the token segment entirely when both cumulative counts are 0 (no data parsed yet)
5. - [x] Update call sites in `src/engine.rs` poll loop to pass the new fields

**Tests:**
- [x] `format_token_count`: 0 → `"0"`, 999 → `"999"`, 1000 → `"1.0k"`, 18200 → `"18.2k"`, 1100000 → `"1.1M"`
- [x] Status line includes token segment when tokens > 0
- [x] Status line omits token segment when both are 0

---

### Task 3: Token Display in Summaries

**Files:** `src/display.rs`, `src/main.rs`

**Steps:**
1. - [x] Update `print_completion` to accept and display total token counts: `Tokens      18,204 input · 4,102 output` — values in **dim**, with comma-separated formatting
2. - [x] Update `print_cancellation` to show the same token line
3. - [x] Update call sites in `src/main.rs` to pass `EngineResult.total_input_tokens` / `total_output_tokens`

**Tests:**
- [x] Completion output includes token line when tokens > 0
- [x] Token line omitted when both are 0
- [x] Comma formatting correct (e.g., `18,204`)

---

### Task 4: Model Name Detection + Startup Display

**Files:** `src/workflow.rs`, `src/display.rs`, `src/main.rs`

**Steps:**
1. - [x] Add `pub fn detect_model_name(&self) -> Option<String>` to `Workflow` — scans `executor.args` and each phase's `extra_args` for `--model` followed by a value. Returns the global model if all phases use the same one, or `None` if mixed or undetectable.
2. - [x] Update `print_run_header` to accept `model: Option<&str>` — if `Some`, show `Model      claude-sonnet-4-5` in **dim**; if `None`, show `Model      (default)` in **dim** to indicate Claude Code's configured default is being used
3. - [x] Update call sites in `src/main.rs` (`run_inner`, `resume_inner`) to pass `workflow.detect_model_name().as_deref()`

**Tests:**
- [x] `detect_model_name` returns `Some("claude-sonnet-4-5")` when `args = ["--model", "claude-sonnet-4-5"]`
- [x] Returns `None` when no `--model` flag present
- [x] Returns `None` when phases use different models
- [x] Startup header shows model name when detected, shows `(default)` when not

---
