## Batch: Completion Modes & Phase Contracts — 2026-03-19

**Features:** F-012 (Completion Signal Modes), F-013 (Completion Signal Phase Restriction), F-014 (Consumes Declaration), F-015 (Produces Declaration), F-016 (Produces Required Flag), F-017 (Advisory Contract Warnings), F-018 (Data Flow Documentation)

---

### Task 1: Schema/Model Prerequisites

**Files:** `src/workflow.rs`, `src/audit.rs`, `src/cli.rs`

All downstream tasks depend on these data model changes landing first.

**`CompletionSignalMode` enum (on `Workflow`, not `WorkflowConfig`):**
- Add to `src/workflow.rs`:
  ```rust
  #[derive(Debug, Clone)]
  pub enum CompletionSignalMode {
      Substring,
      Line,
      Regex(regex::Regex),  // compiled once in Workflow::validate()
  }
  impl Default for CompletionSignalMode { fn default() -> Self { Self::Substring } }
  ```
- `WorkflowConfig` keeps `completion_signal_mode: Option<String>` for TOML deserialization (no change)
- Change `Workflow.completion_signal_mode: String` → `CompletionSignalMode`
- In `Workflow::validate()`: parse the mode string; if `"regex"`, compile `completion_signal` via `Regex::new()`
- Add `WorkflowError::InvalidCompletionSignalMode(String)` and `WorkflowError::InvalidSignalRegex(String)`

**Phase name validation in `Workflow::validate()`:**
- After collecting phase names into `seen: HashSet<String>`, validate every entry in `completion_signal_phases` exists in `seen`
- Add `WorkflowError::UnknownCompletionSignalPhase(String)` for the first unknown name found

**Phase contract fields on `PhaseConfig`:**
- Add with `#[serde(default)]`:
  ```rust
  pub consumes: Vec<String>,
  pub produces: Vec<String>,
  pub produces_required: bool,
  ```
- In `Workflow::validate()`: if any phase has `produces_required = true` and `manifest_enabled = false`, return `WorkflowError::ProducesRequiredWithoutManifest(phase_name)`

**`CostEntry` schema extension (`src/audit.rs`):**
- Add field — do NOT add `#[serde(skip_serializing_if)]`; spec requires field always present:
  ```rust
  #[serde(default)]
  pub produces_violations: Vec<String>,
  ```

**CLI flags (`src/cli.rs`):**
- Add `#[arg(long)] no_contract_check: bool` to both `RunArgs` and `ResumeArgs`
- Thread through to `EngineConfig` with field `no_contract_check: bool`
- Suppression logic in engine: `skip_contract_checks = no_contract_check || no_completion_check`

**Tests:**
- [x] `completion_signal_mode = "regex"` with valid regex → parses to `CompletionSignalMode::Regex(_)`
- [x] `completion_signal_mode = "regex"` with `completion_signal = "["` → `Err(WorkflowError::InvalidSignalRegex(_))`
- [x] `completion_signal_mode = "bogus"` → `Err(WorkflowError::InvalidCompletionSignalMode(_))`
- [x] `completion_signal_phases = ["nonexistent"]` with only phase `"builder"` → `Err(WorkflowError::UnknownCompletionSignalPhase("nonexistent"))`
- [x] `completion_signal_phases = ["builder"]` with phase `"builder"` → parses cleanly
- [x] `produces_required = true` + `manifest_enabled = false` → `Err(WorkflowError::ProducesRequiredWithoutManifest(_))`
- [x] Old `CostEntry` JSONL line without `produces_violations` → deserializes to `produces_violations: []`
- [x] New `CostEntry` serializes `produces_violations: []` even when empty (field always present in JSON output)
- [x] `PhaseConfig` without `consumes`/`produces`/`produces_required` → defaults to empty vecs and `false`

**Steps:**
- [x] Add `CompletionSignalMode` enum to `src/workflow.rs`
- [x] Change `Workflow.completion_signal_mode` field type; update `validate()` to compile regex
- [x] Add `WorkflowError::InvalidCompletionSignalMode`, `InvalidSignalRegex`, `UnknownCompletionSignalPhase`, `ProducesRequiredWithoutManifest` variants
- [x] Add `completion_signal_phases` validation in `validate()` after phase name collection
- [x] Add `consumes`, `produces`, `produces_required` to `PhaseConfig` with `#[serde(default)]`
- [x] Add `produces_required` cross-field validation in `validate()`
- [x] Add `produces_violations: Vec<String>` to `CostEntry` with `#[serde(default)]`
- [x] Add `no_contract_check` to `RunArgs`, `ResumeArgs`, and `EngineConfig`

---

### Task 2: F-012 — Completion Signal Modes

**Files:** `src/completion.rs`, `src/engine.rs`, `src/dry_run.rs`, `tests/signal_modes.rs` (new)

**Depends on:** Task 1 (`CompletionSignalMode` enum on `Workflow`)

**Implementation:**
- Add to `src/completion.rs`:
  ```rust
  pub fn output_regex_matches_signal(output: &str, regex: &Regex) -> bool {
      regex.is_match(output)
  }
  ```
- Update `signal_matches` in `engine.rs` to accept `&CompletionSignalMode` instead of `&str`; match exhaustively:
  ```rust
  match mode {
      Substring => output_contains_signal(output, signal),
      Line      => output_line_contains_signal(output, signal),
      Regex(re) => output_regex_matches_signal(output, re),
  }
  ```
- Update `continue_signal` call site at `engine.rs:~1409`: `continue_signal` always uses **substring** mode regardless of `completion_signal_mode` — call `output_contains_signal` directly (see Open Decision OD-2)
- Update `dry_run.rs::check_completion_signal`: remove the redundant `Regex::new()` call; read the already-compiled regex from `Workflow.completion_signal_mode` (the `Regex(re)` variant); the startup advisory check for regex mode still does a literal substring search for the pattern string in the prompt (not running the regex against prompt text — this is intentional)
- Update all `signal_matches` call sites (including in test helpers in `tests/engine_integration.rs`) to pass `&CompletionSignalMode` instead of `&str`

**Tests (`tests/signal_modes.rs`):**
- [x] `output_regex_matches_signal`: valid regex matches → true
- [x] `output_regex_matches_signal`: valid regex no match → false
- [x] `output_regex_matches_signal`: anchored pattern `^DONE$` matches line of output
- [x] `output_regex_matches_signal`: capture group in pattern — still returns bool, no panic
- [x] `line` mode: `"  DONE  "` (leading/trailing whitespace) → match (trimmed)
- [x] `line` mode: `"DONE_EXTRA"` → no match (trim doesn't help superstring)
- [x] `line` mode: CRLF output `"DONE\r\n"` → match (`trim()` strips `\r`)
- [x] Regression: engine with `completion_signal_mode = "regex"` and matching output → exits 0 (fixes silent substring fallthrough bug)
- [x] `continue_signal` with `completion_signal_mode = "regex"` → continue_signal matched as substring, not regex
- [x] `dry_run` with `completion_signal_mode = "regex"`: signal found in prompt → `SignalCheck { found: true }`

**Steps:**
- [x] Add `output_regex_matches_signal` to `src/completion.rs`
- [x] Update `signal_matches` signature and body in `src/engine.rs`
- [x] Update `continue_signal` call site to always use substring
- [x] Update `dry_run.rs` to use compiled regex from `Workflow`
- [x] Update test helpers in `tests/engine_integration.rs` for new `signal_matches` signature
- [x] Write `tests/signal_modes.rs`

---

### Task 3: F-013 — Completion Signal Phase Restriction

**Files:** `tests/completion_phase_restriction.rs` (new)

**Depends on:** Task 1 (validation already added to `Workflow::validate()`)

The engine check at `engine.rs:~1382` already exists. Task 1 adds the startup validation. This task only adds integration test coverage.

**Tests (`tests/completion_phase_restriction.rs`):**
- [x] `completion_signal_phases = ["nonexistent"]` → `WorkflowError::UnknownCompletionSignalPhase` at `Workflow::from_str` time
- [x] Two-phase workflow (builder, reviewer); `completion_signal_phases = ["reviewer"]`; builder emits signal; run continues; reviewer emits signal → exits 0
- [x] Same setup; builder emits signal; `completion_eligible` is false for builder → signal recorded in logs but does not trigger completion
- [x] Empty `completion_signal_phases` → any phase can trigger completion (existing behavior unchanged)

**Steps:**
- [x] Write `tests/completion_phase_restriction.rs` with mock executor scenarios

---

### Task 4: F-014/F-015/F-016/F-017 — Phase Contracts

**Files:** `src/contracts.rs` (new), `src/engine.rs`, `src/lib.rs`, `tests/phase_contracts.rs` (new)

**Depends on:** Task 1 (fields on `PhaseConfig`, `CostEntry.produces_violations`, `EngineConfig.no_contract_check`)

**`src/contracts.rs`:**

```rust
/// Extract the literal prefix before the first glob metacharacter (*, ?, [).
/// Returns the full pattern if it contains no metacharacters.
pub fn non_glob_prefix(pattern: &str) -> &str { ... }

pub enum ContractWarning {
    ConsumesNoMatchStartup { phase: String, pattern: String },
    ConsumesNoMatchRun    { phase: String, pattern: String, cycle: u32, run: u32 },
}

/// Startup check per phase: for each consumes pattern, warn if no files in context_dir
/// match AND the pattern's non-glob prefix does not appear as a substring in prompt_text.
pub fn check_consumes_at_startup(
    phase_name: &str,
    consumes: &[String],
    context_dir: &Path,
    prompt_text: &str,
) -> Result<Vec<ContractWarning>>

/// Pre-run check (only called for cycle >= 2): warn if patterns still match nothing.
pub fn check_consumes_pre_run(
    phase_name: &str,
    consumes: &[String],
    context_dir: &Path,
    cycle: u32,
    run: u32,
) -> Result<Vec<ContractWarning>>

/// Post-run check: returns patterns that matched no files in added+modified.
/// Deleted files do NOT satisfy a produces pattern.
/// Returns [] when produces is empty or when manifest_enabled = false (caller gates).
pub fn check_produces_after_run(
    produces: &[String],
    diff_added: &[String],
    diff_modified: &[String],
) -> Vec<String>  // violated patterns
```

Use `globset::GlobSetBuilder` (already a dependency) for glob matching in all three functions.

**Non-glob prefix rule:** `non_glob_prefix(pattern)` returns everything before the first `*`, `?`, or `[`. If the pattern starts with a wildcard (e.g. `*.rs`), the prefix is `""`. When the prefix is empty, **skip the prompt-text suppression check** — an empty string is a substring of anything, which would always suppress the warning incorrectly. In that case, only file existence can suppress the warning.

**Engine integration (`src/engine.rs`):**
- Pre-loop: for each phase, if `consumes` non-empty and `!skip_contract_checks`: read resolved prompt text, call `check_consumes_at_startup`, emit warnings to stderr
- Pre-run (immediately before `executor.spawn`, when `run_spec.cycle >= 2`): call `check_consumes_pre_run` if `!skip_contract_checks`; emit warnings
- Post-run: **retain `FileDiff` paths** before they are discarded (currently only counts survive); call `check_produces_after_run`; emit warning to stderr; populate `cost_entry.produces_violations`
- After produces check: if `phase.produces_required && !violations.is_empty()`: write state, print error to stderr, return exit code 2

**Warning message formats (must match spec):**
```
⚠  Phase "reviewer" declares consumes = ["review-notes.md"]
   but no matching files exist in context_dir ("./src")
   and the pattern is not mentioned in the prompt.
   This phase may silently do nothing if its expected inputs are never created.
   Suppress with --no-contract-check or fix the consumes declaration.

⚠  Phase "reviewer" (run 9, cycle 2): consumes = ["review-notes.md"]
   but no matching files found in context_dir. The phase may operate on missing inputs.

⚠  Phase "builder" declared produces = ["src/**/*.rs", "tests/**/*.rs"]
   but no matching files were modified in run 7 (cycle 2, iteration 2/3).
   This may indicate the phase did not complete its intended work.
```

**Tests (`tests/phase_contracts.rs`):**
- [x] `non_glob_prefix("src/**/*.rs")` = `"src/"`
- [x] `non_glob_prefix("review-notes.md")` = `"review-notes.md"` (no metachar → full string)
- [x] `non_glob_prefix("*.rs")` = `""` (starts with metachar → empty prefix)
- [x] `check_consumes_at_startup`: file `review-notes.md` exists in context_dir → no warning
- [x] `check_consumes_at_startup`: no files, prompt contains `"src/"` (prefix of `"src/**/*.rs"`) → no warning
- [x] `check_consumes_at_startup`: no files, prompt does not contain prefix → warning fires with correct message text
- [x] `check_consumes_pre_run` cycle=2: no files match → per-run warning (different message from startup)
- [x] `check_produces_after_run`: `produces = ["src/**/*.rs"]`, `diff_added = ["src/main.rs"]` → `[]` (no violations)
- [x] `check_produces_after_run`: `produces = ["src/**/*.rs"]`, diff empty → `["src/**/*.rs"]`
- [x] `check_produces_after_run`: `produces = ["src/**/*.rs"]`, `diff_deleted = ["src/main.rs"]` only → violation (deleted doesn't count)
- [x] `check_produces_after_run`: `produces = []` → `[]` always
- [x] `check_produces_after_run`: partial match (2 patterns, 1 matched) → only unmatched in violations
- [x] Engine integration: `produces_violations` in `costs.jsonl` is `[]` when all matched, populated when not
- [x] Engine integration: `produces_required = true` + no matching files → exit code 2, state saved, stderr contains error
- [x] Engine integration: `produces_required = false` + no matching files → advisory warning, continues (exit 0 or 1)
- [x] Engine integration: `manifest_enabled = false` → produces check skipped, `produces_violations` always `[]`
- [x] Engine integration: `--no-contract-check` → no consumes or produces warnings emitted
- [ ] Engine integration: `--no-completion-check` → also suppresses contract warnings

**Steps:**
- [x] Create `src/contracts.rs` with `non_glob_prefix`, `check_consumes_at_startup`, `check_consumes_pre_run`, `check_produces_after_run`
- [x] Add `pub mod contracts;` to `src/lib.rs`
- [x] Retain `FileDiff` paths in the engine post-run section (before they are discarded)
- [x] Add pre-loop startup consumes check in engine
- [x] Add pre-run consumes check (cycle >= 2) in engine
- [x] Add post-run produces check in engine; populate `cost_entry.produces_violations`
- [x] Add `produces_required` hard-exit path in engine
- [x] Write `tests/phase_contracts.rs`

---

### Task 5: F-018 — Data Flow Documentation

**Files:** `src/inspect.rs` (new or extend), `src/engine.rs` (startup snapshot), `src/lib.rs`, `tests/inspect.rs` (new)

**Depends on:** Task 4 (`consumes`/`produces` on `PhaseConfig`)

**Workflow snapshot at run start:**
- At the beginning of `run_workflow`, write the per-phase `consumes`/`produces`/`produces_required` declarations to `{output_dir}/workflow_contracts.json`. This enables correct historical data-flow views when the workflow file has changed since the run.

**`src/inspect.rs`:**
```rust
pub struct DeclaredFlow {
    pub phase: String,
    pub consumes: Vec<String>,
    pub produces: Vec<String>,
}

pub struct ActualFileChange {
    pub path: String,
    pub phase: String,
    pub cycle: u32,
    pub run: u32,
    pub change_type: ChangeType,  // Added | Modified | Deleted
}

/// Render the declared data-flow graph (from workflow_contracts.json).
pub fn render_data_flow_declared(phases: &[DeclaredFlow]) -> String

/// Render actual file attribution (from correlating CostEntry run numbers with manifests).
pub fn render_data_flow_actual(changes: &[ActualFileChange]) -> String
```

Output format matches the spec (`phase-contracts.md` lines 124–141):
```
Data flow (declared):
  specs/**/*.md  ──→  [builder]  ──→  src/**/*.rs
                                      tests/**/*.rs
  src/**/*.rs   ──→  [reviewer] ──→  review-notes.md
  tests/**/*.rs ──→  [reviewer]
```

Also add `InspectView::ClaudeOutput` variant to the enum in `src/cli.rs` (stub, not implemented this batch) to complete the shape.

Wire into `cmd_inspect` for `InspectView::DataFlow`: load `workflow_contracts.json` from run directory; load actual changes by correlating `CostEntry` run numbers with manifest file pairs using `read_manifest_gz` + `diff_manifests`; render and print.

**Partial/canceled run behavior:** The declared view always renders (it comes from `workflow_contracts.json`, not execution data). For the actual view, render all changes that were recorded, then append a note if the run status is not `completed`: `(incomplete — run was canceled at cycle N, run M)`. Missing manifests for unstarted phases/runs are silently skipped; no error is raised.

**Tests (`tests/inspect.rs`):**
- [ ] `render_data_flow_declared`: 2-phase workflow with full consumes/produces → ASCII graph matches spec format
- [ ] `render_data_flow_declared`: phase with no contracts → renders without errors (shows phase name, no arrows)
- [ ] `render_data_flow_declared`: consumes-only phase (no produces) → one-sided arrow
- [ ] `render_data_flow_actual`: list of `ActualFileChange` entries → correct attribution by phase and cycle
- [ ] `rings inspect <run-id> --show data-flow` exits 0 and produces output (not stub error message)

**Steps:**
- [ ] Write `workflow_contracts.json` at run start in `src/engine.rs`
- [ ] Create `src/inspect.rs` with `DeclaredFlow`, `ActualFileChange`, `render_data_flow_declared`, `render_data_flow_actual`
- [ ] Add `InspectView::ClaudeOutput` to `src/cli.rs` enum (stub)
- [ ] Add `pub mod inspect;` to `src/lib.rs`
- [ ] Wire `InspectView::DataFlow` dispatch in `cmd_inspect` in `src/main.rs`
- [ ] Write `tests/inspect.rs`

---

### Open Decisions

| ID | Decision | Recommendation |
|----|----------|----------------|
| OD-1 | Exit code for `produces_required` violation | Use **2** (closest to "workflow enforcement error"); document timeout/produces_required collision in REVIEW.md |
| OD-2 | `continue_signal` mode | Always uses **substring** regardless of `completion_signal_mode`; spec doesn't address it; record in REVIEW.md |
| OD-3 | `--no-completion-check` suppression scope | Flags are **fully independent**: `--no-completion-check` = completion signal only; `--no-contract-check` = contract warnings only. `phase-contracts.md` corrected; `commands-and-flags.md` is authoritative. |
| OD-4 | Non-glob prefix computation | Everything before the first `*`, `?`, or `[`; if prefix is empty, prompt-text suppression is **skipped** (only file existence suppresses) |
| OD-5 | `produces` check — deleted files | Only `added` + `modified` satisfy a `produces` pattern; `deleted` does not |
| OD-6 | Historical data-flow snapshot | Write `workflow_contracts.json` to run output dir at startup |
| OD-7 | `produces = []` explicitly declared | Treat identically to absent; no warnings, `produces_violations` always `[]` |
| OD-8 | `InspectView::ClaudeOutput` | Add to enum now (stub, not wired); `#[command(hide = true)]` on `completions` subcommand when implementing |

### Spec Gaps

- `continue_signal` mode: resolved — always uses substring; record decision in REVIEW.md (spec does not address it)
- `--no-completion-check` suppression scope: resolved — flags are independent; `phase-contracts.md` corrected
- Exit code for `produces_required` violations: resolved — `phase-contracts.md` specifies exit code 2; `exit-codes.md` should be updated to mention this case explicitly
- Non-glob prefix computation: resolved — first `*`/`?`/`[`; empty prefix skips prompt-text check
- Data-flow view for canceled/incomplete runs: resolved — render available data, append canceled note
- Case sensitivity: resolved — all modes are case-sensitive; `completion-detection.md` updated

---
