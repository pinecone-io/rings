use crate::audit::{
    append_cost_entry, append_event, extract_resume_commands, write_run_log, BudgetCapEvent,
    BudgetWarningEvent, CostEntry,
};
use crate::cancel::CancelState;
use crate::completion::{output_contains_signal, output_line_contains_signal};
use crate::cost::parse_cost_from_output;
use crate::executor::{extract_response_text, Executor, Invocation};
use crate::state::{FailureReason, StateFile};
use crate::template::{render_prompt, TemplateVars};
use crate::workflow::PhaseConfig;
use crate::workflow::Workflow;
use anyhow::Result;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Result of interruptible sleep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepResult {
    Completed,
    Canceled,
}

/// Tracks cumulative and per-phase budget costs with rolling windows for spike detection.
#[derive(Debug)]
pub struct BudgetTracker {
    pub cumulative_cost: f64,
    pub phase_costs: HashMap<String, f64>,
    pub budget_warned_80_global: bool,
    pub budget_warned_90_global: bool,
    pub budget_warned_80_phase: HashMap<String, bool>,
    pub budget_warned_90_phase: HashMap<String, bool>,
    pub rolling_windows: HashMap<String, VecDeque<f64>>, // per-phase rolling window, cap 5
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BudgetTracker {
    pub fn new() -> Self {
        Self {
            cumulative_cost: 0.0,
            phase_costs: HashMap::new(),
            budget_warned_80_global: false,
            budget_warned_90_global: false,
            budget_warned_80_phase: HashMap::new(),
            budget_warned_90_phase: HashMap::new(),
            rolling_windows: HashMap::new(),
        }
    }

    /// Reconstruct BudgetTracker from costs.jsonl in a single pass.
    pub fn reconstruct_from_costs(path: &Path) -> Result<Self> {
        let mut tracker = Self::new();

        // If costs.jsonl doesn't exist, return empty tracker
        if !path.exists() {
            return Ok(tracker);
        }

        for entry_result in crate::audit::stream_cost_entries(path)? {
            let entry = entry_result.unwrap_or_else(|_| {
                // Skip malformed lines
                CostEntry {
                    run: 0,
                    cycle: 0,
                    phase: String::new(),
                    iteration: 0,
                    cost_usd: None,
                    input_tokens: None,
                    output_tokens: None,
                    cost_confidence: String::new(),
                    files_added: 0,
                    files_modified: 0,
                    files_deleted: 0,
                    files_changed: 0,
                    event: None,
                }
            });

            // Skip entries with None cost
            if let Some(cost) = entry.cost_usd {
                tracker.cumulative_cost += cost;

                // Update phase costs
                tracker
                    .phase_costs
                    .entry(entry.phase.clone())
                    .and_modify(|c| *c += cost)
                    .or_insert(cost);

                // Update rolling window for spike detection
                let window = tracker.rolling_windows.entry(entry.phase).or_default();
                if window.len() >= 5 {
                    window.pop_front();
                }
                window.push_back(cost);
            }
        }

        Ok(tracker)
    }

    /// Check if the cost spike is detected for a phase.
    /// Returns `Some(multiplier)` if cost > 5× rolling average, else `None`.
    /// Requires ≥3 entries in the rolling window; skips if all entries are 0.0.
    /// The average is computed excluding the current (last) entry to avoid the spike
    /// inflating the average itself.
    pub fn check_spike(&self, phase: &str) -> Option<f64> {
        let window = self.rolling_windows.get(phase)?;

        // Need at least 3 entries
        if window.len() < 3 {
            return None;
        }

        // Get the most recent cost (last entry) - the potential spike
        let current_cost = match window.back() {
            Some(&cost) => cost,
            None => return None,
        };

        // Calculate average of all entries EXCEPT the current one
        let sum_without_current: f64 = window.iter().take(window.len() - 1).sum();
        let avg = sum_without_current / (window.len() - 1) as f64;

        // Skip if average is zero (avoid division issues and false positives)
        if avg == 0.0 {
            return None;
        }

        // Spike if current > 5× average (strict >)
        if current_cost > 5.0 * avg {
            return Some(current_cost / avg);
        }

        None
    }

    /// Update the rolling window for a phase with a new cost.
    pub fn update_rolling_window(&mut self, phase: String, cost: f64) {
        let window = self.rolling_windows.entry(phase).or_default();
        if window.len() >= 5 {
            window.pop_front();
        }
        window.push_back(cost);
    }
}

/// Mutable state for the run_workflow loop.
#[derive(Debug)]
pub struct RunContext {
    pub total_runs: u32,
    pub last_cycle: u32,
    pub last_successful_run: u32,
    pub current_display_cycle: u32,
    pub parse_warnings: Vec<crate::cost::ParseWarning>,
    pub budget: BudgetTracker,
}

impl Default for RunContext {
    fn default() -> Self {
        Self::new()
    }
}

impl RunContext {
    pub fn new() -> Self {
        Self {
            total_runs: 0,
            last_cycle: 0,
            last_successful_run: 0,
            current_display_cycle: 0,
            parse_warnings: Vec::new(),
            budget: BudgetTracker::new(),
        }
    }
}

/// Exit reason for state snapshots.
#[derive(Debug, Clone, Copy)]
pub enum ExitReason {
    Success,
    Canceled,
    TimedOut,
    ExecutorError(FailureReason),
    BudgetCap,
    MaxCycles,
}

#[derive(Debug, Clone)]
pub struct RunSpec {
    pub global_run_number: u32,
    pub cycle: u32,
    pub phase_name: String,
    pub phase_index: usize,
    pub phase_iteration: u32,
    pub phase_total_iterations: u32,
    /// Resolved prompt text (after template substitution, if needed — populated by engine)
    pub prompt_text: Option<String>,
}

/// Position to resume from (all fields from the last completed run's state).
#[derive(Debug, Clone)]
pub struct ResumePoint {
    pub last_completed_run: u32,
    pub last_completed_cycle: u32,
    pub last_completed_phase_index: usize,
    pub last_completed_iteration: u32,
}

pub struct RunSchedule<'a> {
    phases: &'a [PhaseConfig],
    max_cycles: u32,
    current_cycle: u32,
    current_phase_index: usize,
    current_iteration: u32,
    global_run_number: u32,
}

impl<'a> RunSchedule<'a> {
    pub fn new(phases: &'a [PhaseConfig], max_cycles: u32) -> Self {
        Self {
            phases,
            max_cycles,
            current_cycle: 1,
            current_phase_index: 0,
            current_iteration: 1,
            global_run_number: 1,
        }
    }

    /// Advance past `last_completed_run` already-completed runs.
    pub fn resume_from(
        phases: &'a [PhaseConfig],
        max_cycles: u32,
        last_completed_run: u32,
    ) -> Self {
        let mut schedule = Self::new(phases, max_cycles);
        for _ in 0..last_completed_run {
            if schedule.next().is_none() {
                break;
            }
        }
        schedule
    }

    /// Resume from a saved position, using cycle/phase/iteration rather than a run count.
    /// This correctly handles workflows that used `continue_signal` to skip phases.
    pub fn resume_from_position(
        phases: &'a [PhaseConfig],
        max_cycles: u32,
        r: &ResumePoint,
    ) -> Self {
        let mut sched = Self {
            phases,
            max_cycles,
            current_cycle: r.last_completed_cycle,
            current_phase_index: r.last_completed_phase_index,
            current_iteration: r.last_completed_iteration,
            global_run_number: r.last_completed_run,
        };
        // Advance past the last completed position; discard the returned spec.
        sched.next();
        sched
    }

    /// Skip remaining phases in `current_cycle`, positioning at the start of the next cycle.
    /// If the schedule has already advanced past `current_cycle`, this is a no-op.
    pub fn skip_to_next_cycle(&mut self, current_cycle: u32) {
        if self.current_cycle == current_cycle {
            self.current_cycle += 1;
            self.current_phase_index = 0;
            self.current_iteration = 1;
        }
        // If current_cycle has already rolled over, we're already at the right position.
    }
}

impl<'a> Iterator for RunSchedule<'a> {
    type Item = RunSpec;

    fn next(&mut self) -> Option<RunSpec> {
        if self.current_cycle > self.max_cycles {
            return None;
        }
        if self.current_phase_index >= self.phases.len() {
            return None;
        }

        let phase = &self.phases[self.current_phase_index];
        let spec = RunSpec {
            global_run_number: self.global_run_number,
            cycle: self.current_cycle,
            phase_name: phase.name.clone(),
            phase_index: self.current_phase_index,
            phase_iteration: self.current_iteration,
            phase_total_iterations: phase.runs_per_cycle,
            prompt_text: None,
        };

        self.global_run_number += 1;
        self.current_iteration += 1;

        if self.current_iteration > phase.runs_per_cycle {
            self.current_iteration = 1;
            self.current_phase_index += 1;
            if self.current_phase_index >= self.phases.len() {
                self.current_phase_index = 0;
                self.current_cycle += 1;
            }
        }

        Some(spec)
    }
}

pub struct EngineConfig {
    pub output_dir: PathBuf,
    pub verbose: bool,
    pub run_id: String,
    pub workflow_file: String,
    pub ancestry_continuation_of: Option<String>,
    pub ancestry_depth: u32,
}

pub struct EngineResult {
    pub exit_code: i32,
    pub completed_cycles: u32,
    pub total_cost_usd: f64,
    pub total_runs: u32,
    pub parse_warnings: Vec<crate::cost::ParseWarning>,
    pub failure_reason: Option<FailureReason>,
}

/// Detect whether `signal` appears in `output` using the given mode.
/// mode "line" requires the signal to appear alone on a trimmed line.
/// Any other value (including "substring") uses substring matching.
fn signal_matches(output: &str, signal: &str, mode: &str) -> bool {
    if mode == "line" {
        output_line_contains_signal(output, signal)
    } else {
        output_contains_signal(output, signal)
    }
}

/// Check if a cost spike is detected for a phase.
/// Returns `Some(multiplier)` if cost > 5× rolling average, else `None`.
/// Requires ≥3 entries in the rolling window; skips if all entries are 0.
/// The average is computed excluding the current (last) entry to avoid the spike
/// inflating the average itself.
fn check_spike(rolling_windows: &HashMap<String, VecDeque<f64>>, phase: &str) -> Option<f64> {
    let window = rolling_windows.get(phase)?;

    // Need at least 3 entries total (to have 2+ for average after removing current)
    if window.len() < 3 {
        return None;
    }

    // Get the most recent cost (last entry) - the potential spike
    let current_cost = match window.back() {
        Some(&cost) => cost,
        None => return None,
    };

    // Calculate average of all entries EXCEPT the current one
    let sum_without_current: f64 = window.iter().take(window.len() - 1).sum();
    let avg = sum_without_current / (window.len() - 1) as f64;

    // Skip if average is zero (avoid false positives)
    if avg == 0.0 {
        return None;
    }

    // Spike if current > 5× average (strict >)
    if current_cost > 5.0 * avg {
        return Some(current_cost / avg);
    }

    None
}

/// Derive a short workflow name from a workflow file path (strips path and `.rings.toml` suffix).
fn workflow_name_from_file(workflow_file: &str) -> String {
    let stem = std::path::Path::new(workflow_file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    stem.trim_end_matches(".rings.toml").to_string()
}

/// Create a state snapshot for the current run context.
fn make_state_snapshot(
    ctx: &RunContext,
    config: &EngineConfig,
    run_spec: &RunSpec,
    reason: ExitReason,
) -> StateFile {
    let (canceled_at, failure_reason) = match reason {
        ExitReason::Canceled => (Some(chrono::Utc::now().to_rfc3339()), None),
        ExitReason::TimedOut => (None, Some(FailureReason::Timeout)),
        ExitReason::ExecutorError(reason) => (None, Some(reason)),
        ExitReason::Success | ExitReason::BudgetCap | ExitReason::MaxCycles => (None, None),
    };

    StateFile {
        schema_version: 1,
        run_id: config.run_id.clone(),
        workflow_file: config.workflow_file.clone(),
        last_completed_run: ctx.last_successful_run,
        last_completed_cycle: ctx.last_cycle,
        last_completed_phase_index: run_spec.phase_index,
        last_completed_iteration: run_spec.phase_iteration,
        total_runs_completed: ctx.total_runs,
        cumulative_cost_usd: ctx.budget.cumulative_cost,
        claude_resume_commands: vec![], // Will be populated by caller if needed
        canceled_at,
        failure_reason,
        ancestry: if config.ancestry_continuation_of.is_some() || config.ancestry_depth > 0 {
            Some(crate::state::AncestryInfo {
                parent_run_id: None, // parent_run_id is set in resume_inner, not in new runs
                continuation_of: config.ancestry_continuation_of.clone(),
                ancestry_depth: config.ancestry_depth,
            })
        } else {
            None
        },
    }
}

/// Save state to the state file (currently unused, kept for future use).
#[allow(dead_code)]
fn save_state(
    ctx: &RunContext,
    config: &EngineConfig,
    run_spec: &RunSpec,
    reason: ExitReason,
    state_path: &Path,
) -> Result<()> {
    let state = make_state_snapshot(ctx, config, run_spec, reason);
    state.write_atomic(state_path)
}

/// Sleep for up to `duration` while polling the cancel state at 100ms intervals.
/// Returns `Canceled` if the cancel state transitions to canceling during the sleep.
/// Returns `Completed` if the full duration elapses without cancellation.
pub fn interruptible_sleep(
    duration: std::time::Duration,
    cancel: Option<&Arc<CancelState>>,
    _tick_callback: impl FnMut(std::time::Duration),
) -> SleepResult {
    if duration.is_zero() {
        return SleepResult::Completed;
    }

    let deadline = std::time::Instant::now() + duration;
    while std::time::Instant::now() < deadline {
        // Check for cancellation
        if let Some(cs) = cancel {
            if cs.is_canceling() {
                return SleepResult::Canceled;
            }
        }

        let remaining = deadline - std::time::Instant::now();
        let sleep_duration = std::cmp::min(remaining, std::time::Duration::from_millis(100));
        if !sleep_duration.is_zero() {
            std::thread::sleep(sleep_duration);
        }
    }
    SleepResult::Completed
}

/// Run a workflow to completion (or until max_cycles, error, or cancellation).
/// Returns the exit code: 0 = signal detected, 1 = max_cycles, 3 = executor error, 130 = canceled.
pub fn run_workflow(
    workflow: &Workflow,
    executor: &dyn Executor,
    config: &EngineConfig,
    resume_from: Option<ResumePoint>,
    cancel: Option<Arc<CancelState>>,
) -> Result<EngineResult> {
    let runs_dir = config.output_dir.join("runs");
    let costs_path = config.output_dir.join("costs.jsonl");
    let state_path = config.output_dir.join("state.json");
    let events_path = config.output_dir.join("events.jsonl");

    std::fs::create_dir_all(&config.output_dir)?;

    // Initialize RunContext to consolidate mutable state
    let mut ctx = RunContext::new();
    let mut cycle_cost = 0.0f64;

    let workflow_name = workflow_name_from_file(&config.workflow_file);

    // Track which phases have been warned about unknown variables (once per phase)
    let mut warned_phases: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut schedule = match resume_from {
        None => RunSchedule::new(&workflow.phases, workflow.max_cycles),
        Some(ref r) => RunSchedule::resume_from_position(&workflow.phases, workflow.max_cycles, r),
    };

    // Restore cumulative cost from resume point if provided.
    if let Some(ref _r) = resume_from {
        // Reconstruct cumulative_cost from costs.jsonl
        if let Ok(content) = std::fs::read_to_string(&costs_path) {
            for line in content.lines() {
                if let Ok(entry) = serde_json::from_str::<crate::audit::CostEntry>(line.trim()) {
                    if let Some(cost) = entry.cost_usd {
                        ctx.budget.cumulative_cost += cost;
                        ctx.budget
                            .phase_costs
                            .entry(entry.phase.clone())
                            .and_modify(|c| *c += cost)
                            .or_insert(cost);

                        // Update rolling window for spike detection
                        let window = ctx.budget.rolling_windows.entry(entry.phase).or_default();
                        if window.len() >= 5 {
                            window.pop_front();
                        }
                        window.push_back(cost);
                    }
                }
            }
        }

        // Initialize warning flags based on reconstructed costs
        if let Some(cap) = workflow.budget_cap_usd {
            let pct_global = (ctx.budget.cumulative_cost / cap * 100.0) as u32;
            ctx.budget.budget_warned_80_global = pct_global >= 80;
            ctx.budget.budget_warned_90_global = pct_global >= 90;
        }

        for phase in &workflow.phases {
            if let Some(cap) = phase.budget_cap_usd {
                if let Some(&phase_cost) = ctx.budget.phase_costs.get(&phase.name) {
                    let pct = (phase_cost / cap * 100.0) as u32;
                    if pct >= 80 {
                        ctx.budget
                            .budget_warned_80_phase
                            .insert(phase.name.clone(), true);
                    }
                    if pct >= 90 {
                        ctx.budget
                            .budget_warned_90_phase
                            .insert(phase.name.clone(), true);
                    }
                }
            }
        }
    }

    while let Some(run_spec) = schedule.next() {
        ctx.last_cycle = run_spec.cycle;

        // Check if entering a new cycle
        if run_spec.cycle != ctx.current_display_cycle {
            // Print cost for previous cycle if not the first one
            if ctx.current_display_cycle > 0 {
                crate::display::print_cycle_cost(cycle_cost);
            }
            // Print header for new cycle
            crate::display::print_cycle_header(run_spec.cycle, workflow.max_cycles);
            cycle_cost = 0.0;
            ctx.current_display_cycle = run_spec.cycle;
        }

        // Resolve prompt text
        let raw_prompt = match (
            &workflow.phases[run_spec.phase_index].prompt_text,
            &workflow.phases[run_spec.phase_index].prompt,
        ) {
            (Some(text), _) => text.clone(),
            (None, Some(file)) => std::fs::read_to_string(file)?,
            _ => unreachable!("workflow validation ensures one of these exists"),
        };

        // Scan for unknown variables once per phase
        if !warned_phases.contains(&run_spec.phase_name) {
            warned_phases.insert(run_spec.phase_name.clone());
            let unknown_vars =
                crate::template::find_unknown_variables(&raw_prompt, crate::template::KNOWN_VARS);
            for unknown_var in unknown_vars {
                let timestamp =
                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
                let event = serde_json::json!({
                    "event": "advisory_warning",
                    "run_id": config.run_id,
                    "phase": run_spec.phase_name,
                    "warning_type": "unknown_variable",
                    "message": format!("Unknown template variable '{{{{{}}}}}' will appear literally in the prompt", unknown_var),
                    "timestamp": timestamp,
                });
                crate::audit::append_event(&events_path, &event)?;
            }
        }

        let vars = TemplateVars {
            phase_name: run_spec.phase_name.clone(),
            cycle: run_spec.cycle,
            max_cycles: Some(workflow.max_cycles),
            run: run_spec.global_run_number,
            iteration: run_spec.phase_iteration,
            runs_per_cycle: run_spec.phase_total_iterations,
            cost_so_far_usd: ctx.budget.cumulative_cost,
            workflow_name: workflow_name.clone(),
            context_dir: workflow.context_dir.clone(),
        };
        let prompt = render_prompt(&raw_prompt, &vars);

        let invocation = Invocation {
            prompt,
            context_dir: PathBuf::from(&workflow.context_dir),
        };

        // Determine timeout for this run: per-phase timeout overrides global.
        let timeout_secs = workflow.phases[run_spec.phase_index]
            .timeout_per_run_secs
            .as_ref()
            .and_then(|d| d.to_secs().ok())
            .or(workflow.timeout_per_run_secs);

        // Spawn the subprocess and implement wait loop with timeout/cancellation.
        let run_start = std::time::Instant::now();
        let mut handle = executor.spawn(&invocation, config.verbose)?;
        let timeout_deadline =
            timeout_secs.map(|secs| run_start + std::time::Duration::from_secs(secs));

        let mut output = crate::executor::ExecutorOutput {
            combined: String::new(),
            exit_code: -1,
        };
        let mut timeout_occurred = false;
        let mut cancel_occurred = false;

        loop {
            // Check ForceKill first (highest priority)
            if let Some(ref cancel_state) = cancel {
                if cancel_state.is_force_kill() {
                    let _ = handle.send_sigkill();
                    // Wait briefly for output collection
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    // Try to get the output, fallback to partial if process doesn't exit
                    output = match handle.try_wait() {
                        Ok(Some(out)) => out,
                        _ => match handle.partial_output() {
                            Ok(partial) => crate::executor::ExecutorOutput {
                                combined: partial,
                                exit_code: 137,
                            },
                            Err(_) => crate::executor::ExecutorOutput {
                                combined: String::new(),
                                exit_code: 137,
                            },
                        },
                    };
                    cancel_occurred = true;
                    break;
                }
            }

            // Check Canceling (second priority)
            if let Some(ref cancel_state) = cancel {
                if cancel_state.is_canceling() && !cancel_occurred {
                    let _ = handle.send_sigterm();
                    // Save state immediately after SIGTERM
                    let mut state =
                        make_state_snapshot(&ctx, config, &run_spec, ExitReason::Canceled);
                    state.claude_resume_commands = vec![];
                    let _ = state.write_atomic(&state_path);

                    // Wait up to 5s for graceful shutdown
                    let grace_deadline =
                        std::time::Instant::now() + std::time::Duration::from_secs(5);
                    let mut exited_gracefully = false;
                    loop {
                        // Check force_kill again at the start of each iteration (F-053 fix)
                        if cancel_state.is_force_kill() {
                            let _ = handle.send_sigkill();
                            break;
                        }

                        match handle.try_wait() {
                            Ok(Some(out)) => {
                                output = out;
                                exited_gracefully = true;
                                break;
                            }
                            Ok(None) => {
                                if std::time::Instant::now() >= grace_deadline {
                                    // Grace period expired, SIGKILL
                                    let _ = handle.send_sigkill();
                                    break;
                                }
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            }
                            Err(_) => break,
                        }
                    }

                    // If didn't exit gracefully, collect partial output
                    if !exited_gracefully {
                        output = match handle.partial_output() {
                            Ok(partial) => crate::executor::ExecutorOutput {
                                combined: partial,
                                exit_code: 130,
                            },
                            Err(_) => crate::executor::ExecutorOutput {
                                combined: String::new(),
                                exit_code: 130,
                            },
                        };
                    }
                    cancel_occurred = true;
                    break;
                }
            }

            // Check timeout (third priority)
            if let Some(deadline) = timeout_deadline {
                if std::time::Instant::now() >= deadline {
                    let _ = handle.send_sigterm();
                    // Save state immediately after SIGTERM
                    let mut state =
                        make_state_snapshot(&ctx, config, &run_spec, ExitReason::TimedOut);
                    state.claude_resume_commands = vec![];
                    let _ = state.write_atomic(&state_path);

                    // Wait up to 5s for graceful shutdown
                    let grace_deadline =
                        std::time::Instant::now() + std::time::Duration::from_secs(5);
                    let mut exited_gracefully = false;
                    loop {
                        // Check force_kill again at the start of each iteration (F-053 fix)
                        if let Some(ref cancel_state) = cancel {
                            if cancel_state.is_force_kill() {
                                let _ = handle.send_sigkill();
                                break;
                            }
                        }

                        match handle.try_wait() {
                            Ok(Some(out)) => {
                                output = out;
                                exited_gracefully = true;
                                break;
                            }
                            Ok(None) => {
                                if std::time::Instant::now() >= grace_deadline {
                                    // Grace period expired, SIGKILL
                                    let _ = handle.send_sigkill();
                                    break;
                                }
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            }
                            Err(_) => break,
                        }
                    }

                    // If didn't exit gracefully, collect partial output with timeout exit code
                    if !exited_gracefully {
                        output = match handle.partial_output() {
                            Ok(partial) => crate::executor::ExecutorOutput {
                                combined: partial,
                                exit_code: 2,
                            },
                            Err(_) => crate::executor::ExecutorOutput {
                                combined: String::new(),
                                exit_code: 2,
                            },
                        };
                    }
                    timeout_occurred = true;
                    break;
                }
            }

            // Try to wait for normal completion
            match handle.try_wait() {
                Ok(Some(out)) => {
                    output = out;
                    break;
                }
                Ok(None) => {
                    // Process still running, poll again in 100ms
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => {
                    // Error waiting for process
                    return Err(e);
                }
            }
        }

        let elapsed_secs = run_start.elapsed().as_secs();

        // When --output-format json is used, cost lives in the JSON object and
        // the text response is in the `result` field. Extract the text for
        // signal matching and resume command detection.
        let response_text = extract_response_text(&output.combined);

        // Record cost
        let cost = parse_cost_from_output(&output.combined);
        ctx.budget.cumulative_cost += cost.cost_usd.unwrap_or(0.0);
        ctx.total_runs += 1;

        // Accumulate low-confidence parse warnings
        if matches!(
            cost.confidence,
            crate::cost::ParseConfidence::Low | crate::cost::ParseConfidence::None
        ) {
            ctx.parse_warnings.push(crate::cost::ParseWarning {
                run_number: run_spec.global_run_number,
                cycle: run_spec.cycle,
                phase: run_spec.phase_name.clone(),
                confidence: cost.confidence.clone(),
                raw_match: cost.raw_match.clone(),
            });
        }

        // Print per-run result
        crate::display::print_run_result(&run_spec, cost.cost_usd.unwrap_or(0.0), elapsed_secs);

        // Accumulate cycle cost
        cycle_cost += cost.cost_usd.unwrap_or(0.0);

        // Write run log
        write_run_log(&runs_dir, run_spec.global_run_number, &output.combined)?;

        // Handle timeout
        if timeout_occurred {
            // Print final cycle cost before returning
            if ctx.current_display_cycle > 0 {
                crate::display::print_cycle_cost(cycle_cost);
            }
            append_cost_entry(
                &costs_path,
                &CostEntry {
                    run: run_spec.global_run_number,
                    cycle: run_spec.cycle,
                    phase: run_spec.phase_name.clone(),
                    iteration: run_spec.phase_iteration,
                    cost_usd: cost.cost_usd,
                    input_tokens: cost.input_tokens,
                    output_tokens: cost.output_tokens,
                    cost_confidence: format!("{:?}", cost.confidence).to_lowercase(),
                    files_added: 0,
                    files_modified: 0,
                    files_deleted: 0,
                    files_changed: 0,
                    event: None,
                },
            )?;
            return Ok(EngineResult {
                exit_code: 2,
                completed_cycles: ctx.last_cycle,
                total_cost_usd: ctx.budget.cumulative_cost,
                total_runs: ctx.total_runs,
                parse_warnings: ctx.parse_warnings,
                failure_reason: None,
            });
        }

        // Handle cancellation
        if cancel_occurred {
            // Print final cycle cost before returning
            if ctx.current_display_cycle > 0 {
                crate::display::print_cycle_cost(cycle_cost);
            }
            append_cost_entry(
                &costs_path,
                &CostEntry {
                    run: run_spec.global_run_number,
                    cycle: run_spec.cycle,
                    phase: run_spec.phase_name.clone(),
                    iteration: run_spec.phase_iteration,
                    cost_usd: cost.cost_usd,
                    input_tokens: cost.input_tokens,
                    output_tokens: cost.output_tokens,
                    cost_confidence: format!("{:?}", cost.confidence).to_lowercase(),
                    files_added: 0,
                    files_modified: 0,
                    files_deleted: 0,
                    files_changed: 0,
                    event: None,
                },
            )?;
            return Ok(EngineResult {
                exit_code: 130,
                completed_cycles: ctx.last_cycle,
                total_cost_usd: ctx.budget.cumulative_cost,
                total_runs: ctx.total_runs,
                parse_warnings: ctx.parse_warnings,
                failure_reason: None,
            });
        }

        // Handle executor error — save state with PREVIOUS completed run so the
        // failing run will be retried on resume.
        let resume_commands = extract_resume_commands(&response_text);
        if output.exit_code != 0 {
            // Classify the error based on output patterns
            let failure_reason = {
                let output_str = &output.combined;
                let profile = &workflow.compiled_error_profile;
                // Check quota patterns first (first-match-wins)
                let mut reason = FailureReason::Unknown;
                for regex in &profile.quota_regexes {
                    if regex.is_match(output_str) {
                        reason = FailureReason::Quota;
                        break;
                    }
                }
                // If no quota match, check auth patterns
                if matches!(reason, FailureReason::Unknown) {
                    for regex in &profile.auth_regexes {
                        if regex.is_match(output_str) {
                            reason = FailureReason::Auth;
                            break;
                        }
                    }
                }
                reason
            };

            // Print final cycle cost before returning
            if ctx.current_display_cycle > 0 {
                crate::display::print_cycle_cost(cycle_cost);
            }
            // Write state BEFORE costs.jsonl to prevent duplicate cost entries on resume.
            let mut state = make_state_snapshot(
                &ctx,
                config,
                &run_spec,
                ExitReason::ExecutorError(failure_reason),
            );
            state.claude_resume_commands = resume_commands;
            state.write_atomic(&state_path)?;
            append_cost_entry(
                &costs_path,
                &CostEntry {
                    run: run_spec.global_run_number,
                    cycle: run_spec.cycle,
                    phase: run_spec.phase_name.clone(),
                    iteration: run_spec.phase_iteration,
                    cost_usd: cost.cost_usd,
                    input_tokens: cost.input_tokens,
                    output_tokens: cost.output_tokens,
                    cost_confidence: format!("{:?}", cost.confidence).to_lowercase(),
                    files_added: 0,
                    files_modified: 0,
                    files_deleted: 0,
                    files_changed: 0,
                    event: None,
                },
            )?;
            return Ok(EngineResult {
                exit_code: 3,
                completed_cycles: ctx.last_cycle,
                total_cost_usd: ctx.budget.cumulative_cost,
                total_runs: ctx.total_runs,
                parse_warnings: ctx.parse_warnings,
                failure_reason: Some(failure_reason),
            });
        }

        // Run succeeded — persist state first (C-2: state before costs prevents duplicate entries
        // on resume if interrupted between the two writes).
        let mut state = make_state_snapshot(&ctx, config, &run_spec, ExitReason::Success);
        state.claude_resume_commands = resume_commands.clone();
        state.write_atomic(&state_path)?;

        // Append cost entry after state is safely checkpointed.
        append_cost_entry(
            &costs_path,
            &CostEntry {
                run: run_spec.global_run_number,
                cycle: run_spec.cycle,
                phase: run_spec.phase_name.clone(),
                iteration: run_spec.phase_iteration,
                cost_usd: cost.cost_usd,
                input_tokens: cost.input_tokens,
                output_tokens: cost.output_tokens,
                cost_confidence: format!("{:?}", cost.confidence).to_lowercase(),
                files_added: 0,
                files_modified: 0,
                files_deleted: 0,
                files_changed: 0,
                event: None,
            },
        )?;

        ctx.last_successful_run = run_spec.global_run_number;

        // Update phase costs and check budget caps
        ctx.budget
            .phase_costs
            .entry(run_spec.phase_name.clone())
            .and_modify(|c| *c += cost.cost_usd.unwrap_or(0.0))
            .or_insert(cost.cost_usd.unwrap_or(0.0));

        // Update rolling window for spike detection (only if cost is Some)
        if let Some(cost_val) = cost.cost_usd {
            let window = ctx
                .budget
                .rolling_windows
                .entry(run_spec.phase_name.clone())
                .or_default();
            if window.len() >= 5 {
                window.pop_front();
            }
            window.push_back(cost_val);

            // Check for cost spike
            if let Some(multiplier) = check_spike(&ctx.budget.rolling_windows, &run_spec.phase_name)
            {
                let timestamp =
                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
                let event = serde_json::json!({
                    "event": "advisory_warning",
                    "run_id": config.run_id,
                    "phase": run_spec.phase_name,
                    "warning_type": "cost_spike",
                    "message": format!("Cost spike detected: ${:.2} is {:.1}× the rolling average", cost_val, multiplier),
                    "multiplier": multiplier,
                    "timestamp": timestamp,
                });
                crate::audit::append_event(&events_path, &event)?;
            }
        }

        // Check global budget cap
        if let Some(cap) = workflow.budget_cap_usd {
            let pct = (ctx.budget.cumulative_cost / cap * 100.0) as u32;

            // ≥100%: budget cap reached
            if ctx.budget.cumulative_cost >= cap {
                // Print final cycle cost before returning
                if ctx.current_display_cycle > 0 {
                    crate::display::print_cycle_cost(cycle_cost);
                }
                // Print budget cap reached message
                crate::display::print_budget_cap_reached(cap, ctx.budget.cumulative_cost);

                // Emit budget_cap event
                let event = BudgetCapEvent {
                    event: "budget_cap".to_string(),
                    run_id: config.run_id.clone(),
                    cost_usd: ctx.budget.cumulative_cost,
                    budget_cap_usd: cap,
                    scope: "global".to_string(),
                    runs_completed: ctx.total_runs,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = append_event(&events_path, &serde_json::to_value(&event)?);

                // Save state before returning
                let state = make_state_snapshot(&ctx, config, &run_spec, ExitReason::BudgetCap);
                state.write_atomic(&state_path)?;

                return Ok(EngineResult {
                    exit_code: 4,
                    completed_cycles: ctx.last_cycle,
                    total_cost_usd: ctx.budget.cumulative_cost,
                    total_runs: ctx.total_runs,
                    parse_warnings: ctx.parse_warnings,
                    failure_reason: None,
                });
            }

            // ≥80%: warning (once)
            if pct >= 80 && !ctx.budget.budget_warned_80_global {
                ctx.budget.budget_warned_80_global = true;
                eprintln!(
                    "⚠  Budget: ${:.2} spent — 80% of ${:.2} cap.",
                    ctx.budget.cumulative_cost, cap
                );
                let event = BudgetWarningEvent {
                    event: "budget_warning".to_string(),
                    run_id: config.run_id.clone(),
                    cost_usd: ctx.budget.cumulative_cost,
                    budget_cap_usd: cap,
                    pct: 80,
                    scope: "global".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = append_event(&events_path, &serde_json::to_value(&event)?);
            }

            // ≥90%: warning (once)
            if pct >= 90 && !ctx.budget.budget_warned_90_global {
                ctx.budget.budget_warned_90_global = true;
                eprintln!(
                    "⚠  Budget: ${:.2} spent — 90% of ${:.2} cap. Approaching limit.",
                    ctx.budget.cumulative_cost, cap
                );
                let event = BudgetWarningEvent {
                    event: "budget_warning".to_string(),
                    run_id: config.run_id.clone(),
                    cost_usd: ctx.budget.cumulative_cost,
                    budget_cap_usd: cap,
                    pct: 90,
                    scope: "global".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = append_event(&events_path, &serde_json::to_value(&event)?);
            }
        }

        // Check per-phase budget caps
        for phase in &workflow.phases {
            if let Some(cap) = phase.budget_cap_usd {
                if let Some(&phase_cost) = ctx.budget.phase_costs.get(&phase.name) {
                    let pct = (phase_cost / cap * 100.0) as u32;

                    // ≥100%: phase budget cap reached
                    if phase_cost >= cap {
                        // Print final cycle cost before returning
                        if ctx.current_display_cycle > 0 {
                            crate::display::print_cycle_cost(cycle_cost);
                        }
                        // Print budget cap reached message
                        crate::display::print_budget_cap_reached(cap, phase_cost);

                        // Emit budget_cap event with phase scope
                        let event = BudgetCapEvent {
                            event: "budget_cap".to_string(),
                            run_id: config.run_id.clone(),
                            cost_usd: phase_cost,
                            budget_cap_usd: cap,
                            scope: format!("phase:{}", phase.name),
                            runs_completed: ctx.total_runs,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        let _ = append_event(&events_path, &serde_json::to_value(&event)?);

                        // Save state before returning
                        let state =
                            make_state_snapshot(&ctx, config, &run_spec, ExitReason::BudgetCap);
                        state.write_atomic(&state_path)?;

                        return Ok(EngineResult {
                            exit_code: 4,
                            completed_cycles: ctx.last_cycle,
                            total_cost_usd: ctx.budget.cumulative_cost,
                            total_runs: ctx.total_runs,
                            parse_warnings: ctx.parse_warnings,
                            failure_reason: None,
                        });
                    }

                    // ≥80%: warning (once per phase)
                    if pct >= 80
                        && !ctx
                            .budget
                            .budget_warned_80_phase
                            .get(&phase.name)
                            .copied()
                            .unwrap_or(false)
                    {
                        ctx.budget
                            .budget_warned_80_phase
                            .insert(phase.name.clone(), true);
                        eprintln!(
                            "⚠  Budget: ${:.2} spent — 80% of ${:.2} cap (phase: {}).",
                            phase_cost, cap, phase.name
                        );
                        let event = BudgetWarningEvent {
                            event: "budget_warning".to_string(),
                            run_id: config.run_id.clone(),
                            cost_usd: phase_cost,
                            budget_cap_usd: cap,
                            pct: 80,
                            scope: format!("phase:{}", phase.name),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        let _ = append_event(&events_path, &serde_json::to_value(&event)?);
                    }

                    // ≥90%: warning (once per phase)
                    if pct >= 90
                        && !ctx
                            .budget
                            .budget_warned_90_phase
                            .get(&phase.name)
                            .copied()
                            .unwrap_or(false)
                    {
                        ctx.budget
                            .budget_warned_90_phase
                            .insert(phase.name.clone(), true);
                        eprintln!("⚠  Budget: ${:.2} spent — 90% of ${:.2} cap. Approaching limit (phase: {}).", phase_cost, cap, phase.name);
                        let event = BudgetWarningEvent {
                            event: "budget_warning".to_string(),
                            run_id: config.run_id.clone(),
                            cost_usd: phase_cost,
                            budget_cap_usd: cap,
                            pct: 90,
                            scope: format!("phase:{}", phase.name),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        let _ = append_event(&events_path, &serde_json::to_value(&event)?);
                    }
                }
            }
        }

        // Check for cancellation (Ctrl+C)
        if let Some(ref cancel_state) = cancel {
            if cancel_state.is_canceling() {
                // Print final cycle cost before returning
                if ctx.current_display_cycle > 0 {
                    crate::display::print_cycle_cost(cycle_cost);
                }
                // Save state with canceled_at timestamp before returning
                let mut state = make_state_snapshot(&ctx, config, &run_spec, ExitReason::Canceled);
                state.claude_resume_commands = resume_commands.clone();
                state.write_atomic(&state_path)?;
                return Ok(EngineResult {
                    exit_code: 130,
                    completed_cycles: ctx.last_cycle,
                    total_cost_usd: ctx.budget.cumulative_cost,
                    total_runs: ctx.total_runs,
                    parse_warnings: ctx.parse_warnings,
                    failure_reason: None,
                });
            }
        }

        // Check completion signal (respecting completion_signal_phases if configured).
        let completion_eligible = workflow.completion_signal_phases.is_empty()
            || workflow
                .completion_signal_phases
                .contains(&run_spec.phase_name);
        if completion_eligible
            && signal_matches(
                &response_text,
                &workflow.completion_signal,
                &workflow.completion_signal_mode,
            )
        {
            // Print final cycle cost before returning
            if ctx.current_display_cycle > 0 {
                crate::display::print_cycle_cost(cycle_cost);
            }
            return Ok(EngineResult {
                exit_code: 0,
                completed_cycles: ctx.last_cycle,
                total_cost_usd: ctx.budget.cumulative_cost,
                total_runs: ctx.total_runs,
                parse_warnings: ctx.parse_warnings,
                failure_reason: None,
            });
        }

        // Check continue_signal: skip remaining phases in this cycle.
        if let Some(ref cs) = workflow.continue_signal {
            if signal_matches(&response_text, cs, &workflow.completion_signal_mode) {
                schedule.skip_to_next_cycle(run_spec.cycle);
            }
        }

        // Inter-run delay: poll in 100ms slices so cancellation is detected promptly.
        if workflow.delay_between_runs > 0 {
            let duration = std::time::Duration::from_secs(workflow.delay_between_runs);
            let sleep_result = interruptible_sleep(duration, cancel.as_ref(), |_elapsed| {});
            if sleep_result == SleepResult::Canceled {
                break;
            }
        }
    }

    // Print final cycle cost before returning
    if ctx.current_display_cycle > 0 {
        crate::display::print_cycle_cost(cycle_cost);
    }

    Ok(EngineResult {
        exit_code: 1,
        completed_cycles: ctx.last_cycle,
        total_cost_usd: ctx.budget.cumulative_cost,
        total_runs: ctx.total_runs,
        parse_warnings: ctx.parse_warnings,
        failure_reason: None,
    })
}
