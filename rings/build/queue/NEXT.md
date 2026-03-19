## Batch: Visual Output Overhaul — 2026-03-19

**Features:** F-094 (`--no-color`), F-125 (Human Output Mode), F-129 (Animated Spinner),
F-183 (Color System), F-184 (Phase Cost Bar Chart), F-185 (Budget Gauge),
F-186 (Styled Startup Header), F-187 (Styled Cycle Transitions),
F-188 (Styled List Table), F-189 (Styled Dry Run Output)

**Design direction:** Turborepo/Vercel aesthetic — minimal, clean, subtle colors, generous whitespace.
**Dependency:** `owo-colors` (zero-alloc) + hand-rolled spinner. No throwaway work if TUI comes later.

### Task 1: Style Module Foundation

**Files:** `Cargo.toml`, `src/style.rs` (new), `src/lib.rs`, `src/cli.rs`, `src/main.rs`

**Steps:**
1. [x] Add `owo-colors = "4"` to `[dependencies]` in `Cargo.toml`
2. [x] Create `src/style.rs` with:
   - `color_enabled() -> bool` — checks an `AtomicBool` (default: true for TTY, false for non-TTY)
   - `set_no_color()` — sets the `AtomicBool` to false
   - Semantic helper functions: `dim(s)`, `bold(s)`, `success(s)`, `error(s)`, `warn(s)`, `accent(s)`, `muted(s)` — each applies the corresponding `owo-colors` style if `color_enabled()`, otherwise returns the input unchanged
   - `SPINNER_FRAMES: &[&str] = &["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"]`
   - `spinner_frame(tick: usize) -> &'static str` — returns `SPINNER_FRAMES[tick % SPINNER_FRAMES.len()]`
3. [x] Register `pub mod style;` in `src/lib.rs`
4. [x] Add `--no-color` to CLI global args in `src/cli.rs` (global flag, available on all subcommands)
5. [x] Wire in `src/main.rs`: check `--no-color` flag + `NO_COLOR` env var + TTY detection on stderr; call `set_no_color()` if any are true

**Tests:**
- [x] `color_enabled` respects `AtomicBool` toggle
- [x] `color_enabled` respects `NO_COLOR` env var
- [x] `spinner_frame` cycles correctly through all 10 frames
- [x] `dim`/`bold`/`success`/`error`/`warn`/`accent`/`muted` return unstyled text when color disabled

**No other tasks depend on the test file; all tasks depend on `src/style.rs` existing.**

---

### Task 2: Animated Spinner + Rich Status Line (depends Task 1)

**Files:** `src/display.rs`, `src/engine.rs`

**Steps:**
1. [x] Rewrite `print_run_start`, `print_run_elapsed`, `print_run_result` in `src/display.rs`
2. [x] New signatures accept `max_cycles` + `cumulative_cost` + `tick: usize` (for spinner frame)
3. [x] Status line format: `⠹  Cycle 3/10  │  builder  2/3  │  $1.47 total  │  02:34`
   - Spinner via `style::spinner_frame(tick)`
   - Separators (`│`) via `style::dim()`
   - Cycle number via `style::bold()`
   - Cost via `style::accent()`
   - Elapsed via `style::muted()`
4. [x] Engine poll loop (`src/engine.rs` ~line 837): pass tick counter + cumulative cost; update spinner every 100ms (already polling at 100ms; currently only updates display per second — remove the 1s gate)
5. [x] Non-TTY: suppress spinner animation (print static status line once per run, no carriage return rewrite)

**Tests:**
- [x] Status line format contains expected segments (cycle, phase, cost, elapsed)
- [x] Spinner frame advances on successive ticks
- [x] Non-TTY suppresses carriage-return rewrite

---

### Task 3: Styled Startup Header (depends Task 1)

**Files:** `src/display.rs`, `src/main.rs`

**Steps:**
1. [x] Rewrite `print_run_header` in `src/display.rs`
2. [x] New signature: accept workflow details struct or individual params (phases, max_cycles, budget_cap, context_dir, output_dir, version)
3. [x] Layout:
   ```
   rings v0.1.0                    ← style::bold()

     Workflow   my-task.rings.toml  ← label style::dim(), value plain
     Context    ./src
     Phases     builder ×10, reviewer ×1
     Max        50 cycles · 550 runs
     Budget     $5.00               ← style::accent()
     Output     ~/.local/share/...  ← style::muted()
   ```
4. [x] Budget line only shown when `budget_cap_usd` is Some
5. [x] Update call sites in `src/main.rs` (`run_inner` ~line 243, `resume_inner` ~line 560)

**Tests:**
- [x] Output contains expected labels (`Workflow`, `Context`, `Phases`, `Max`, `Output`)
- [x] Budget line present when budget_cap is Some, absent when None
- [x] Respects no-color (no ANSI escapes when color disabled)

---

### Task 4: Styled Cycle Transitions (depends Task 1)

**Files:** `src/display.rs`, `src/engine.rs`

**Steps:**
1. Rewrite `print_cycle_header` and `print_cycle_cost` in `src/display.rs`
2. Merge into a single `print_cycle_boundary(cycle: u32, prev_cycle_cost: Option<f64>)`
3. Format: `── Cycle 2 ────────────────────────── $0.14 prev ──`
   - Divider (`──`) via `style::dim()`
   - Cycle number via `style::bold()`
   - Cost via `style::accent()`
   - First cycle (no prev cost): `── Cycle 1 ──────────────────────────────────────`
4. Update call sites in `src/engine.rs` (~lines 597-603) to call the merged function

**Tests:**
- Output format matches spec pattern
- First cycle has no cost suffix
- Subsequent cycles show previous cycle cost in cyan

---

### Task 5: Richer Summaries — Completion, Cancellation, Errors (depends Task 1)

**Files:** `src/display.rs`, `src/style.rs` (or display.rs), `src/engine.rs`, `src/main.rs`

**Steps:**
1. Add bar chart rendering helper: `render_bar_chart(items: &[(String, f64, u32)], max_width: usize) -> Vec<String>`
   - `█` blocks proportional to cost share, max `max_width` chars wide (default 20)
   - Phase name left-aligned, cost in accent, run count in parens
2. Add budget gauge rendering helper: `render_budget_gauge(spent: f64, cap: f64, width: usize) -> String`
   - `█` for consumed, `░` for remaining
   - Color: green < 60%, yellow 60–85%, red > 85%
3. Rewrite `print_completion` to accept `phase_costs: &[(String, f64, u32)]` (name, cost, runs)
   - Green `✓` via `style::success()`
   - Cost values via `style::accent()`
   - Labels via `style::dim()`
   - Include bar chart and budget gauge
4. Expose `phase_costs` and `phase_run_counts` from `EngineResult` in `src/engine.rs`
5. Update `print_cancellation`: red `✗` via `style::error()`, resume command via `style::accent()` + `style::bold()`, include bar chart
6. Update `print_quota_error`, `print_auth_error`, `print_executor_error`: red `✗`, resume command bold cyan
7. Update `print_budget_cap_reached` with budget gauge
8. Update `print_parse_warnings` with yellow coloring via `style::warn()`
9. Update all call sites in `src/main.rs`

**Tests:**
- Bar chart proportions: 100% cost in one phase → full bar; 50/50 → equal bars
- Budget gauge: < 60% → green, 70% → yellow, 90% → red
- Budget gauge: 0% → all `░`, 100% → all `█`
- Phase breakdown format includes phase name, cost, run count
- Completion output includes `✓`; cancellation includes `✗`

---

### Task 6: Styled List + Dry Run (depends Task 1)

**Files:** `src/main.rs`

**Steps:**
1. Update `list_inner` in `src/main.rs` (~lines 787-824):
   - Header row via `style::bold()`
   - Status column: `completed` → `style::success()`, `incomplete`/`canceled` → `style::warn()`, `failed` → `style::error()`
   - Cost column via `style::accent()`
   - Divider lines via `style::dim()`
2. Update dry-run output block in `src/main.rs` (~lines 110-157):
   - Labels via `style::dim()`
   - Values via `style::bold()`
   - `✓` (signal found) via `style::success()`, `✗` (not found) via `style::error()`
   - Phase table header via `style::bold()`

**Tests:**
- List output applies success color to "completed" status
- List output applies error color to "failed" status
- Dry-run `✓` uses success styling, `✗` uses error styling

---
