use crate::audit::{
    append_cost_entry, append_event, extract_resume_commands, write_run_log, BudgetCapEvent,
    BudgetWarningEvent, CostEntry,
};
use crate::cancel::CancelState;
use crate::completion::{output_contains_signal, output_line_contains_signal};
use crate::cost::parse_cost_from_output;
use crate::executor::{extract_response_text, Executor, Invocation};
use crate::state::StateFile;
use crate::template::{render_prompt, TemplateVars};
use crate::workflow::PhaseConfig;
use crate::workflow::Workflow;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

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
}

pub struct EngineResult {
    pub exit_code: i32,
    pub completed_cycles: u32,
    pub total_cost_usd: f64,
    pub total_runs: u32,
    pub parse_warnings: Vec<crate::cost::ParseWarning>,
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

/// Derive a short workflow name from a workflow file path (strips path and `.rings.toml` suffix).
fn workflow_name_from_file(workflow_file: &str) -> String {
    let stem = std::path::Path::new(workflow_file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    stem.trim_end_matches(".rings.toml").to_string()
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

    let mut cumulative_cost = 0.0f64;
    let mut total_runs = 0u32;
    let mut last_cycle = 0u32;
    let mut last_successful_run: u32 = 0;
    let mut current_display_cycle = 0u32;
    let mut cycle_cost = 0.0f64;

    // Budget cap tracking
    let mut phase_costs: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let mut budget_warned_80_global = false;
    let mut budget_warned_90_global = false;
    let mut budget_warned_80_phase: std::collections::HashMap<String, bool> =
        std::collections::HashMap::new();
    let mut budget_warned_90_phase: std::collections::HashMap<String, bool> =
        std::collections::HashMap::new();
    let parse_warnings = Vec::new();

    let workflow_name = workflow_name_from_file(&config.workflow_file);

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
                        cumulative_cost += cost;
                        phase_costs
                            .entry(entry.phase.clone())
                            .and_modify(|c| *c += cost)
                            .or_insert(cost);
                    }
                }
            }
        }

        // Initialize warning flags based on reconstructed costs
        if let Some(cap) = workflow.budget_cap_usd {
            let pct_global = (cumulative_cost / cap * 100.0) as u32;
            budget_warned_80_global = pct_global >= 80;
            budget_warned_90_global = pct_global >= 90;
        }

        for phase in &workflow.phases {
            if let Some(cap) = phase.budget_cap_usd {
                if let Some(&phase_cost) = phase_costs.get(&phase.name) {
                    let pct = (phase_cost / cap * 100.0) as u32;
                    if pct >= 80 {
                        budget_warned_80_phase.insert(phase.name.clone(), true);
                    }
                    if pct >= 90 {
                        budget_warned_90_phase.insert(phase.name.clone(), true);
                    }
                }
            }
        }
    }

    while let Some(run_spec) = schedule.next() {
        last_cycle = run_spec.cycle;

        // Check if entering a new cycle
        if run_spec.cycle != current_display_cycle {
            // Print cost for previous cycle if not the first one
            if current_display_cycle > 0 {
                crate::display::print_cycle_cost(cycle_cost);
            }
            // Print header for new cycle
            crate::display::print_cycle_header(run_spec.cycle, workflow.max_cycles);
            cycle_cost = 0.0;
            current_display_cycle = run_spec.cycle;
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

        let vars = TemplateVars {
            phase_name: run_spec.phase_name.clone(),
            cycle: run_spec.cycle,
            max_cycles: Some(workflow.max_cycles),
            run: run_spec.global_run_number,
            iteration: run_spec.phase_iteration,
            runs_per_cycle: run_spec.phase_total_iterations,
            cost_so_far_usd: cumulative_cost,
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
                    let state = StateFile {
                        schema_version: 1,
                        run_id: config.run_id.clone(),
                        workflow_file: config.workflow_file.clone(),
                        last_completed_run: last_successful_run,
                        last_completed_cycle: last_cycle,
                        last_completed_phase_index: run_spec.phase_index,
                        last_completed_iteration: run_spec.phase_iteration,
                        total_runs_completed: total_runs,
                        cumulative_cost_usd: cumulative_cost,
                        claude_resume_commands: vec![],
                        canceled_at: Some(chrono::Utc::now().to_rfc3339()),
                        failure_reason: None,
                    };
                    let _ = state.write_atomic(&state_path);

                    // Wait up to 5s for graceful shutdown
                    let grace_deadline =
                        std::time::Instant::now() + std::time::Duration::from_secs(5);
                    let mut exited_gracefully = false;
                    loop {
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
                    let state = StateFile {
                        schema_version: 1,
                        run_id: config.run_id.clone(),
                        workflow_file: config.workflow_file.clone(),
                        last_completed_run: last_successful_run,
                        last_completed_cycle: last_cycle,
                        last_completed_phase_index: run_spec.phase_index,
                        last_completed_iteration: run_spec.phase_iteration,
                        total_runs_completed: total_runs,
                        cumulative_cost_usd: cumulative_cost,
                        claude_resume_commands: vec![],
                        canceled_at: None,
                        failure_reason: Some("timeout".to_string()),
                    };
                    let _ = state.write_atomic(&state_path);

                    // Wait up to 5s for graceful shutdown
                    let grace_deadline =
                        std::time::Instant::now() + std::time::Duration::from_secs(5);
                    let mut exited_gracefully = false;
                    loop {
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
        cumulative_cost += cost.cost_usd.unwrap_or(0.0);
        total_runs += 1;

        // Print per-run result
        crate::display::print_run_result(&run_spec, cost.cost_usd.unwrap_or(0.0), elapsed_secs);

        // Accumulate cycle cost
        cycle_cost += cost.cost_usd.unwrap_or(0.0);

        // Write run log
        write_run_log(&runs_dir, run_spec.global_run_number, &output.combined)?;

        // Handle timeout
        if timeout_occurred {
            // Print final cycle cost before returning
            if current_display_cycle > 0 {
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
                },
            )?;
            return Ok(EngineResult {
                exit_code: 2,
                completed_cycles: last_cycle,
                total_cost_usd: cumulative_cost,
                total_runs,
                parse_warnings,
            });
        }

        // Handle cancellation
        if cancel_occurred {
            // Print final cycle cost before returning
            if current_display_cycle > 0 {
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
                },
            )?;
            return Ok(EngineResult {
                exit_code: 130,
                completed_cycles: last_cycle,
                total_cost_usd: cumulative_cost,
                total_runs,
                parse_warnings,
            });
        }

        // Handle executor error — save state with PREVIOUS completed run so the
        // failing run will be retried on resume.
        let resume_commands = extract_resume_commands(&response_text);
        if output.exit_code != 0 {
            // Print final cycle cost before returning
            if current_display_cycle > 0 {
                crate::display::print_cycle_cost(cycle_cost);
            }
            // Write state BEFORE costs.jsonl to prevent duplicate cost entries on resume.
            let state = StateFile {
                schema_version: 1,
                run_id: config.run_id.clone(),
                workflow_file: config.workflow_file.clone(),
                last_completed_run: last_successful_run,
                last_completed_cycle: run_spec.cycle,
                last_completed_phase_index: run_spec.phase_index,
                last_completed_iteration: run_spec.phase_iteration,
                total_runs_completed: total_runs,
                cumulative_cost_usd: cumulative_cost,
                claude_resume_commands: resume_commands,
                canceled_at: None,
                failure_reason: None,
            };
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
                },
            )?;
            return Ok(EngineResult {
                exit_code: 3,
                completed_cycles: last_cycle,
                total_cost_usd: cumulative_cost,
                total_runs,
                parse_warnings,
            });
        }

        // Run succeeded — persist state first (C-2: state before costs prevents duplicate entries
        // on resume if interrupted between the two writes).
        let state = StateFile {
            schema_version: 1,
            run_id: config.run_id.clone(),
            workflow_file: config.workflow_file.clone(),
            last_completed_run: run_spec.global_run_number,
            last_completed_cycle: run_spec.cycle,
            last_completed_phase_index: run_spec.phase_index,
            last_completed_iteration: run_spec.phase_iteration,
            total_runs_completed: total_runs,
            cumulative_cost_usd: cumulative_cost,
            claude_resume_commands: resume_commands.clone(),
            canceled_at: None,
            failure_reason: None,
        };
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
            },
        )?;

        last_successful_run = run_spec.global_run_number;

        // Update phase costs and check budget caps
        phase_costs
            .entry(run_spec.phase_name.clone())
            .and_modify(|c| *c += cost.cost_usd.unwrap_or(0.0))
            .or_insert(cost.cost_usd.unwrap_or(0.0));

        // Check global budget cap
        if let Some(cap) = workflow.budget_cap_usd {
            let pct = (cumulative_cost / cap * 100.0) as u32;

            // ≥100%: budget cap reached
            if cumulative_cost >= cap {
                // Print final cycle cost before returning
                if current_display_cycle > 0 {
                    crate::display::print_cycle_cost(cycle_cost);
                }
                // Print budget cap reached message
                crate::display::print_budget_cap_reached(cap, cumulative_cost);

                // Emit budget_cap event
                let event = BudgetCapEvent {
                    event: "budget_cap".to_string(),
                    run_id: config.run_id.clone(),
                    cost_usd: cumulative_cost,
                    budget_cap_usd: cap,
                    scope: "global".to_string(),
                    runs_completed: total_runs,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = append_event(&events_path, &serde_json::to_value(&event)?);

                // Save state before returning
                let state = StateFile {
                    schema_version: 1,
                    run_id: config.run_id.clone(),
                    workflow_file: config.workflow_file.clone(),
                    last_completed_run: last_successful_run,
                    last_completed_cycle: last_cycle,
                    last_completed_phase_index: run_spec.phase_index,
                    last_completed_iteration: run_spec.phase_iteration,
                    total_runs_completed: total_runs,
                    cumulative_cost_usd: cumulative_cost,
                    claude_resume_commands: vec![],
                    canceled_at: None,
                    failure_reason: None,
                };
                state.write_atomic(&state_path)?;

                return Ok(EngineResult {
                    exit_code: 4,
                    completed_cycles: last_cycle,
                    total_cost_usd: cumulative_cost,
                    total_runs,
                    parse_warnings,
                });
            }

            // ≥80%: warning (once)
            if pct >= 80 && !budget_warned_80_global {
                budget_warned_80_global = true;
                eprintln!(
                    "⚠  Budget: ${:.2} spent — 80% of ${:.2} cap.",
                    cumulative_cost, cap
                );
                let event = BudgetWarningEvent {
                    event: "budget_warning".to_string(),
                    run_id: config.run_id.clone(),
                    cost_usd: cumulative_cost,
                    budget_cap_usd: cap,
                    pct: 80,
                    scope: "global".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = append_event(&events_path, &serde_json::to_value(&event)?);
            }

            // ≥90%: warning (once)
            if pct >= 90 && !budget_warned_90_global {
                budget_warned_90_global = true;
                eprintln!(
                    "⚠  Budget: ${:.2} spent — 90% of ${:.2} cap. Approaching limit.",
                    cumulative_cost, cap
                );
                let event = BudgetWarningEvent {
                    event: "budget_warning".to_string(),
                    run_id: config.run_id.clone(),
                    cost_usd: cumulative_cost,
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
                if let Some(&phase_cost) = phase_costs.get(&phase.name) {
                    let pct = (phase_cost / cap * 100.0) as u32;

                    // ≥100%: phase budget cap reached
                    if phase_cost >= cap {
                        // Print final cycle cost before returning
                        if current_display_cycle > 0 {
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
                            runs_completed: total_runs,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        let _ = append_event(&events_path, &serde_json::to_value(&event)?);

                        // Save state before returning
                        let state = StateFile {
                            schema_version: 1,
                            run_id: config.run_id.clone(),
                            workflow_file: config.workflow_file.clone(),
                            last_completed_run: last_successful_run,
                            last_completed_cycle: last_cycle,
                            last_completed_phase_index: run_spec.phase_index,
                            last_completed_iteration: run_spec.phase_iteration,
                            total_runs_completed: total_runs,
                            cumulative_cost_usd: cumulative_cost,
                            claude_resume_commands: vec![],
                            canceled_at: None,
                            failure_reason: None,
                        };
                        state.write_atomic(&state_path)?;

                        return Ok(EngineResult {
                            exit_code: 4,
                            completed_cycles: last_cycle,
                            total_cost_usd: cumulative_cost,
                            total_runs,
                            parse_warnings,
                        });
                    }

                    // ≥80%: warning (once per phase)
                    if pct >= 80
                        && !budget_warned_80_phase
                            .get(&phase.name)
                            .copied()
                            .unwrap_or(false)
                    {
                        budget_warned_80_phase.insert(phase.name.clone(), true);
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
                        && !budget_warned_90_phase
                            .get(&phase.name)
                            .copied()
                            .unwrap_or(false)
                    {
                        budget_warned_90_phase.insert(phase.name.clone(), true);
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
                if current_display_cycle > 0 {
                    crate::display::print_cycle_cost(cycle_cost);
                }
                // Save state with canceled_at timestamp before returning
                let state = StateFile {
                    schema_version: 1,
                    run_id: config.run_id.clone(),
                    workflow_file: config.workflow_file.clone(),
                    last_completed_run: last_successful_run,
                    last_completed_cycle: last_cycle,
                    last_completed_phase_index: run_spec.phase_index,
                    last_completed_iteration: run_spec.phase_iteration,
                    total_runs_completed: total_runs,
                    cumulative_cost_usd: cumulative_cost,
                    claude_resume_commands: resume_commands.clone(),
                    canceled_at: Some(chrono::Utc::now().to_rfc3339()),
                    failure_reason: None,
                };
                state.write_atomic(&state_path)?;
                return Ok(EngineResult {
                    exit_code: 130,
                    completed_cycles: last_cycle,
                    total_cost_usd: cumulative_cost,
                    total_runs,
                    parse_warnings,
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
            if current_display_cycle > 0 {
                crate::display::print_cycle_cost(cycle_cost);
            }
            return Ok(EngineResult {
                exit_code: 0,
                completed_cycles: last_cycle,
                total_cost_usd: cumulative_cost,
                total_runs,
                parse_warnings,
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
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_secs(workflow.delay_between_runs);
            while std::time::Instant::now() < deadline {
                if let Some(ref cs) = cancel {
                    if cs.is_canceling() {
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    // Print final cycle cost before returning
    if current_display_cycle > 0 {
        crate::display::print_cycle_cost(cycle_cost);
    }

    Ok(EngineResult {
        exit_code: 1,
        completed_cycles: last_cycle,
        total_cost_usd: cumulative_cost,
        total_runs,
        parse_warnings,
    })
}
