#[cfg(not(unix))]
compile_error!("rings requires a Unix platform");

pub mod audit;
pub mod cancel;
pub mod cli;
pub mod completion;
pub mod cost;
pub mod display;
pub mod duration;
pub mod engine;
pub mod executor;
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
        status: "running".to_string(),
    };
    meta.write(&run_dir.join("run.toml"))?;

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
        0 => "completed",
        1 => "incomplete", // max_cycles reached without completion signal
        130 => "canceled",
        _ => "failed",
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
    let meta_path = run_dir.join("run.toml");

    let saved_state = state::StateFile::read(&state_path)
        .with_context(|| format!("Cannot read state for run {}", args.run_id))?;
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

    eprintln!("Resuming {}", args.run_id);
    eprintln!("Workflow:  {}", meta.workflow_file);
    eprintln!("Previous cost: ${:.3}", saved_state.cumulative_cost_usd);
    eprintln!();

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
        0 => "completed",
        1 => "incomplete",
        130 => "canceled",
        _ => "failed",
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

    Ok(result.exit_code)
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
