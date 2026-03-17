#[cfg(not(unix))]
compile_error!("rings requires a Unix platform");

pub mod audit;
pub mod cancel;
pub mod cli;
pub mod completion;
pub mod cost;
pub mod display;
pub mod dry_run;
pub mod duration;
pub mod engine;
pub mod executor;
pub mod list;
#[cfg(unix)]
pub mod lock;
pub mod state;
pub mod template;
pub mod workflow;

use anyhow::{bail, Context, Result};
use cancel::CancelState;
use clap::Parser;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use cli::{Cli, Command};
use engine::{run_workflow, EngineConfig, ResumePoint};
use executor::{ClaudeExecutor, ConfigurableExecutor};
#[cfg(unix)]
use lock::ContextLock;

fn main() {
    // Ignore SIGPIPE so that broken pipe errors (e.g., when piping rings output
    // through `head`) do not cause unexpected crashes.
    #[cfg(unix)]
    {
        use nix::sys::signal::{signal, SigHandler, Signal};
        // SAFETY: SIG_IGN is a valid signal handler with no state.
        unsafe {
            let _ = signal(Signal::SIGPIPE, SigHandler::SigIgn);
        }
    }

    let cancel = Arc::new(CancelState::new());
    {
        let cancel_clone = Arc::clone(&cancel);
        ctrlc::set_handler(move || {
            cancel_clone.signal_received();
        })
        .expect("Failed to install Ctrl+C handler");
    }

    let cli = Cli::parse();
    let exit_code = match cli.command {
        Command::Run(args) => cmd_run(args, Arc::clone(&cancel)),
        Command::Resume(args) => cmd_resume(args, Arc::clone(&cancel)),
        Command::List(args) => cmd_list(args, cli.output_format),
    };
    std::process::exit(exit_code);
}

fn cmd_run(args: cli::RunArgs, cancel: Arc<CancelState>) -> i32 {
    match run_inner(args, cancel) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    }
}

fn run_inner(args: cli::RunArgs, cancel: Arc<CancelState>) -> Result<i32> {
    // Load and validate workflow
    let toml_content = std::fs::read_to_string(&args.workflow_file)
        .with_context(|| format!("Cannot read workflow file: {}", args.workflow_file))?;
    let mut workflow = workflow::Workflow::from_str(&toml_content)
        .with_context(|| format!("Invalid workflow file: {}", args.workflow_file))?;

    // Apply CLI overrides
    if let Some(max_cycles) = args.max_cycles {
        workflow.max_cycles = max_cycles;
    }
    if let Some(delay) = args.delay {
        workflow.delay_between_runs = delay;
    }
    if let Some(cap) = args.budget_cap {
        if cap <= 0.0 {
            bail!("--budget-cap must be greater than zero");
        }
        workflow.budget_cap_usd = Some(cap);
    }
    if let Some(ref timeout_str) = args.timeout_per_run {
        let secs = duration::parse_duration_secs(timeout_str)
            .with_context(|| format!("invalid --timeout-per-run value: {timeout_str:?}"))?;
        workflow.timeout_per_run_secs = Some(secs);
    }

    // Handle dry-run mode
    if args.dry_run {
        let plan = dry_run::DryRunPlan::from_workflow(&workflow, &args.workflow_file)?;

        // Output in human format (table with ✓/✗)
        println!("Dry run: {}", args.workflow_file);
        println!("  completion_signal: {:?}", workflow.completion_signal);
        println!("  context_dir:       {}", workflow.context_dir);
        println!("  max_cycles:        {}", workflow.max_cycles);
        println!();
        println!("  Cycle structure (repeating):");
        for phase in &plan.phases {
            println!(
                "    Phase {}: {} ×{} (prompt: {})",
                plan.phases
                    .iter()
                    .position(|p| p.name == phase.name)
                    .unwrap()
                    + 1,
                phase.name,
                phase.runs_per_cycle,
                phase.prompt_source
            );
        }
        println!();
        println!("  Total runs per cycle: {}", plan.runs_per_cycle_total);
        if let Some(max_total) = plan.max_total_runs {
            println!("  Maximum total runs:   {}", max_total);
        }
        println!();
        println!("  Prompt check:");
        for phase in &plan.phases {
            if phase.signal_check.found {
                if let Some(line_num) = phase.signal_check.line_number {
                    println!(
                        "    ✓ \"{}\" found in {} (line {})",
                        workflow.completion_signal, phase.prompt_source, line_num
                    );
                } else {
                    println!(
                        "    ✓ \"{}\" found in {}",
                        workflow.completion_signal, phase.prompt_source
                    );
                }
            } else {
                println!(
                    "    ✗ \"{}\" not found in {}",
                    workflow.completion_signal, phase.prompt_source
                );
            }
        }

        return Ok(0);
    }

    // Check executor is available
    let executor_binary = workflow
        .executor
        .as_ref()
        .map(|e| e.binary.as_str())
        .unwrap_or("claude");
    which::which(executor_binary).with_context(|| {
        format!(
            "'{executor_binary}' not found on PATH. rings requires Claude Code to be installed."
        )
    })?;

    // Resolve output directory
    let output_base =
        resolve_output_dir(args.output_dir.as_deref(), workflow.output_dir.as_deref());
    let run_id = generate_run_id();
    let run_dir = output_base.join(&run_id);
    std::fs::create_dir_all(&run_dir)
        .with_context(|| format!("Cannot create output directory: {}", run_dir.display()))?;

    // Write run.toml
    let mut meta = state::RunMeta {
        run_id: run_id.clone(),
        workflow_file: std::fs::canonicalize(&args.workflow_file)
            .unwrap_or_else(|_| PathBuf::from(&args.workflow_file))
            .to_string_lossy()
            .to_string(),
        started_at: chrono::Utc::now().to_rfc3339(),
        rings_version: env!("CARGO_PKG_VERSION").to_string(),
        status: state::RunStatus::Running,
        phase_fingerprint: Some(workflow.structural_fingerprint()),
    };
    meta.write(&run_dir.join("run.toml"))?;

    // Advisory check: no budget cap configured
    if workflow.budget_cap_usd.is_none() && args.budget_cap.is_none() {
        eprintln!(
            "⚠  Warning: No budget cap configured. \
             Use --budget-cap or budget_cap_usd to prevent unbounded spend."
        );
    }

    // Advisory check: completion signal in prompts
    if !args.no_completion_check {
        let mut prompt_texts: Vec<String> = Vec::new();
        for phase in &workflow.phases {
            if let Some(text) = &phase.prompt_text {
                prompt_texts.push(text.clone());
            } else if let Some(file) = &phase.prompt {
                // Best-effort: skip file read failures (advisory only)
                if let Ok(content) = std::fs::read_to_string(file) {
                    prompt_texts.push(content);
                }
            }
        }
        let texts: Vec<&str> = prompt_texts.iter().map(String::as_str).collect();
        if !completion::any_prompt_contains_signal(&texts, &workflow.completion_signal) {
            eprintln!(
                "⚠  completion_signal '{}' not found in any prompt. \
                 Use --no-completion-check to suppress this warning.",
                workflow.completion_signal
            );
        }
    }

    display::print_run_header(&run_id, &args.workflow_file);

    // Acquire context directory lock
    #[cfg(unix)]
    let _lock = {
        let context_dir = PathBuf::from(&workflow.context_dir);
        match ContextLock::acquire(&context_dir, &run_id, args.force_lock) {
            Ok(result) => {
                if let Some(stale_info) = &result.stale_removed {
                    eprintln!(
                        "Warning: Removed stale lock file from previous run {} (PID={} no longer running).",
                        stale_info.run_id, stale_info.pid
                    );
                }
                result.lock
            }
            Err(e) => {
                eprintln!("{}", e);
                return Ok(2);
            }
        }
    };

    let config = EngineConfig {
        output_dir: run_dir.clone(),
        verbose: args.verbose,
        run_id: run_id.clone(),
        workflow_file: std::fs::canonicalize(&args.workflow_file)
            .unwrap_or_else(|_| PathBuf::from(&args.workflow_file))
            .to_string_lossy()
            .to_string(),
    };

    let run_start = std::time::Instant::now();
    let result = if let Some(ref exec_cfg) = workflow.executor {
        let executor = ConfigurableExecutor {
            binary: exec_cfg.binary.clone(),
            args: exec_cfg.args.clone(),
        };
        run_workflow(
            &workflow,
            &executor,
            &config,
            None,
            Some(Arc::clone(&cancel)),
        )?
    } else {
        let executor = ClaudeExecutor;
        run_workflow(
            &workflow,
            &executor,
            &config,
            None,
            Some(Arc::clone(&cancel)),
        )?
    };
    let total_elapsed_secs = run_start.elapsed().as_secs();

    let final_status = match result.exit_code {
        0 => state::RunStatus::Completed,
        1 => state::RunStatus::Incomplete, // max_cycles reached without completion signal
        4 => state::RunStatus::Stopped,    // budget cap reached
        130 => state::RunStatus::Canceled,
        _ => state::RunStatus::Failed,
    };
    meta.update_status(&run_dir.join("run.toml"), final_status)?;

    // Print completion, error, or max-cycles summary based on exit code
    match result.exit_code {
        0 => {
            // Completion: read state.json to get last cycle/phase/run
            let state_path = run_dir.join("state.json");
            if let Ok(state) = state::StateFile::read(&state_path) {
                let phase = workflow.phases.get(state.last_completed_phase_index);
                let phase_name = phase.map(|p| p.name.as_str()).unwrap_or("unknown");
                display::print_completion(
                    state.last_completed_cycle,
                    state.last_completed_run,
                    phase_name,
                    result.total_cost_usd,
                    result.total_runs,
                    total_elapsed_secs,
                    &run_dir.to_string_lossy(),
                );
            }
        }
        1 => {
            // Max cycles reached
            display::print_max_cycles(
                workflow.max_cycles,
                result.total_cost_usd,
                result.total_runs,
                &run_id,
            );
        }
        3 => {
            // Executor error: read state.json to determine which run failed and get log path
            let state_path = run_dir.join("state.json");
            if let Ok(state) = state::StateFile::read(&state_path) {
                let failed_run_number = state.last_completed_run + 1;
                let log_path = run_dir
                    .join("runs")
                    .join(format!("{:03}.log", failed_run_number));
                display::print_executor_error(
                    failed_run_number,
                    3,
                    &run_id,
                    &log_path.to_string_lossy(),
                );
            }
        }
        4 => {
            // Budget cap reached: read state.json to get budget cap value from workflow
            if let Some(cap) = workflow.budget_cap_usd {
                display::print_budget_cap_reached(cap, result.total_cost_usd);
            }
        }
        130 => {
            // Cancellation: load the state that was saved during cancellation to get last run position
            let state_path = run_dir.join("state.json");
            if let Ok(state) = state::StateFile::read(&state_path) {
                let phase = workflow.phases.get(state.last_completed_phase_index);
                let phase_name = phase.map(|p| p.name.as_str()).unwrap_or("unknown");
                display::print_cancellation(
                    &run_id,
                    state.last_completed_cycle,
                    phase_name,
                    state.cumulative_cost_usd,
                    &state.claude_resume_commands,
                );
            }
        }
        _ => {}
    }

    // Print low-confidence cost parse warnings
    display::print_parse_warnings(&result.parse_warnings);

    Ok(result.exit_code)
}

fn cmd_resume(args: cli::ResumeArgs, cancel: Arc<CancelState>) -> i32 {
    match resume_inner(args, cancel) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    }
}

fn resume_inner(args: cli::ResumeArgs, cancel: Arc<CancelState>) -> Result<i32> {
    // Find run directory
    let output_base = resolve_output_dir(args.output_dir.as_deref(), None);
    let run_dir = output_base.join(&args.run_id);
    let state_path = run_dir.join("state.json");
    let costs_path = run_dir.join("costs.jsonl");
    let meta_path = run_dir.join("run.toml");

    // Load state with fallback recovery from costs.jsonl
    let saved_state = match state::StateFile::load_or_recover(&state_path, &costs_path) {
        state::StateLoadResult::Ok(state) => state,
        state::StateLoadResult::Recovered { state, warning } => {
            eprintln!("{}", warning);
            state
        }
        state::StateLoadResult::Unrecoverable {
            state_path: sp,
            costs_path: cp,
        } => {
            let state_path_canonical =
                std::fs::canonicalize(&sp).unwrap_or_else(|_| sp.to_path_buf());
            let costs_path_canonical =
                std::fs::canonicalize(&cp).unwrap_or_else(|_| cp.to_path_buf());
            eprintln!(
                "Cannot resume: state.json is corrupt and costs.jsonl could not reconstruct the run position.\n  state.json: {}\n  costs.jsonl: {}\nPlease inspect these files manually.",
                state_path_canonical.display(),
                costs_path_canonical.display()
            );
            return Ok(2);
        }
    };

    let mut meta = state::RunMeta::read(&meta_path)
        .with_context(|| format!("Cannot read run.toml for run {}", args.run_id))?;

    // Reload workflow
    let toml_content = std::fs::read_to_string(&meta.workflow_file)
        .with_context(|| format!("Cannot read workflow file: {}", meta.workflow_file))?;
    let mut workflow = workflow::Workflow::from_str(&toml_content)?;

    if let Some(max_cycles) = args.max_cycles {
        workflow.max_cycles = max_cycles;
    }
    if let Some(delay) = args.delay {
        workflow.delay_between_runs = delay;
    }
    if let Some(cap) = args.budget_cap {
        if cap <= 0.0 {
            bail!("--budget-cap must be greater than zero");
        }
        workflow.budget_cap_usd = Some(cap);
    }
    if let Some(ref timeout_str) = args.timeout_per_run {
        let secs = duration::parse_duration_secs(timeout_str)
            .with_context(|| format!("invalid --timeout-per-run value: {timeout_str:?}"))?;
        workflow.timeout_per_run_secs = Some(secs);
    }

    // Check workflow structural changes
    let current_fingerprint = workflow.structural_fingerprint();
    match &meta.phase_fingerprint {
        None => {
            // Old run.toml without fingerprint; skip check with advisory warning
            eprintln!(
                "⚠  Warning: run.toml has no phase fingerprint (created with older rings version). \
                 Skipping structural change detection."
            );
        }
        Some(saved_fingerprint) => {
            if saved_fingerprint != &current_fingerprint {
                // Detect type of structural change
                if current_fingerprint.len() > saved_fingerprint.len() {
                    eprintln!("Cannot resume: workflow has phases not present in the saved run.");
                    return Ok(2);
                } else if current_fingerprint.len() < saved_fingerprint.len() {
                    eprintln!(
                        "Cannot resume: saved run has phases removed from the current workflow."
                    );
                    return Ok(2);
                } else if current_fingerprint != *saved_fingerprint {
                    // Same length but different: must be reordered
                    eprintln!("Cannot resume: phase order has changed since this run was created.");
                    return Ok(2);
                }
            }
        }
    }

    // Check for non-structural changes (runs_per_cycle change may require clamping)
    // If runs_per_cycle of the last_completed_phase_index changed, clamp last_completed_iteration
    let mut saved_state = saved_state;
    if let Some(saved_fingerprint) = &meta.phase_fingerprint {
        if saved_fingerprint == &current_fingerprint {
            // Fingerprints match (no structural changes)
            // Check if runs_per_cycle changed for the last_completed_phase_index
            if (saved_state.last_completed_phase_index as usize) < workflow.phases.len() {
                let current_runs_per_cycle =
                    workflow.phases[saved_state.last_completed_phase_index].runs_per_cycle;
                // If current runs_per_cycle is smaller than last_completed_iteration, clamp
                // and emit warning
                if saved_state.last_completed_iteration > current_runs_per_cycle {
                    saved_state.last_completed_iteration = current_runs_per_cycle;
                    eprintln!(
                        "Workflow file has changed since this run was created. \
                         Non-structural changes will take effect from the resume point."
                    );
                }
            }
        }
    }

    // Advisory check: no budget cap configured
    if workflow.budget_cap_usd.is_none() && args.budget_cap.is_none() {
        eprintln!(
            "⚠  Warning: No budget cap configured. \
             Use --budget-cap or budget_cap_usd to prevent unbounded spend."
        );
    }

    eprintln!("Resuming {}", args.run_id);
    eprintln!("Workflow:  {}", meta.workflow_file);
    eprintln!("Previous cost: ${:.3}", saved_state.cumulative_cost_usd);
    eprintln!();

    // Acquire context directory lock
    #[cfg(unix)]
    let _lock = {
        let context_dir = PathBuf::from(&workflow.context_dir);
        match ContextLock::acquire(&context_dir, &args.run_id, args.force_lock) {
            Ok(result) => {
                if let Some(stale_info) = &result.stale_removed {
                    eprintln!(
                        "Warning: Removed stale lock file from previous run {} (PID={} no longer running).",
                        stale_info.run_id, stale_info.pid
                    );
                }
                result.lock
            }
            Err(e) => {
                eprintln!("{}", e);
                return Ok(2);
            }
        }
    };

    // Build a resume-aware engine config (reuse same run_dir)
    let config = EngineConfig {
        output_dir: run_dir.clone(),
        verbose: args.verbose,
        run_id: args.run_id.clone(),
        workflow_file: meta.workflow_file.clone(),
    };

    let resume_point = Some(ResumePoint {
        last_completed_run: saved_state.last_completed_run,
        last_completed_cycle: saved_state.last_completed_cycle,
        last_completed_phase_index: saved_state.last_completed_phase_index,
        last_completed_iteration: saved_state.last_completed_iteration,
    });

    // Use position-based resume so continue_signal skips are handled correctly.
    let run_start = std::time::Instant::now();
    let result = if let Some(ref exec_cfg) = workflow.executor {
        let executor = ConfigurableExecutor {
            binary: exec_cfg.binary.clone(),
            args: exec_cfg.args.clone(),
        };
        run_workflow(
            &workflow,
            &executor,
            &config,
            resume_point,
            Some(Arc::clone(&cancel)),
        )?
    } else {
        let executor = ClaudeExecutor;
        run_workflow(
            &workflow,
            &executor,
            &config,
            resume_point,
            Some(Arc::clone(&cancel)),
        )?
    };
    let total_elapsed_secs = run_start.elapsed().as_secs();

    // Update run.toml status based on exit code
    let final_status = match result.exit_code {
        0 => state::RunStatus::Completed,
        1 => state::RunStatus::Incomplete,
        4 => state::RunStatus::Stopped, // budget cap reached
        130 => state::RunStatus::Canceled,
        _ => state::RunStatus::Failed,
    };
    meta.update_status(&meta_path, final_status)?;

    // Print completion, error, or max-cycles summary based on exit code
    match result.exit_code {
        0 => {
            // Completion: read state.json to get last cycle/phase/run
            let state_path = run_dir.join("state.json");
            if let Ok(state) = state::StateFile::read(&state_path) {
                let phase = workflow.phases.get(state.last_completed_phase_index);
                let phase_name = phase.map(|p| p.name.as_str()).unwrap_or("unknown");
                display::print_completion(
                    state.last_completed_cycle,
                    state.last_completed_run,
                    phase_name,
                    result.total_cost_usd,
                    result.total_runs,
                    total_elapsed_secs,
                    &run_dir.to_string_lossy(),
                );
            }
        }
        1 => {
            // Max cycles reached
            display::print_max_cycles(
                workflow.max_cycles,
                result.total_cost_usd,
                result.total_runs,
                &args.run_id,
            );
        }
        3 => {
            // Executor error: read state.json to determine which run failed and get log path
            let state_path = run_dir.join("state.json");
            if let Ok(state) = state::StateFile::read(&state_path) {
                let failed_run_number = state.last_completed_run + 1;
                let log_path = run_dir
                    .join("runs")
                    .join(format!("{:03}.log", failed_run_number));
                display::print_executor_error(
                    failed_run_number,
                    3,
                    &args.run_id,
                    &log_path.to_string_lossy(),
                );
            }
        }
        4 => {
            // Budget cap reached: read state.json to get budget cap value from workflow
            if let Some(cap) = workflow.budget_cap_usd {
                display::print_budget_cap_reached(cap, result.total_cost_usd);
            }
        }
        130 => {
            // Cancellation: load the state that was saved during cancellation to get last run position
            let state_path = run_dir.join("state.json");
            if let Ok(state) = state::StateFile::read(&state_path) {
                let phase = workflow.phases.get(state.last_completed_phase_index);
                let phase_name = phase.map(|p| p.name.as_str()).unwrap_or("unknown");
                display::print_cancellation(
                    &args.run_id,
                    state.last_completed_cycle,
                    phase_name,
                    state.cumulative_cost_usd,
                    &state.claude_resume_commands,
                );
            }
        }
        _ => {}
    }

    // Print low-confidence cost parse warnings
    display::print_parse_warnings(&result.parse_warnings);

    Ok(result.exit_code)
}

fn cmd_list(args: cli::ListArgs, output_format: cli::OutputFormat) -> i32 {
    match list_inner(args, output_format) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    }
}

fn list_inner(args: cli::ListArgs, output_format: cli::OutputFormat) -> Result<i32> {
    // Resolve base directory for scanning runs
    let base_dir = resolve_output_dir(None, None);

    // Parse since filter
    let since_filter = if let Some(since_str) = args.since {
        Some(since_str.parse::<duration::SinceSpec>()?)
    } else {
        None
    };

    // Parse status filter
    let status_filter = if let Some(status_str) = args.status {
        Some(status_str.parse::<state::RunStatus>()?)
    } else {
        None
    };

    let filters = list::ListFilters {
        since: since_filter,
        status: status_filter,
        workflow: args.workflow,
        limit: args.limit,
    };

    let runs = list::list_runs(&filters, &base_dir)?;

    // Output results
    match output_format {
        cli::OutputFormat::Human => {
            // Print human-readable table
            if runs.is_empty() {
                eprintln!("No runs found.");
            } else {
                eprintln!(
                    "{:<20} {:<20} {:<40} {:<12} {:<8} {:<10}",
                    "RUN ID", "DATE", "WORKFLOW", "STATUS", "CYCLES", "COST"
                );
                eprintln!("{}", "-".repeat(110));
                for run in &runs {
                    let date_str = run.started_at.format("%Y-%m-%d %H:%M:%S").to_string();
                    let cost_str = run
                        .total_cost_usd
                        .map(|c| format!("${:.3}", c))
                        .unwrap_or_else(|| "—".to_string());
                    let status_display = if run.status == state::RunStatus::Running {
                        // Check if it looks stale (started > 24h ago with no lock file)
                        let now = chrono::Utc::now();
                        let hours_ago = (now - run.started_at).num_hours();
                        if hours_ago > 24 {
                            "Running (stale?)".to_string()
                        } else {
                            run.status.to_string()
                        }
                    } else {
                        run.status.to_string()
                    };
                    eprintln!(
                        "{:<20} {:<20} {:<40} {:<12} {:<8} {:<10}",
                        run.run_id,
                        date_str,
                        run.workflow,
                        status_display,
                        run.cycles_completed,
                        cost_str
                    );
                }
            }
        }
        cli::OutputFormat::Jsonl => {
            // Print JSONL output
            for run in &runs {
                let json = serde_json::json!({
                    "run_id": run.run_id,
                    "started_at": run.started_at.to_rfc3339(),
                    "workflow": run.workflow,
                    "status": run.status.to_string(),
                    "cycles_completed": run.cycles_completed,
                    "total_cost_usd": run.total_cost_usd,
                });
                println!("{}", json);
            }
        }
    }

    Ok(0)
}

fn resolve_output_dir(cli_override: Option<&str>, workflow_override: Option<&str>) -> PathBuf {
    if let Some(p) = cli_override {
        return PathBuf::from(p);
    }
    if let Some(p) = workflow_override {
        return PathBuf::from(p);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rings")
        .join("runs")
}

fn generate_run_id() -> String {
    let now = chrono::Utc::now();
    let ts = now.format("%Y%m%d_%H%M%S");
    let short_uuid = uuid::Uuid::new_v4().to_string()[..6].to_string();
    format!("run_{ts}_{short_uuid}")
}
