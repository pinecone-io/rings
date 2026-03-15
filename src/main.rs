pub mod audit;
pub mod cli;
pub mod completion;
pub mod cost;
pub mod display;
pub mod engine;
pub mod executor;
pub mod state;
pub mod template;
pub mod workflow;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cli::{Cli, Command};
use engine::{run_workflow, EngineConfig};
use executor::ClaudeExecutor;

fn main() {
    let cli = Cli::parse();
    let exit_code = match cli.command {
        Command::Run(args) => cmd_run(args),
        Command::Resume(args) => cmd_resume(args),
    };
    std::process::exit(exit_code);
}

fn cmd_run(args: cli::RunArgs) -> i32 {
    match run_inner(args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    }
}

fn run_inner(args: cli::RunArgs) -> Result<i32> {
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

    // Check executor is available
    which::which("claude")
        .context("'claude' not found on PATH. rings requires Claude Code to be installed.")?;

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
        let prompt_texts: Vec<String> = workflow
            .phases
            .iter()
            .filter_map(|p| p.prompt_text.clone())
            .collect();
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

    let executor = ClaudeExecutor;
    let config = EngineConfig {
        output_dir: run_dir.clone(),
        verbose: args.verbose,
    };

    // Install Ctrl+C handler (simple: set a flag; full graceful shutdown is future work)
    let canceled = Arc::new(AtomicBool::new(false));
    {
        let canceled = canceled.clone();
        ctrlc::set_handler(move || {
            canceled.store(true, Ordering::SeqCst);
        })
        .context("Failed to install Ctrl+C handler")?;
    }

    let result = run_workflow(&workflow, &executor, &config, None)?;

    let final_status = match result.exit_code {
        0 => "completed",
        1 => "incomplete", // max_cycles reached without completion signal
        _ => "failed",
    };
    meta.update_status(&run_dir.join("run.toml"), final_status)?;

    Ok(result.exit_code)
}

fn cmd_resume(args: cli::ResumeArgs) -> i32 {
    match resume_inner(args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    }
}

fn resume_inner(args: cli::ResumeArgs) -> Result<i32> {
    // Find run directory
    let output_base = resolve_output_dir(args.output_dir.as_deref(), None);
    let run_dir = output_base.join(&args.run_id);
    let state_path = run_dir.join("state.json");
    let meta_path = run_dir.join("run.toml");

    let saved_state = state::StateFile::read(&state_path)
        .with_context(|| format!("Cannot read state for run {}", args.run_id))?;
    let meta = state::RunMeta::read(&meta_path)
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

    eprintln!("Resuming {}", args.run_id);
    eprintln!("Workflow:  {}", meta.workflow_file);
    eprintln!("Previous cost: ${:.3}", saved_state.cumulative_cost_usd);
    eprintln!();

    // Build a resume-aware engine config (reuse same run_dir)
    let executor = ClaudeExecutor;
    let config = EngineConfig {
        output_dir: run_dir.clone(),
        verbose: args.verbose,
    };

    // Use RunSchedule::resume_from to skip already-completed runs
    let result = run_workflow(
        &workflow,
        &executor,
        &config,
        Some(saved_state.last_completed_run),
    )?;

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
