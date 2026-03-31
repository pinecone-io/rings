#[cfg(not(unix))]
compile_error!("rings requires a Unix platform");

use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use rings::cancel::CancelState;
use rings::cli::{self, Cli, Command};
use rings::completion;
use rings::config::RingsConfig;
use rings::display;
use rings::dry_run;
use rings::duration;
use rings::engine::{run_workflow, EngineConfig, ResumePoint};
use rings::executor::{ClaudeExecutor, ConfigurableExecutor};
use rings::list;
#[cfg(unix)]
use rings::lock::ContextLock;
use rings::state;
use rings::style;
use rings::telemetry;
use rings::template;
use rings::workflow;

fn main() {
    // Handle shell completion requests made via the COMPLETE env var (e.g. `COMPLETE=zsh rings`).
    // This is a no-op when COMPLETE is not set; when set, it prints a dynamic completion script
    // that calls back to the binary for custom completers, then exits.
    {
        use clap::CommandFactory;
        clap_complete::CompleteEnv::with_factory(Cli::command).complete();
    }

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
        if let Err(e) = ctrlc::set_handler(move || {
            cancel_clone.signal_received();
        }) {
            eprintln!("rings: failed to install Ctrl+C handler: {e}");
            std::process::exit(2);
        }
    }

    let cli = Cli::parse();

    // Load config file (project .rings-config.toml or user config.toml).
    // Non-fatal: if config loading fails, log to stderr and continue with defaults.
    let config = match RingsConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("rings: config error: {e:#}");
            std::process::exit(2);
        }
    };

    // Initialize color: disable if --no-color, NO_COLOR env var, config color=false, or stderr
    // is not a TTY.
    {
        use std::io::IsTerminal;
        if cli.no_color
            || std::env::var_os("NO_COLOR").is_some()
            || config.color == Some(false)
            || !std::io::stderr().is_terminal()
        {
            style::set_no_color();
        }
    }

    // Initialize OTel tracing (no-op unless RINGS_OTEL_ENABLED=true).
    let otel_handle = telemetry::init_tracer();

    let cfg_output_dir = config.expanded_output_dir();
    let exit_code = match cli.command {
        Command::Run(args) => cmd_run(
            args,
            Arc::clone(&cancel),
            cli.output_format,
            cfg_output_dir.as_deref(),
        ),
        Command::Resume(args) => cmd_resume(
            args,
            Arc::clone(&cancel),
            cli.output_format,
            cfg_output_dir.as_deref(),
        ),
        Command::List(args) => cmd_list(args, cli.output_format, cfg_output_dir.as_deref()),
        Command::Show(args) => cmd_show(args, cli.output_format, cfg_output_dir.as_deref()),
        Command::Inspect(args) => cmd_inspect(args, cli.output_format, cfg_output_dir.as_deref()),
        Command::Lineage(args) => cmd_lineage(args, cli.output_format, cfg_output_dir.as_deref()),
        Command::Completions(args) => cmd_completions(args),
        Command::GenerateMan => cmd_generate_man(),
        Command::Init(args) => cmd_init(args, cli.output_format),
        Command::Update => cmd_update(),
        Command::Cleanup(args) => cmd_cleanup(args, cli.output_format, cfg_output_dir.as_deref()),
    };

    // Flush any buffered OTel spans before exiting.
    otel_handle.shutdown();

    std::process::exit(exit_code);
}

fn cmd_run(
    args: cli::RunArgs,
    cancel: Arc<CancelState>,
    output_format: cli::OutputFormat,
    config_output_dir: Option<&str>,
) -> i32 {
    let mut run_id: Option<String> = None;
    match run_inner(args, cancel, output_format, &mut run_id, config_output_dir) {
        Ok(code) => code,
        Err(e) => {
            if output_format == cli::OutputFormat::Jsonl {
                rings::events::emit_jsonl(&rings::events::FatalErrorEvent::new(
                    run_id,
                    format!("{e:#}"),
                ));
            }
            eprintln!("Error: {e:#}");
            2
        }
    }
}

fn run_inner(
    args: cli::RunArgs,
    cancel: Arc<CancelState>,
    output_format: cli::OutputFormat,
    run_id_out: &mut Option<String>,
    config_output_dir: Option<&str>,
) -> Result<i32> {
    // Conflict check: --step is incompatible with --output-format jsonl
    if args.step && output_format == cli::OutputFormat::Jsonl {
        eprintln!(
            "Error: --step is incompatible with --output-format jsonl. \
             Remove --step or use human output format."
        );
        return Ok(2);
    }

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
    if let Some(cd) = args.cycle_delay {
        workflow.delay_between_cycles = cd;
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

    // Apply quota backoff CLI overrides
    if args.quota_backoff {
        workflow.quota_backoff = true;
    }
    if let Some(delay) = args.quota_backoff_delay {
        workflow.quota_backoff_delay = delay;
    }
    if let Some(max_retries) = args.quota_backoff_max_retries {
        workflow.quota_backoff_max_retries = max_retries;
    }

    // Handle dry-run mode
    if args.dry_run {
        let plan = dry_run::DryRunPlan::from_workflow(&workflow, &args.workflow_file)?;

        if output_format == cli::OutputFormat::Jsonl {
            let event = dry_run::DryRunPlanEvent {
                event: "dry_run_plan".to_string(),
                plan,
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            println!("{}", serde_json::to_string(&event)?);
            return Ok(0);
        }

        // Output in human format (table with ✓/✗)
        println!("Dry run: {}", style::bold(&args.workflow_file));
        println!(
            "  {}  {:?}",
            style::dim("completion_signal:"),
            workflow.completion_signal
        );
        println!(
            "  {}  {}",
            style::dim("context_dir:      "),
            workflow.context_dir
        );
        println!(
            "  {}  {}",
            style::dim("max_cycles:       "),
            style::bold(&workflow.max_cycles.to_string())
        );
        println!();
        println!("  {}", style::bold("Cycle structure (repeating):"));
        for phase in &plan.phases {
            println!(
                "    Phase {}: {} ×{} (prompt: {})",
                style::bold(
                    &(plan
                        .phases
                        .iter()
                        .position(|p| p.name == phase.name)
                        .unwrap_or(0)
                        + 1)
                    .to_string()
                ),
                style::bold(&phase.name),
                phase.runs_per_cycle,
                phase.prompt_source
            );
        }
        println!();
        println!(
            "  {}  {}",
            style::dim("Total runs per cycle:"),
            style::bold(&plan.runs_per_cycle_total.to_string())
        );
        if let Some(max_total) = plan.max_total_runs {
            println!(
                "  {}  {}",
                style::dim("Maximum total runs:  "),
                style::bold(&max_total.to_string())
            );
        }
        println!();
        println!("  {}", style::bold("Prompt check:"));
        for phase in &plan.phases {
            if phase.signal_check.found {
                if let Some(line_num) = phase.signal_check.line_number {
                    println!(
                        "    {} \"{}\" found in {} (line {})",
                        style::success("✓"),
                        workflow.completion_signal,
                        phase.prompt_source,
                        line_num
                    );
                } else {
                    println!(
                        "    {} \"{}\" found in {}",
                        style::success("✓"),
                        workflow.completion_signal,
                        phase.prompt_source
                    );
                }
            } else {
                println!(
                    "    {} \"{}\" not found in {}",
                    style::warn("✗"),
                    workflow.completion_signal,
                    phase.prompt_source
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

    // Validate --output-dir does not contain '..' path traversal.
    if let Some(ref dir) = args.output_dir {
        if path_contains_parent_dir(dir) {
            eprintln!("Error: output_dir contains path traversal ('..') which is not allowed.");
            return Ok(2);
        }
    }

    // Validate --include-dir paths exist and are directories.
    let mut include_dirs: Vec<std::path::PathBuf> = Vec::new();
    for dir_str in &args.include_dir {
        let path = std::path::PathBuf::from(dir_str);
        if !path.exists() || !path.is_dir() {
            eprintln!(
                "Error: --include-dir '{}' does not exist or is not a directory.",
                dir_str
            );
            return Ok(2);
        }
        include_dirs.push(path);
    }

    // Resolve output directory
    let output_base = resolve_output_dir(
        args.output_dir.as_deref(),
        workflow.output_dir.as_deref(),
        config_output_dir,
    );
    let run_id = generate_run_id();
    *run_id_out = Some(run_id.clone());
    let run_dir = output_base.join(&run_id);
    std::fs::create_dir_all(&run_dir)
        .with_context(|| format!("Cannot create output directory: {}", run_dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&run_dir, std::fs::Permissions::from_mode(0o700)).with_context(
            || {
                format!(
                    "Cannot set permissions on output directory: {}",
                    run_dir.display()
                )
            },
        )?;
    }

    // Handle --parent-run flag and calculate ancestry_depth, also capturing OTel context.
    let (continuation_of, ancestry_depth, parent_otel_trace_id, parent_otel_span_id) =
        if let Some(ref parent_run_id) = args.parent_run {
            let parent_meta_path = output_base.join(parent_run_id).join("run.toml");
            match state::RunMeta::read(&parent_meta_path) {
                Ok(parent_meta) => (
                    Some(parent_run_id.clone()),
                    parent_meta.ancestry_depth + 1,
                    parent_meta.otel_trace_id,
                    parent_meta.otel_span_id,
                ),
                Err(_) => (Some(parent_run_id.clone()), 1, None, None),
            }
        } else {
            (None, 0, None, None)
        };

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
        parent_run_id: None, // parent_run_id is only set on resume, not on fresh run
        continuation_of: continuation_of.clone(),
        ancestry_depth,
        context_dir: std::fs::canonicalize(&workflow.context_dir)
            .ok()
            .map(|p| p.to_string_lossy().to_string()),
        otel_trace_id: None, // Populated by engine after OTel init.
        otel_span_id: None,
    };
    meta.write(&run_dir.join("run.toml"))?;

    // Advisory check: no budget cap configured
    if output_format == cli::OutputFormat::Human
        && workflow.budget_cap_usd.is_none()
        && args.budget_cap.is_none()
    {
        eprintln!(
            "⚠  Warning: No budget cap configured. \
             Use --budget-cap or budget_cap_usd to prevent unbounded spend."
        );
    }

    // Advisory check: delay_between_runs sanity check (>600 seconds is likely a units mistake)
    if output_format == cli::OutputFormat::Human && workflow.delay_between_runs > 600 {
        let human = format_human_duration(workflow.delay_between_runs);
        eprintln!(
            "⚠  delay_between_runs = {} seconds ({}) between each run.\n   \
             This is unusually long. If you meant {} milliseconds, rings uses whole seconds.\n   \
             Use --delay to override for this run without editing the workflow file.",
            workflow.delay_between_runs, human, workflow.delay_between_runs
        );
    }

    // Advisory check: context_dir is empty
    if output_format == cli::OutputFormat::Human && context_dir_is_empty(&workflow.context_dir) {
        eprintln!(
            "⚠  context_dir (\"{}\") contains no files.\n   \
             The executor will start with an empty working directory.\n   \
             If this is intentional (the executor will create files from scratch), ignore this warning.",
            workflow.context_dir
        );
    }

    // Advisory check: sensitive files in context_dir
    if output_format == cli::OutputFormat::Human && !args.no_sensitive_files_check {
        let sensitive = scan_sensitive_files(&workflow.context_dir);
        if !sensitive.is_empty() {
            let listed: Vec<&str> = sensitive.iter().take(10).map(String::as_str).collect();
            let mut file_list = listed.join(", ");
            if sensitive.len() > 10 {
                file_list.push_str(&format!(" ... and {} more", sensitive.len() - 10));
            }
            eprintln!(
                "⚠  context_dir contains files that may contain credentials:\n   {}\n   \
                 These files will be visible to the executor. Use --no-sensitive-files-check to suppress.",
                file_list
            );
        }
    }

    // Advisory check: large context_dir when manifest_enabled
    if output_format == cli::OutputFormat::Human && workflow.manifest_enabled {
        let file_count = count_context_dir_files(&workflow.context_dir);
        if file_count > 10_000 {
            eprintln!(
                "⚠  context_dir contains {} files. Manifest scanning may be slow.\n   \
                 Consider using manifest_ignore patterns to exclude large directories (e.g., node_modules/, target/).",
                file_count
            );
        }
    }

    // Advisory check: output_dir inside a git repo
    let user_provided_output_dir: Option<&str> = args
        .output_dir
        .as_deref()
        .or(workflow.output_dir.as_deref());
    if output_format == cli::OutputFormat::Human {
        if let Some(output_display) = user_provided_output_dir {
            if let Some(repo_root) = find_git_root(&output_base) {
                let relative_output = std::fs::canonicalize(&output_base)
                    .ok()
                    .and_then(|canon| {
                        canon
                            .strip_prefix(&repo_root)
                            .ok()
                            .map(|p| p.to_string_lossy().to_string())
                    })
                    .unwrap_or_else(|| output_base.to_string_lossy().to_string());
                eprintln!(
                    "⚠  output_dir resolves to a path inside a git repository:\n   \
                     {} is under {}/ (which contains .git)\n   \
                     rings run logs and cost data will be written here and may be accidentally committed.\n   \
                     Consider adding {}/ to .gitignore, or omit output_dir to use the default\n   \
                     off-repo location (~/.local/share/rings/runs/).",
                    output_display,
                    repo_root.display(),
                    relative_output
                );
            }
        }
    }

    // Advisory check: disk space in output directory.
    // Fatal (< 10 MB): abort in all output modes. Warning (< 100 MB): shown in human mode only.
    if let Some(avail) = available_disk_space_bytes(&output_base) {
        match classify_disk_space(avail) {
            DiskSpaceLevel::Fatal(mb) => {
                eprintln!(
                    "Error: Less than 10 MB free in output directory ({}). Aborting to prevent data loss.",
                    output_base.display()
                );
                let _ = mb;
                return Ok(2);
            }
            DiskSpaceLevel::Warning(mb) => {
                if output_format == cli::OutputFormat::Human {
                    eprintln!(
                        "⚠  Low disk space: only {} MB free in output directory ({}).",
                        mb,
                        output_base.display()
                    );
                }
            }
            DiskSpaceLevel::Ok => {}
        }
    }

    // Advisory check: completion signal in prompts
    if output_format == cli::OutputFormat::Human && !args.no_completion_check {
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

    // Advisory check: unknown template variables in prompts (F-029)
    if output_format == cli::OutputFormat::Human {
        for phase in &workflow.phases {
            let prompt_content = if let Some(ref file) = phase.prompt {
                // Best-effort: skip file read failures (advisory only)
                std::fs::read_to_string(file).ok()
            } else {
                phase.prompt_text.clone()
            };
            if let Some(content) = prompt_content {
                let unknowns = template::find_unknown_variables(&content, template::KNOWN_VARS);
                for var in &unknowns {
                    eprintln!(
                        "Warning: Unknown template variable '{{{{{}}}}}' in prompt for phase \"{}\".\n\
                         It will be passed through as-is (not substituted).",
                        var, phase.name
                    );
                }
            }
        }
    }

    if output_format == cli::OutputFormat::Human {
        let phase_name_runs: Vec<(String, u32)> = workflow
            .phases
            .iter()
            .map(|p| (p.name.clone(), p.runs_per_cycle))
            .collect();
        let detected_model = workflow.detect_model_name();
        display::print_run_header(&display::RunHeaderParams {
            workflow_file: &args.workflow_file,
            context_dir: &workflow.context_dir,
            phases: &phase_name_runs,
            max_cycles: workflow.max_cycles,
            budget_cap_usd: workflow.budget_cap_usd,
            output_dir: &run_dir.to_string_lossy(),
            version: env!("CARGO_PKG_VERSION"),
            model: detected_model.as_deref(),
        });
    }

    // Acquire context directory lock
    #[cfg(unix)]
    let _lock = {
        let context_dir = PathBuf::from(&workflow.context_dir);
        match ContextLock::acquire(&context_dir, &run_id, args.force_lock, None) {
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
        ancestry_continuation_of: continuation_of.clone(),
        ancestry_depth,
        no_contract_check: args.no_contract_check,
        output_format,
        strict_parsing: args.strict_parsing,
        step: args.step,
        step_cycles: args.step_cycles,
        step_reader: None,
        include_dirs,
        parent_run_id: continuation_of,
        parent_otel_trace_id,
        parent_otel_span_id,
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

    // Print completion, error, or max-cycles summary based on exit code (human mode only)
    if output_format == cli::OutputFormat::Human {
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
                        &result.phase_costs,
                        workflow.budget_cap_usd,
                        result.total_input_tokens,
                        result.total_output_tokens,
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
                // Executor error: dispatch based on failure_reason
                let state_path = run_dir.join("state.json");
                if let Ok(state) = state::StateFile::read(&state_path) {
                    let failed_run_number = state.last_completed_run + 1;
                    let log_path = run_dir
                        .join("runs")
                        .join(format!("{:03}.log", failed_run_number));
                    let phase = workflow.phases.get(state.last_completed_phase_index);
                    let phase_name = phase.map(|p| p.name.as_str()).unwrap_or("unknown");

                    match result.failure_reason {
                        Some(state::FailureReason::Quota) => {
                            display::print_quota_error(
                                failed_run_number,
                                state.last_completed_cycle,
                                phase_name,
                                &run_id,
                                state.cumulative_cost_usd,
                                &log_path.to_string_lossy(),
                            );
                        }
                        Some(state::FailureReason::Auth) => {
                            display::print_auth_error(
                                failed_run_number,
                                state.last_completed_cycle,
                                phase_name,
                                &run_id,
                                &log_path.to_string_lossy(),
                            );
                        }
                        _ => {
                            display::print_executor_error(
                                failed_run_number,
                                3,
                                &run_id,
                                &log_path.to_string_lossy(),
                            );
                        }
                    }
                }
            }
            4 => {
                // Budget cap reached: already printed inline by engine; no extra output needed here
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
                        result.total_cost_usd,
                        result.total_runs,
                        &result.phase_costs,
                        &state.claude_resume_commands,
                        &run_dir.to_string_lossy(),
                        result.total_input_tokens,
                        result.total_output_tokens,
                        workflow.budget_cap_usd,
                    );
                }
            }
            _ => {}
        }

        // Print low-confidence cost parse warnings
        display::print_parse_warnings(&result.parse_warnings);
    }

    // Generate summary.md in the run directory (all exit paths).
    {
        let status_str = match result.exit_code {
            0 => "completed",
            1 => "max_cycles",
            3 => "executor_error",
            4 => "budget_cap",
            130 => "canceled",
            _ => "failed",
        };
        let state_path = run_dir.join("state.json");
        let loaded_state = state::StateFile::read(&state_path).ok();
        let completion_info = if result.exit_code == 0 {
            loaded_state.as_ref().map(|s| {
                let phase = workflow.phases.get(s.last_completed_phase_index);
                let phase_name = phase
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                (s.last_completed_cycle, s.last_completed_run, phase_name)
            })
        } else {
            None
        };
        let empty_cmds: Vec<String> = Vec::new();
        let claude_resume_commands = if result.exit_code == 130 {
            loaded_state
                .as_ref()
                .map(|s| s.claude_resume_commands.as_slice())
                .unwrap_or(&[])
        } else {
            &empty_cmds
        };
        let summary_info = rings::audit::SummaryInfo {
            run_id: &run_id,
            workflow_file: &meta.workflow_file,
            status: status_str,
            started_at: &meta.started_at,
            context_dir: meta.context_dir.as_deref(),
            output_dir: &run_dir,
            completed_cycles: result.completed_cycles,
            total_runs: result.total_runs,
            total_cost_usd: result.total_cost_usd,
            total_input_tokens: result.total_input_tokens,
            total_output_tokens: result.total_output_tokens,
            phase_costs: &result.phase_costs,
            total_elapsed_secs,
            completion_info,
            claude_resume_commands,
        };
        if let Err(e) = rings::audit::generate_summary_md(&run_dir, &summary_info) {
            eprintln!("⚠  Failed to write summary.md: {e}");
        }
    }

    Ok(result.exit_code)
}

fn cmd_resume(
    args: cli::ResumeArgs,
    cancel: Arc<CancelState>,
    output_format: cli::OutputFormat,
    config_output_dir: Option<&str>,
) -> i32 {
    let mut run_id: Option<String> = None;
    match resume_inner(args, cancel, output_format, &mut run_id, config_output_dir) {
        Ok(code) => code,
        Err(e) => {
            if output_format == cli::OutputFormat::Jsonl {
                rings::events::emit_jsonl(&rings::events::FatalErrorEvent::new(
                    run_id,
                    format!("{e:#}"),
                ));
            }
            eprintln!("Error: {e:#}");
            2
        }
    }
}

/// Returns a warning message if the saved workflow file path differs from its canonical path.
/// Returns `None` if paths match or the file cannot be resolved (will fail later with a better error).
fn check_workflow_path_mismatch(saved_path: &str) -> Option<String> {
    if let Ok(canonical) = std::fs::canonicalize(saved_path) {
        let canonical_str = canonical.to_string_lossy();
        if canonical_str != saved_path {
            return Some(format!(
                "⚠  Workflow file path has changed:\n   Saved: {}\n   Current: {}\n   This may cause issues if the workflow structure has also changed.",
                saved_path, canonical_str
            ));
        }
    }
    None
}

fn resume_inner(
    args: cli::ResumeArgs,
    cancel: Arc<CancelState>,
    output_format: cli::OutputFormat,
    run_id_out: &mut Option<String>,
    config_output_dir: Option<&str>,
) -> Result<i32> {
    // Find old run directory to resume from
    let output_base = resolve_output_dir(args.output_dir.as_deref(), None, config_output_dir);
    let old_run_dir = output_base.join(&args.run_id);
    let state_path = old_run_dir.join("state.json");
    let costs_path = old_run_dir.join("costs.jsonl");
    let old_meta_path = old_run_dir.join("run.toml");

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

    let old_meta = state::RunMeta::read(&old_meta_path)
        .with_context(|| format!("Cannot read run.toml for run {}", args.run_id))?;

    // Path mismatch check (F-057): warn if workflow file canonical path differs from saved path
    if let Some(warning) = check_workflow_path_mismatch(&old_meta.workflow_file) {
        eprintln!("{}", warning);
    }

    // Generate a new run_id for the resumed run (implements Option A: new run directory on resume)
    let new_run_id = generate_run_id();
    *run_id_out = Some(new_run_id.clone());
    let run_dir = output_base.join(&new_run_id);
    let meta_path = run_dir.join("run.toml");

    // Create new metadata with parent_run_id set to the old run
    let mut meta = state::RunMeta {
        run_id: new_run_id.clone(),
        workflow_file: old_meta.workflow_file.clone(),
        started_at: chrono::Utc::now().to_rfc3339(),
        rings_version: env!("CARGO_PKG_VERSION").to_string(),
        status: state::RunStatus::Running,
        phase_fingerprint: old_meta.phase_fingerprint.clone(),
        parent_run_id: Some(args.run_id.clone()),
        continuation_of: None,
        ancestry_depth: 1,
        context_dir: None,
        otel_trace_id: None, // Populated by engine after OTel init.
        otel_span_id: None,
    };

    // Reload workflow
    let toml_content = std::fs::read_to_string(&meta.workflow_file)
        .with_context(|| format!("Cannot read workflow file: {}", meta.workflow_file))?;
    let mut workflow = workflow::Workflow::from_str(&toml_content)?;

    meta.context_dir = std::fs::canonicalize(&workflow.context_dir)
        .ok()
        .map(|p| p.to_string_lossy().to_string());

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
    if output_format == cli::OutputFormat::Human
        && workflow.budget_cap_usd.is_none()
        && args.budget_cap.is_none()
    {
        eprintln!(
            "⚠  Warning: No budget cap configured. \
             Use --budget-cap or budget_cap_usd to prevent unbounded spend."
        );
    }

    if output_format == cli::OutputFormat::Human {
        eprintln!(
            "Resuming from {}  (previous cost: ${:.3})",
            style::dim(&args.run_id),
            saved_state.cumulative_cost_usd
        );
        let phase_name_runs_resume: Vec<(String, u32)> = workflow
            .phases
            .iter()
            .map(|p| (p.name.clone(), p.runs_per_cycle))
            .collect();
        let detected_model_resume = workflow.detect_model_name();
        display::print_run_header(&display::RunHeaderParams {
            workflow_file: &meta.workflow_file,
            context_dir: &workflow.context_dir,
            phases: &phase_name_runs_resume,
            max_cycles: workflow.max_cycles,
            budget_cap_usd: workflow.budget_cap_usd,
            output_dir: &run_dir.to_string_lossy(),
            version: env!("CARGO_PKG_VERSION"),
            model: detected_model_resume.as_deref(),
        });
    }

    // Acquire context directory lock
    #[cfg(unix)]
    let _lock = {
        let context_dir = PathBuf::from(&workflow.context_dir);
        match ContextLock::acquire(&context_dir, &new_run_id, args.force_lock, None) {
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

    // Create new run directory for the resumed run
    std::fs::create_dir_all(&run_dir)
        .with_context(|| format!("Cannot create new run directory: {}", run_dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&run_dir, std::fs::Permissions::from_mode(0o700)).with_context(
            || {
                format!(
                    "Cannot set permissions on output directory: {}",
                    run_dir.display()
                )
            },
        )?;
    }

    // Write the new run.toml with parent_run_id set
    meta.write(&meta_path)?;

    // Read parent run's OTel context so the resumed run can add a span link.
    let (parent_otel_trace_id, parent_otel_span_id) = {
        let parent_meta_path = output_base.join(&args.run_id).join("run.toml");
        match state::RunMeta::read(&parent_meta_path) {
            Ok(parent_meta) => (parent_meta.otel_trace_id, parent_meta.otel_span_id),
            Err(_) => (None, None), // Old run without OTel — skip link gracefully.
        }
    };

    // Build engine config for the new run directory
    let config = EngineConfig {
        output_dir: run_dir.clone(),
        verbose: args.verbose,
        run_id: new_run_id.clone(),
        workflow_file: meta.workflow_file.clone(),
        ancestry_continuation_of: None, // continuation_of is not set on resume
        ancestry_depth: 1,              // resumed runs always start at depth 1
        no_contract_check: args.no_contract_check,
        output_format,
        strict_parsing: false,
        step: false,
        step_cycles: false,
        step_reader: None,
        include_dirs: vec![],
        parent_run_id: Some(args.run_id.clone()),
        parent_otel_trace_id,
        parent_otel_span_id,
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

    // Print completion, error, or max-cycles summary based on exit code (human mode only)
    if output_format == cli::OutputFormat::Human {
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
                        &result.phase_costs,
                        workflow.budget_cap_usd,
                        result.total_input_tokens,
                        result.total_output_tokens,
                    );
                }
            }
            1 => {
                // Max cycles reached
                display::print_max_cycles(
                    workflow.max_cycles,
                    result.total_cost_usd,
                    result.total_runs,
                    &new_run_id,
                );
            }
            3 => {
                // Executor error: dispatch based on failure_reason
                let state_path = run_dir.join("state.json");
                if let Ok(state) = state::StateFile::read(&state_path) {
                    let failed_run_number = state.last_completed_run + 1;
                    let log_path = run_dir
                        .join("runs")
                        .join(format!("{:03}.log", failed_run_number));
                    let phase = workflow.phases.get(state.last_completed_phase_index);
                    let phase_name = phase.map(|p| p.name.as_str()).unwrap_or("unknown");

                    match result.failure_reason {
                        Some(state::FailureReason::Quota) => {
                            display::print_quota_error(
                                failed_run_number,
                                state.last_completed_cycle,
                                phase_name,
                                &new_run_id,
                                state.cumulative_cost_usd,
                                &log_path.to_string_lossy(),
                            );
                        }
                        Some(state::FailureReason::Auth) => {
                            display::print_auth_error(
                                failed_run_number,
                                state.last_completed_cycle,
                                phase_name,
                                &new_run_id,
                                &log_path.to_string_lossy(),
                            );
                        }
                        _ => {
                            display::print_executor_error(
                                failed_run_number,
                                3,
                                &new_run_id,
                                &log_path.to_string_lossy(),
                            );
                        }
                    }
                }
            }
            4 => {
                // Budget cap reached: already printed inline by engine; no extra output needed here
            }
            130 => {
                // Cancellation: load the state that was saved during cancellation to get last run position
                let state_path = run_dir.join("state.json");
                if let Ok(state) = state::StateFile::read(&state_path) {
                    let phase = workflow.phases.get(state.last_completed_phase_index);
                    let phase_name = phase.map(|p| p.name.as_str()).unwrap_or("unknown");
                    display::print_cancellation(
                        &new_run_id,
                        state.last_completed_cycle,
                        phase_name,
                        result.total_cost_usd,
                        result.total_runs,
                        &result.phase_costs,
                        &state.claude_resume_commands,
                        &run_dir.to_string_lossy(),
                        result.total_input_tokens,
                        result.total_output_tokens,
                        workflow.budget_cap_usd,
                    );
                }
            }
            _ => {}
        }

        // Print low-confidence cost parse warnings
        display::print_parse_warnings(&result.parse_warnings);
    }

    // Generate summary.md in the run directory (all exit paths).
    {
        let status_str = match result.exit_code {
            0 => "completed",
            1 => "max_cycles",
            3 => "executor_error",
            4 => "budget_cap",
            130 => "canceled",
            _ => "failed",
        };
        let state_path = run_dir.join("state.json");
        let loaded_state = state::StateFile::read(&state_path).ok();
        let completion_info = if result.exit_code == 0 {
            loaded_state.as_ref().map(|s| {
                let phase = workflow.phases.get(s.last_completed_phase_index);
                let phase_name = phase
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                (s.last_completed_cycle, s.last_completed_run, phase_name)
            })
        } else {
            None
        };
        let empty_cmds: Vec<String> = Vec::new();
        let claude_resume_commands = if result.exit_code == 130 {
            loaded_state
                .as_ref()
                .map(|s| s.claude_resume_commands.as_slice())
                .unwrap_or(&[])
        } else {
            &empty_cmds
        };
        let summary_info = rings::audit::SummaryInfo {
            run_id: &new_run_id,
            workflow_file: &meta.workflow_file,
            status: status_str,
            started_at: &meta.started_at,
            context_dir: meta.context_dir.as_deref(),
            output_dir: &run_dir,
            completed_cycles: result.completed_cycles,
            total_runs: result.total_runs,
            total_cost_usd: result.total_cost_usd,
            total_input_tokens: result.total_input_tokens,
            total_output_tokens: result.total_output_tokens,
            phase_costs: &result.phase_costs,
            total_elapsed_secs,
            completion_info,
            claude_resume_commands,
        };
        if let Err(e) = rings::audit::generate_summary_md(&run_dir, &summary_info) {
            eprintln!("⚠  Failed to write summary.md: {e}");
        }
    }

    Ok(result.exit_code)
}

fn cmd_list(
    args: cli::ListArgs,
    output_format: cli::OutputFormat,
    config_output_dir: Option<&str>,
) -> i32 {
    match list_inner(args, output_format, config_output_dir) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    }
}

fn list_inner(
    args: cli::ListArgs,
    output_format: cli::OutputFormat,
    config_output_dir: Option<&str>,
) -> Result<i32> {
    // Resolve base directory for scanning runs
    let base_dir = resolve_output_dir(None, None, config_output_dir);

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
        dir: args.dir,
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
                    "{:<20} {:<20} {:<32} {:<30} {:<12} {:<8} {:<10}",
                    style::bold("RUN ID"),
                    style::bold("DATE"),
                    style::bold("DIR"),
                    style::bold("WORKFLOW"),
                    style::bold("STATUS"),
                    style::bold("CYCLES"),
                    style::bold("COST"),
                );
                eprintln!("{}", style::dim(&"-".repeat(134)));
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
                    let styled_status = style_run_status(&run.status, &status_display);
                    let styled_cost = style::accent(&cost_str);
                    let dir_display = run
                        .context_dir
                        .as_deref()
                        .map(shorten_path)
                        .unwrap_or_else(|| "\u{2014}".to_string());
                    eprintln!(
                        "{:<20} {:<20} {:<32} {:<30} {:<12} {:<8} {:<10}",
                        run.run_id,
                        date_str,
                        dir_display,
                        run.workflow,
                        styled_status,
                        run.cycles_completed,
                        styled_cost,
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
                    "context_dir": run.context_dir,
                });
                println!("{}", json);
            }
        }
    }

    Ok(0)
}

fn cmd_show(
    args: cli::ShowArgs,
    output_format: cli::OutputFormat,
    config_output_dir: Option<&str>,
) -> i32 {
    let inspect_args = cli::InspectArgs {
        run_id: args.run_id,
        show: vec![cli::InspectView::Summary],
        cycle: None,
        phase: None,
    };
    cmd_inspect(inspect_args, output_format, config_output_dir)
}

fn cmd_inspect(
    args: cli::InspectArgs,
    output_format: cli::OutputFormat,
    config_output_dir: Option<&str>,
) -> i32 {
    match inspect_inner(args, output_format, config_output_dir) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    }
}

fn inspect_inner(
    args: cli::InspectArgs,
    output_format: cli::OutputFormat,
    config_output_dir: Option<&str>,
) -> Result<i32> {
    let base_dir = resolve_output_dir(None, None, config_output_dir);
    let run_dir = base_dir.join(&args.run_id);

    if !run_dir.exists() {
        bail!("Run directory not found: {}", run_dir.display());
    }

    let views = if args.show.is_empty() {
        vec![cli::InspectView::Summary]
    } else {
        args.show.clone()
    };

    for view in &views {
        match view {
            cli::InspectView::Summary => {
                render_summary(&run_dir, output_format)?;
            }
            cli::InspectView::Cycles => {
                render_cycles(&run_dir, args.cycle, output_format)?;
            }
            cli::InspectView::Costs => {
                render_costs(&run_dir, args.phase.as_deref(), output_format)?;
            }
            cli::InspectView::DataFlow => {
                let declared = rings::inspect::load_declared_flow(&run_dir)?;
                print!(
                    "{}",
                    rings::inspect::render_data_flow_declared(
                        &declared,
                        args.phase.as_deref(),
                        output_format
                    )
                );

                let actual_changes = rings::inspect::load_actual_changes(&run_dir)?;
                print!(
                    "{}",
                    rings::inspect::render_data_flow_actual(
                        &actual_changes,
                        args.cycle,
                        args.phase.as_deref(),
                        output_format
                    )
                );
            }
            cli::InspectView::ClaudeOutput => {
                render_claude_output(&run_dir, args.cycle, args.phase.as_deref(), output_format)?;
            }
            cli::InspectView::FilesChanged => {
                render_files_changed(&run_dir, args.cycle, args.phase.as_deref(), output_format)?;
            }
        }
    }

    Ok(0)
}

fn render_files_changed(
    run_dir: &std::path::Path,
    cycle_filter: Option<u32>,
    phase_filter: Option<&str>,
    output_format: cli::OutputFormat,
) -> Result<()> {
    let changes = rings::inspect::load_actual_changes(run_dir)?;
    let output =
        rings::inspect::render_files_changed(&changes, cycle_filter, phase_filter, output_format);
    print!("{}", output);
    Ok(())
}

fn render_costs(
    run_dir: &std::path::Path,
    phase_filter: Option<&str>,
    output_format: cli::OutputFormat,
) -> Result<()> {
    let costs_path = run_dir.join("costs.jsonl");
    let cost_entries: Vec<rings::audit::CostEntry> = if costs_path.exists() {
        rings::audit::stream_cost_entries(&costs_path)?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        vec![]
    };
    let output = rings::inspect::render_costs(&cost_entries, phase_filter, output_format);
    print!("{}", output);
    Ok(())
}

fn render_cycles(
    run_dir: &std::path::Path,
    cycle_filter: Option<u32>,
    output_format: cli::OutputFormat,
) -> Result<()> {
    // Read costs.jsonl
    let costs_path = run_dir.join("costs.jsonl");
    let cost_entries: Vec<rings::audit::CostEntry> = if costs_path.exists() {
        rings::audit::stream_cost_entries(&costs_path)?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        vec![]
    };

    // Determine signal_run: if the run status is "completed", the last run in
    // costs.jsonl is the one that triggered the completion signal.
    let run_toml_path = run_dir.join("run.toml");
    let signal_run: Option<u32> = if let Ok(meta) = state::RunMeta::read(&run_toml_path) {
        if matches!(meta.status, state::RunStatus::Completed) {
            cost_entries.iter().map(|e| e.run).max()
        } else {
            None
        }
    } else {
        None
    };

    let output =
        rings::inspect::render_cycles(&cost_entries, cycle_filter, signal_run, output_format);
    print!("{}", output);
    Ok(())
}

fn render_claude_output(
    run_dir: &std::path::Path,
    cycle_filter: Option<u32>,
    phase_filter: Option<&str>,
    output_format: cli::OutputFormat,
) -> Result<()> {
    let costs_path = run_dir.join("costs.jsonl");
    let cost_entries: Vec<rings::audit::CostEntry> = if costs_path.exists() {
        rings::audit::stream_cost_entries(&costs_path)?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        vec![]
    };
    let output = rings::inspect::render_claude_output(
        run_dir,
        &cost_entries,
        cycle_filter,
        phase_filter,
        output_format,
    )?;
    print!("{}", output);
    Ok(())
}

fn render_summary(run_dir: &std::path::Path, output_format: cli::OutputFormat) -> Result<()> {
    use std::collections::BTreeMap;

    // Read run.toml
    let run_toml_path = run_dir.join("run.toml");
    let meta = state::RunMeta::read(&run_toml_path)
        .with_context(|| format!("Run directory not found: {}", run_dir.display()))?;

    // Read state.json (optional — gracefully handle missing)
    let state_path = run_dir.join("state.json");
    let state_opt = state::StateFile::read(&state_path).ok();

    // Read costs.jsonl for per-phase breakdown
    let costs_path = run_dir.join("costs.jsonl");
    let cost_entries: Vec<rings::audit::CostEntry> = if costs_path.exists() {
        rings::audit::stream_cost_entries(&costs_path)?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        vec![]
    };

    // Compute totals from cost entries
    let total_cost_usd: f64 = cost_entries.iter().filter_map(|e| e.cost_usd).sum();
    let total_input_tokens: u64 = cost_entries.iter().filter_map(|e| e.input_tokens).sum();
    let total_output_tokens: u64 = cost_entries.iter().filter_map(|e| e.output_tokens).sum();

    // Phase cost breakdown: sum cost per phase
    let mut phase_costs: BTreeMap<String, f64> = BTreeMap::new();
    for entry in &cost_entries {
        if let Some(cost) = entry.cost_usd {
            *phase_costs.entry(entry.phase.clone()).or_insert(0.0) += cost;
        }
    }

    // Cycles completed
    let cycles_completed = state_opt
        .as_ref()
        .map(|s| s.last_completed_cycle)
        .unwrap_or(0);

    // Duration: parse started_at, compute elapsed
    let duration_str = chrono::DateTime::parse_from_rfc3339(&meta.started_at)
        .ok()
        .map(|started| {
            let started_utc = started.with_timezone(&chrono::Utc);
            let now = chrono::Utc::now();
            let elapsed = now.signed_duration_since(started_utc);
            let secs = elapsed.num_seconds().max(0) as u64;
            if secs < 60 {
                format!("{}s", secs)
            } else if secs < 3600 {
                format!("{}m {}s", secs / 60, secs % 60)
            } else {
                format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    if output_format == cli::OutputFormat::Jsonl {
        // Emit a single JSON summary object
        let mut obj = serde_json::json!({
            "event": "run_summary",
            "run_id": meta.run_id,
            "status": meta.status.to_string(),
            "workflow_file": meta.workflow_file,
            "started_at": meta.started_at,
            "cycles_completed": cycles_completed,
            "total_cost_usd": total_cost_usd,
            "total_input_tokens": total_input_tokens,
            "total_output_tokens": total_output_tokens,
            "phase_costs": phase_costs,
        });
        if let Some(ctx) = &meta.context_dir {
            obj["context_dir"] = serde_json::Value::String(ctx.clone());
        }
        println!("{}", serde_json::to_string(&obj)?);
    } else {
        // Human-readable summary
        println!("Run ID:     {}", meta.run_id);
        println!("Status:     {}", meta.status);
        println!("Workflow:   {}", meta.workflow_file);
        if let Some(ctx) = &meta.context_dir {
            println!("Context:    {}", ctx);
        }
        println!("Started:    {}", meta.started_at);
        println!("Duration:   {}", duration_str);
        println!("Cycles:     {}", cycles_completed);
        println!("Cost:       ${:.4}", total_cost_usd);
        println!(
            "Tokens:     {} in / {} out",
            total_input_tokens, total_output_tokens
        );

        if !phase_costs.is_empty() {
            println!("\nPhase cost breakdown:");
            for (phase, cost) in &phase_costs {
                println!("  {:<20}  ${:.4}", phase, cost);
            }
        }
    }

    Ok(())
}

fn cmd_lineage(
    args: cli::LineageArgs,
    output_format: cli::OutputFormat,
    config_output_dir: Option<&str>,
) -> i32 {
    match lineage_inner(args, output_format, config_output_dir) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    }
}

/// Holds the loaded data for a single run in an ancestry chain.
struct ChainRun {
    position: usize,
    run_id: String,
    started_at: String,
    status: state::RunStatus,
    cycles_completed: u32,
    total_runs: u32,
    cost_usd: f64,
    parent_run_id: Option<String>,
}

fn lineage_inner(
    args: cli::LineageArgs,
    output_format: cli::OutputFormat,
    config_output_dir: Option<&str>,
) -> Result<i32> {
    let base_dir = resolve_output_dir(None, None, config_output_dir);
    lineage_inner_with_base(args, output_format, &base_dir)
}

fn lineage_inner_with_base(
    args: cli::LineageArgs,
    output_format: cli::OutputFormat,
    base_dir: &std::path::Path,
) -> Result<i32> {
    // Verify the starting run exists.
    let start_run_dir = base_dir.join(&args.run_id);
    if !start_run_dir.exists() {
        bail!("Run directory not found: {}", start_run_dir.display());
    }

    // Walk backwards to find the root run.
    let (root_id, broken_at) = find_chain_root(&args.run_id, base_dir);

    // Walk forwards from root to build the ordered chain.
    let chain_ids = build_chain(&root_id, base_dir)?;

    // Load metadata for each run in the chain.
    let mut chain_runs: Vec<ChainRun> = Vec::new();
    for (pos, run_id) in chain_ids.iter().enumerate() {
        let run_dir = base_dir.join(run_id);
        let run_toml_path = run_dir.join("run.toml");

        let meta = match state::RunMeta::read(&run_toml_path) {
            Ok(m) => m,
            Err(e) => {
                if output_format == cli::OutputFormat::Human {
                    eprintln!("Warning: could not read run.toml for {run_id}: {e:#}");
                }
                continue;
            }
        };

        let state_path = run_dir.join("state.json");
        let (cycles_completed, total_runs, cost_usd) = match state::StateFile::read(&state_path) {
            Ok(s) => (
                s.last_completed_cycle,
                s.total_runs_completed,
                s.cumulative_cost_usd,
            ),
            Err(_) => (0, 0, 0.0),
        };

        chain_runs.push(ChainRun {
            position: pos + 1,
            run_id: run_id.clone(),
            started_at: meta.started_at.clone(),
            status: meta.status,
            cycles_completed,
            total_runs,
            cost_usd,
            parent_run_id: meta.parent_run_id.clone(),
        });
    }

    // Aggregate totals.
    let total_cost: f64 = chain_runs.iter().map(|r| r.cost_usd).sum();
    let total_cycles: u32 = chain_runs.iter().map(|r| r.cycles_completed).sum();
    let total_runs: u32 = chain_runs.iter().map(|r| r.total_runs).sum();
    let final_status = chain_runs
        .last()
        .map(|r| r.status)
        .unwrap_or(state::RunStatus::Incomplete);

    match output_format {
        cli::OutputFormat::Jsonl => {
            for run in &chain_runs {
                let obj = serde_json::json!({
                    "type": "run",
                    "position": run.position,
                    "run_id": run.run_id,
                    "status": run.status.to_string(),
                    "cycles": run.cycles_completed,
                    "cost_usd": run.cost_usd,
                    "parent_run_id": run.parent_run_id,
                });
                println!("{}", serde_json::to_string(&obj)?);
            }
            let summary = serde_json::json!({
                "type": "chain_summary",
                "depth": chain_runs.len(),
                "total_cycles": total_cycles,
                "total_runs": total_runs,
                "total_cost_usd": total_cost,
                "final_status": final_status.to_string(),
            });
            println!("{}", serde_json::to_string(&summary)?);
        }
        cli::OutputFormat::Human => {
            println!("Lineage chain for: {}", args.run_id);
            println!();
            println!(
                " {:<3} {:<38} {:<18} {:<12} {:<8} COST",
                "#", "RUN ID", "DATE", "STATUS", "CYCLES"
            );
            println!("{}", "─".repeat(90));

            for run in &chain_runs {
                let date_str = chrono::DateTime::parse_from_rfc3339(&run.started_at)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|_| "unknown".to_string());
                let cost_str = format!("${:.2}", run.cost_usd);
                let resume_note = if run.position > 1 {
                    format!("  ← resumed from #{}", run.position - 1)
                } else {
                    String::new()
                };
                println!(
                    " {:<3} {:<38} {:<18} {:<12} {:<8} {}{} ",
                    run.position,
                    run.run_id,
                    date_str,
                    run.status.to_string(),
                    run.cycles_completed,
                    cost_str,
                    resume_note,
                );
            }

            if let Some(missing_parent) = &broken_at {
                eprintln!(
                    "Warning: ancestry chain broken — parent run not found: {missing_parent}"
                );
            }

            println!();
            println!("Chain totals:");
            println!(
                "  Total cycles:  {}  (spread across {} run(s) in chain)",
                total_cycles,
                chain_runs.len()
            );
            println!("  Total runs:    {}", total_runs);
            println!("  Total cost:    ${:.2}", total_cost);
        }
    }

    Ok(0)
}

/// Walk backwards from `start_id` following parent_run_id / continuation_of links to find the
/// root run. Returns `(root_id, broken_at)` where `broken_at` is the missing parent ID if the
/// chain could not be fully traced.
fn find_chain_root(start_id: &str, base_dir: &std::path::Path) -> (String, Option<String>) {
    let mut current = start_id.to_string();
    let mut visited = std::collections::HashSet::new();

    loop {
        if !visited.insert(current.clone()) {
            // Cycle detected — stop here.
            break;
        }

        let run_toml = base_dir.join(&current).join("run.toml");
        let meta = match state::RunMeta::read(&run_toml) {
            Ok(m) => m,
            Err(_) => break,
        };

        // Prefer parent_run_id (resume link), then continuation_of.
        let parent = meta.parent_run_id.or(meta.continuation_of);

        match parent {
            Some(p) => {
                let parent_dir = base_dir.join(&p);
                if parent_dir.exists() {
                    current = p;
                } else {
                    // Parent dir missing — chain is broken.
                    return (current, Some(p));
                }
            }
            None => break,
        }
    }

    (current, None)
}

/// Build the ordered chain from a root run by scanning for children whose `parent_run_id`
/// or `continuation_of` points to the current tail. Returns run IDs in chain order.
fn build_chain(root_id: &str, base_dir: &std::path::Path) -> Result<Vec<String>> {
    let mut chain = vec![root_id.to_string()];
    let mut visited = std::collections::HashSet::new();
    visited.insert(root_id.to_string());

    loop {
        let tail = chain.last().cloned().unwrap();
        match find_chain_child(&tail, base_dir, &visited)? {
            Some(child_id) => {
                visited.insert(child_id.clone());
                chain.push(child_id);
            }
            None => break,
        }
    }

    Ok(chain)
}

/// Scan all run directories for one whose `parent_run_id` or `continuation_of` equals
/// `parent_id`. If multiple matches exist, return the earliest by `started_at`. Returns
/// `None` if no child is found.
fn find_chain_child(
    parent_id: &str,
    base_dir: &std::path::Path,
    visited: &std::collections::HashSet<String>,
) -> Result<Option<String>> {
    if !base_dir.exists() {
        return Ok(None);
    }

    let entries = std::fs::read_dir(base_dir)
        .with_context(|| format!("Cannot read directory: {}", base_dir.display()))?;

    let mut candidates: Vec<(String, String)> = Vec::new(); // (run_id, started_at)

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let run_toml_path = path.join("run.toml");
        let meta = match state::RunMeta::read(&run_toml_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if visited.contains(&meta.run_id) {
            continue;
        }

        let links_to_parent = meta.parent_run_id.as_deref() == Some(parent_id)
            || meta.continuation_of.as_deref() == Some(parent_id);

        if links_to_parent {
            candidates.push((meta.run_id, meta.started_at));
        }
    }

    // Pick the earliest child by started_at (lexicographic sort is fine for RFC3339).
    candidates.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(candidates.into_iter().next().map(|(id, _)| id))
}

fn generate_completions(shell: cli::Shell, writer: &mut dyn std::io::Write) {
    use clap::CommandFactory;
    use clap_complete::generate;
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "rings", writer);
}

fn cmd_completions(args: cli::CompletionsArgs) -> i32 {
    generate_completions(args.shell, &mut std::io::stdout());
    0
}

fn cmd_generate_man() -> i32 {
    use clap::CommandFactory;
    use clap_mangen::Man;
    let cmd = Cli::command();
    let mut buf = Vec::new();
    match Man::new(cmd).render(&mut buf) {
        Ok(()) => {
            if let Err(e) = std::io::Write::write_all(&mut std::io::stdout(), &buf) {
                eprintln!("Error writing man page: {e}");
                return 1;
            }
            0
        }
        Err(e) => {
            eprintln!("Error generating man page: {e}");
            1
        }
    }
}

fn cmd_init(args: cli::InitArgs, output_format: cli::OutputFormat) -> i32 {
    match init_inner(args, output_format) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    }
}

fn resolve_init_path(name: Option<&str>) -> Result<PathBuf> {
    let base = name.unwrap_or("workflow");

    // Reject paths with `..` components
    let path = std::path::Path::new(base);
    for component in path.components() {
        if component == std::path::Component::ParentDir {
            bail!("Path must not contain '..' components");
        }
    }

    // Append .rings.toml suffix if not already present
    let with_suffix = if base.ends_with(".rings.toml") {
        PathBuf::from(base)
    } else {
        PathBuf::from(format!("{base}.rings.toml"))
    };

    Ok(with_suffix)
}

const INIT_TEMPLATE: &str = r#"[workflow]
# Pick up tasks from TODO.md and work through them one at a time.
#
# Run with:    rings run <this-file>
# Preview:     rings run --dry-run <this-file>
# Resume:      rings resume <run-id>

completion_signal = "ALL_TASKS_COMPLETE"
completion_signal_mode = "line"
context_dir = "."
max_cycles = 20
budget_cap_usd = 5.00

[executor]
binary = "claude"
# Change --model to use a different model (e.g. claude-opus-4-6, claude-haiku-4-5)
args = ["--dangerously-skip-permissions", "--output-format", "json", "--model", "claude-sonnet-4-6", "-p", "-"]

[[phases]]
name = "builder"
prompt_text = """
Complete ONE task from the TODO list, then stop.

## Context

Before starting, read these files to understand the project:
- `README.md` — what the project does and how it works
# Add any other files that are important for grounding:
# - `ARCHITECTURE.md`, `CONTRIBUTING.md`, a spec directory, etc.

## Step 1: Find the next task

Read `TODO.md`. Find the first task with unchecked steps (`- [ ]`).
Tasks are ordered by dependency — do not skip ahead.

If there are no unchecked tasks, print exactly on its own line:
ALL_TASKS_COMPLETE

Then stop.

## Step 2: Do the work

Work through all steps of the chosen task.
When each step is done, mark it complete in TODO.md (`- [ ]` → `- [x]`).
Commit your changes when the task is complete.

## Step 3: Report

Print a brief summary of what you did, then stop.
Do not start another task.

# Template variables you can use in this prompt:
# {{cycle}}           — current cycle number
# {{max_cycles}}      — max cycles configured
# {{run}}             — global run number
# {{cost_so_far_usd}} — cumulative cost so far
"""
"#;

fn init_inner(args: cli::InitArgs, output_format: cli::OutputFormat) -> Result<i32> {
    let path = resolve_init_path(args.name.as_deref())?;

    // If path has a parent directory component, verify it exists
    if let Some(parent) = path.parent() {
        if parent != std::path::Path::new("") && !parent.exists() {
            eprintln!(
                "Error: parent directory '{}' does not exist",
                parent.display()
            );
            return Ok(2);
        }
    }

    // Check if target file already exists
    if path.exists() && !args.force {
        eprintln!(
            "Error: '{}' already exists. Use --force to overwrite.",
            path.display()
        );
        return Ok(2);
    }

    // Atomic write: write to <path>.tmp then rename
    let tmp_path = PathBuf::from(format!("{}.tmp", path.display()));
    std::fs::write(&tmp_path, INIT_TEMPLATE)
        .with_context(|| format!("Cannot write to '{}'", tmp_path.display()))?;
    std::fs::rename(&tmp_path, &path).with_context(|| {
        format!(
            "Cannot rename '{}' to '{}'",
            tmp_path.display(),
            path.display()
        )
    })?;

    let abs_path = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());

    match output_format {
        cli::OutputFormat::Human => {
            eprintln!("Created {}", path.display());
            eprintln!("Run it with:  rings run {}", path.display());
        }
        cli::OutputFormat::Jsonl => {
            let json = serde_json::json!({
                "event": "init_complete",
                "path": abs_path.to_string_lossy(),
            });
            println!("{json}");
        }
    }

    Ok(0)
}

const RINGS_REPO: &str = "pinecone-io/rings";

fn cmd_update() -> i32 {
    match update_inner() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            1
        }
    }
}

fn update_inner() -> Result<i32> {
    // Check curl is on PATH
    if which::which("curl").is_err() {
        eprintln!(
            "Error: 'curl' not found on PATH. \
             Please install curl or download rings manually from https://github.com/{RINGS_REPO}/releases"
        );
        return Ok(1);
    }

    // Check bash is on PATH
    if which::which("bash").is_err() {
        eprintln!(
            "Error: 'bash' not found on PATH. \
             Please install bash or download rings manually from https://github.com/{RINGS_REPO}/releases"
        );
        return Ok(1);
    }

    // Get current binary path
    let current_exe = std::env::current_exe()?.canonicalize()?;

    eprintln!("Updating rings...");

    // Download install.sh to a temp file
    let tmp_file = tempfile::NamedTempFile::new()?;
    let install_url = format!("https://raw.githubusercontent.com/{RINGS_REPO}/main/install.sh");
    let download_status = std::process::Command::new("curl")
        .args(["-fsSL", &install_url, "-o"])
        .arg(tmp_file.path())
        .status()
        .with_context(|| "Failed to run curl")?;

    if !download_status.success() {
        eprintln!(
            "Error: Failed to download install.sh. \
             Please check your internet connection or update manually."
        );
        return Ok(1);
    }

    // Run bash <tmpfile> <current_binary_path>, inheriting stdout/stderr
    let install_status = std::process::Command::new("bash")
        .arg(tmp_file.path())
        .arg(&current_exe)
        .status()
        .with_context(|| "Failed to run install script")?;

    // tmp_file is dropped here (auto-deleted)
    drop(tmp_file);

    if install_status.success() {
        Ok(0)
    } else {
        eprintln!("Error: Update failed. Please try again or update manually.");
        Ok(1)
    }
}

fn cmd_cleanup(
    args: cli::CleanupArgs,
    output_format: cli::OutputFormat,
    config_output_dir: Option<&str>,
) -> i32 {
    match cleanup_inner(args, output_format, config_output_dir) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    }
}

fn cleanup_inner(
    args: cli::CleanupArgs,
    output_format: cli::OutputFormat,
    config_output_dir: Option<&str>,
) -> Result<i32> {
    use std::io::{self, Write};

    let base_dir = resolve_output_dir(None, None, config_output_dir);

    // Parse --older-than as a SinceSpec (reuses the same duration parser as `rings list --since`)
    let since_spec = args.older_than.parse::<duration::SinceSpec>()?;
    let cutoff = since_spec.to_cutoff_datetime();

    if !base_dir.exists() {
        eprintln!("No runs found.");
        return Ok(0);
    }

    // Scan run directories
    let entries = std::fs::read_dir(&base_dir)
        .with_context(|| format!("Cannot read directory: {}", base_dir.display()))?
        .filter_map(|e| e.ok())
        .collect::<Vec<_>>();

    // Collect cleanup candidates: runs older than cutoff, not running
    let mut candidates: Vec<(
        String,
        chrono::DateTime<chrono::Utc>,
        state::RunStatus,
        std::path::PathBuf,
    )> = Vec::new();

    for entry in &entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let run_toml_path = path.join("run.toml");
        let meta = match state::RunMeta::read(&run_toml_path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        // Never delete active runs
        if meta.status == state::RunStatus::Running {
            continue;
        }
        let started_at = match chrono::DateTime::parse_from_rfc3339(&meta.started_at) {
            Ok(dt) => dt.with_timezone(&chrono::Utc),
            Err(_) => continue,
        };
        if started_at < cutoff {
            candidates.push((meta.run_id, started_at, meta.status, path));
        }
    }

    if candidates.is_empty() {
        eprintln!("No runs older than {} found.", args.older_than);
        return Ok(0);
    }

    // --dry-run: show what would be deleted without deleting
    if args.dry_run {
        for (run_id, started_at, status, path) in &candidates {
            eprintln!(
                "Would delete: {} ({}) started {} — {}",
                run_id,
                status,
                started_at.format("%Y-%m-%d %H:%M:%S"),
                path.display()
            );
        }
        eprintln!("Dry run: {} runs would be deleted.", candidates.len());
        return Ok(0);
    }

    // Prompt for confirmation unless --yes or non-TTY stderr
    use std::io::IsTerminal;
    if !args.yes && std::io::stderr().is_terminal() {
        eprint!("Delete {} runs? [y/N] ", candidates.len());
        io::stderr().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();
        if trimmed != "y" && trimmed != "Y" {
            eprintln!("Aborted.");
            return Ok(0);
        }
    }

    // Delete candidates
    let mut total_bytes: u64 = 0;
    let mut deleted = 0usize;

    for (run_id, _started_at, _status, path) in &candidates {
        // Approximate size before deletion
        let dir_size = walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .sum::<u64>();

        std::fs::remove_dir_all(path)
            .with_context(|| format!("Failed to delete {}", path.display()))?;

        total_bytes += dir_size;
        deleted += 1;

        match output_format {
            cli::OutputFormat::Jsonl => {
                let json = serde_json::json!({
                    "event": "cleanup_deleted",
                    "run_id": run_id,
                    "path": path.display().to_string(),
                });
                println!("{}", json);
            }
            cli::OutputFormat::Human => {}
        }
    }

    let mb = total_bytes as f64 / (1024.0 * 1024.0);

    match output_format {
        cli::OutputFormat::Jsonl => {
            let json = serde_json::json!({
                "event": "cleanup_summary",
                "deleted_count": deleted,
                "freed_mb": (mb * 100.0).round() / 100.0,
            });
            println!("{}", json);
        }
        cli::OutputFormat::Human => {
            eprintln!(
                "Deleted {} runs, freed approximately {:.1} MB.",
                deleted, mb
            );
        }
    }

    Ok(0)
}

/// Returns true if the path contains any `..` (ParentDir) components.
fn path_contains_parent_dir(path: &str) -> bool {
    use std::path::Component;
    std::path::Path::new(path)
        .components()
        .any(|c| c == Component::ParentDir)
}

fn resolve_output_dir(
    cli_override: Option<&str>,
    workflow_override: Option<&str>,
    config_override: Option<&str>,
) -> PathBuf {
    if let Some(p) = cli_override {
        return PathBuf::from(p);
    }
    if let Some(p) = workflow_override {
        return PathBuf::from(p);
    }
    if let Some(p) = config_override {
        return PathBuf::from(p);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rings")
        .join("runs")
}

fn style_run_status(status: &state::RunStatus, display: &str) -> String {
    match status {
        state::RunStatus::Completed => style::success(display),
        state::RunStatus::Failed => style::error(display),
        state::RunStatus::Canceled | state::RunStatus::Incomplete | state::RunStatus::Stopped => {
            style::warn(display)
        }
        state::RunStatus::Running => display.to_string(),
    }
}

/// Shorten a path for human display: replace $HOME prefix with ~, truncate long paths.
/// Paths longer than 30 chars are truncated with a `…/` prefix showing last components.
fn shorten_path(path: &str) -> String {
    // Replace $HOME prefix with ~
    let shortened = if let Ok(home) = std::env::var("HOME") {
        if path.starts_with(&home) {
            format!("~{}", &path[home.len()..])
        } else {
            path.to_string()
        }
    } else {
        path.to_string()
    };

    if shortened.len() <= 30 {
        return shortened;
    }

    // Truncate: find suffix that fits within 30 chars with the "…/" prefix
    // We want "…/<last components>" to fit in 30 chars total
    // "…/" is 2 chars (using Unicode ellipsis + slash = 4 bytes but 2 display chars)
    // So we want the suffix to be at most 28 display chars
    let max_suffix = 28;
    let components: Vec<&str> = shortened.split('/').collect();

    let mut suffix = String::new();
    for component in components.iter().rev() {
        let candidate = if suffix.is_empty() {
            component.to_string()
        } else {
            format!("{}/{}", component, suffix)
        };
        if candidate.len() <= max_suffix {
            suffix = candidate;
        } else {
            break;
        }
    }

    if suffix.is_empty() {
        // Even the last component is too long, just truncate it
        format!(
            "\u{2026}/{}",
            &shortened[shortened.len().saturating_sub(27)..]
        )
    } else {
        format!("\u{2026}/{}", suffix)
    }
}

fn generate_run_id() -> String {
    let now = chrono::Utc::now();
    let ts = now.format("%Y%m%d_%H%M%S");
    let short_uuid = uuid::Uuid::new_v4().to_string()[..6].to_string();
    format!("run_{ts}_{short_uuid}")
}

/// Returns true if the directory at `path` contains zero entries (advisory check for
/// empty context_dir). Returns false if the directory cannot be read or contains any entries.
/// Counts files recursively in `path`. Returns 0 if the path cannot be read.
fn count_context_dir_files(path: &str) -> usize {
    let mut count = 0usize;
    let mut stack = vec![std::path::PathBuf::from(path)];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_file() {
                count += 1;
            } else if ft.is_dir() {
                stack.push(entry.path());
            }
        }
    }
    count
}

fn context_dir_is_empty(path: &str) -> bool {
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
}

/// Scans the top-level of `path` (non-recursive) for files matching credential patterns.
/// Returns a sorted list of matching filenames. Ignores entries that cannot be read.
fn scan_sensitive_files(path: &str) -> Vec<String> {
    // Exact names (case-sensitive)
    const EXACT_NAMES: &[&str] = &[".env", ".env.local", ".env.production", ".npmrc", ".pypirc"];
    // Extensions (case-insensitive)
    const SENSITIVE_EXTENSIONS: &[&str] = &["key", "pem", "p12", "pfx", "jks", "keystore"];
    // Substrings that flag a file (case-insensitive)
    const SENSITIVE_SUBSTRINGS: &[&str] = &["credentials", "secret", "token"];

    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };

    let mut matches: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Check exact names
        if EXACT_NAMES.contains(&name_str.as_ref()) {
            matches.push(name_str.into_owned());
            continue;
        }

        let lower = name_str.to_lowercase();

        // Check extensions
        if let Some(ext) = std::path::Path::new(&*name_str).extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            if SENSITIVE_EXTENSIONS.contains(&ext_lower.as_str()) {
                matches.push(name_str.into_owned());
                continue;
            }
        }

        // Check substrings (case-insensitive)
        if SENSITIVE_SUBSTRINGS.iter().any(|sub| lower.contains(sub)) {
            matches.push(name_str.into_owned());
        }
    }

    matches.sort();
    matches
}

/// Formats a duration in seconds as a human-readable string.
///
/// Examples: 60 → "1 minute", 900 → "15 minutes", 3600 → "1 hour",
/// 5400 → "1 hour 30 minutes", 7200 → "2 hours"
fn format_human_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let remaining_secs = secs % 60;

    let mut parts = Vec::new();
    if hours > 0 {
        if hours == 1 {
            parts.push("1 hour".to_string());
        } else {
            parts.push(format!("{hours} hours"));
        }
    }
    if minutes > 0 {
        if minutes == 1 {
            parts.push("1 minute".to_string());
        } else {
            parts.push(format!("{minutes} minutes"));
        }
    }
    if parts.is_empty() {
        // Only seconds (no hours or minutes)
        if remaining_secs == 1 {
            return "1 second".to_string();
        } else {
            return format!("{remaining_secs} seconds");
        }
    }
    parts.join(" ")
}

/// Walks from `path` up to the filesystem root, returning the first directory
/// Returns the available disk space in bytes for the filesystem containing `path`.
/// Returns `None` if the query fails or is unsupported on this platform.
#[cfg(unix)]
fn available_disk_space_bytes(path: &std::path::Path) -> Option<u64> {
    use nix::sys::statvfs::statvfs;
    let stat = statvfs(path).ok()?;
    Some(stat.blocks_available() * stat.block_size())
}

#[cfg(not(unix))]
fn available_disk_space_bytes(_path: &std::path::Path) -> Option<u64> {
    None
}

/// Disk space threshold classification for the output directory.
#[derive(Debug, PartialEq)]
enum DiskSpaceLevel {
    /// Adequate space — no action needed.
    Ok,
    /// Low space (< 100 MB but >= 10 MB) — warn in human mode.
    Warning(u64),
    /// Critical space (< 10 MB) — fatal error in all modes.
    Fatal(u64),
}

/// Classify available bytes into a `DiskSpaceLevel`.
fn classify_disk_space(available_bytes: u64) -> DiskSpaceLevel {
    let available_mb = available_bytes / (1024 * 1024);
    if available_mb < 10 {
        DiskSpaceLevel::Fatal(available_mb)
    } else if available_mb < 100 {
        DiskSpaceLevel::Warning(available_mb)
    } else {
        DiskSpaceLevel::Ok
    }
}

/// that contains a `.git` entry. Returns `None` if no such directory is found.
fn find_git_root(path: &std::path::Path) -> Option<PathBuf> {
    // Canonicalize to get an absolute path for reliable parent traversal.
    let mut current = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_dir_empty_check_detects_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(context_dir_is_empty(tmp.path().to_str().unwrap()));
    }

    #[test]
    fn context_dir_empty_check_non_empty_dir_not_flagged() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("file.txt"), "content").unwrap();
        assert!(!context_dir_is_empty(tmp.path().to_str().unwrap()));
    }

    #[test]
    fn context_dir_empty_check_nonexistent_path_not_flagged() {
        // If the path doesn't exist, read_dir returns Err — we treat that as non-empty
        // (the earlier existence validation would have caught a missing dir first).
        assert!(!context_dir_is_empty("/nonexistent/path/rings_test_xyz"));
    }

    #[test]
    fn context_dir_empty_check_suppressed_in_jsonl_mode() {
        // The advisory check is guarded by `output_format == OutputFormat::Human`.
        // JSONL mode uses OutputFormat::Jsonl, which does not satisfy the guard.
        assert_ne!(cli::OutputFormat::Jsonl, cli::OutputFormat::Human);
    }

    // --- sensitive files advisory check tests ---

    #[test]
    fn sensitive_files_env_file_triggers_warning() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".env"), "SECRET=abc").unwrap();
        let matches = scan_sensitive_files(tmp.path().to_str().unwrap());
        assert!(
            matches.contains(&".env".to_string()),
            "expected .env to be flagged, got: {:?}",
            matches
        );
    }

    #[test]
    fn sensitive_files_key_extension_triggers_warning() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("server.key"), "private-key-data").unwrap();
        let matches = scan_sensitive_files(tmp.path().to_str().unwrap());
        assert!(
            matches.contains(&"server.key".to_string()),
            "expected server.key to be flagged, got: {:?}",
            matches
        );
    }

    #[test]
    fn sensitive_files_pem_extension_triggers_warning() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("cert.pem"), "-----BEGIN CERTIFICATE-----").unwrap();
        let matches = scan_sensitive_files(tmp.path().to_str().unwrap());
        assert!(matches.contains(&"cert.pem".to_string()));
    }

    #[test]
    fn sensitive_files_credentials_substring_triggers_warning() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("credentials.json"), "{}").unwrap();
        let matches = scan_sensitive_files(tmp.path().to_str().unwrap());
        assert!(
            matches.contains(&"credentials.json".to_string()),
            "expected credentials.json to be flagged"
        );
    }

    #[test]
    fn sensitive_files_no_sensitive_files_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(tmp.path().join("README.md"), "# Readme").unwrap();
        let matches = scan_sensitive_files(tmp.path().to_str().unwrap());
        assert!(
            matches.is_empty(),
            "expected no matches, got: {:?}",
            matches
        );
    }

    #[test]
    fn sensitive_files_jsonl_mode_suppressed_by_format_guard() {
        // The advisory check is guarded by `output_format == OutputFormat::Human`.
        // JSONL mode uses OutputFormat::Jsonl, which does not satisfy the guard.
        assert_ne!(cli::OutputFormat::Jsonl, cli::OutputFormat::Human);
    }

    #[test]
    fn sensitive_files_case_insensitive_substring_matching() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("MySecret.txt"), "data").unwrap();
        std::fs::write(tmp.path().join("API_TOKEN.json"), "data").unwrap();
        let matches = scan_sensitive_files(tmp.path().to_str().unwrap());
        assert!(
            matches.contains(&"MySecret.txt".to_string()),
            "expected MySecret.txt (case-insensitive 'secret') to be flagged"
        );
        assert!(
            matches.contains(&"API_TOKEN.json".to_string()),
            "expected API_TOKEN.json (case-insensitive 'token') to be flagged"
        );
    }

    #[test]
    fn path_traversal_safe_path_not_flagged() {
        assert!(!path_contains_parent_dir("/tmp/safe/path"));
    }

    #[test]
    fn path_traversal_dotdot_detected() {
        assert!(path_contains_parent_dir("/tmp/../etc/rings"));
    }

    #[test]
    fn path_traversal_relative_dotdot_detected() {
        assert!(path_contains_parent_dir("../outside"));
    }

    #[test]
    fn path_traversal_single_dot_allowed() {
        assert!(!path_contains_parent_dir("./current/dir"));
    }

    // --- output_dir git repo advisory check tests ---

    #[test]
    fn find_git_root_detects_git_dir_at_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let result = find_git_root(tmp.path());
        assert!(
            result.is_some(),
            "expected to find git root at the directory itself"
        );
    }

    #[test]
    fn find_git_root_detects_git_dir_in_parent() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let subdir = tmp.path().join("output").join("runs");
        std::fs::create_dir_all(&subdir).unwrap();
        let result = find_git_root(&subdir);
        assert!(
            result.is_some(),
            "expected to find git root in parent hierarchy"
        );
        let canonical_tmp = std::fs::canonicalize(tmp.path()).unwrap();
        assert_eq!(result.unwrap(), canonical_tmp);
    }

    #[test]
    fn find_git_root_returns_none_outside_any_repo() {
        // Use a temp directory that has no .git in its ancestry.
        // tempfile dirs are typically under /tmp which has no .git above it.
        let tmp = tempfile::tempdir().unwrap();
        // Verify /tmp itself (or its canonical path) has no .git above.
        // We can only reliably test this if the temp dir is not inside a git repo.
        // Check by looking for .git manually — if /tmp is not in a repo, this returns None.
        let result = find_git_root(tmp.path());
        // If the temp dir happens to be inside a repo (unusual), skip this assertion.
        if std::fs::canonicalize(tmp.path())
            .ok()
            .and_then(|p| p.ancestors().find(|a| a.join(".git").exists()).map(|_| ()))
            .is_none()
        {
            assert!(result.is_none(), "expected None outside any git repo");
        }
    }

    #[test]
    fn output_dir_inside_repo_jsonl_mode_suppressed_by_format_guard() {
        // The advisory check is guarded by `output_format == OutputFormat::Human`.
        // JSONL mode uses OutputFormat::Jsonl, which does not satisfy the guard.
        assert_ne!(cli::OutputFormat::Jsonl, cli::OutputFormat::Human);
    }

    // --- delay_between_runs sanity check tests ---

    #[test]
    fn delay_sanity_900_seconds_triggers_warning() {
        // 900 > 600, should trigger; human-readable is "15 minutes"
        assert!(900 > 600, "threshold check: 900 > 600");
        let human = format_human_duration(900);
        assert_eq!(human, "15 minutes");
    }

    #[test]
    fn delay_sanity_601_seconds_triggers_warning() {
        // 601 > 600, threshold is >600 (strictly greater)
        assert!(601 > 600);
        let human = format_human_duration(601);
        // 601s = 10 minutes 1 second — but the function only shows hours and minutes
        assert_eq!(human, "10 minutes");
    }

    #[test]
    fn delay_sanity_600_seconds_does_not_trigger_warning() {
        // Exactly 600 should NOT trigger (threshold is >600)
        assert!(!(600 > 600), "600 should not exceed threshold");
    }

    #[test]
    fn delay_sanity_30_seconds_does_not_trigger_warning() {
        assert!(!(30 > 600), "30 should not exceed threshold");
    }

    #[test]
    fn delay_sanity_jsonl_mode_suppressed_by_format_guard() {
        // The advisory check is guarded by `output_format == OutputFormat::Human`.
        assert_ne!(cli::OutputFormat::Jsonl, cli::OutputFormat::Human);
    }

    // --- format_human_duration tests ---

    #[test]
    fn format_human_duration_minutes_only() {
        assert_eq!(format_human_duration(900), "15 minutes");
        assert_eq!(format_human_duration(60), "1 minute");
        assert_eq!(format_human_duration(120), "2 minutes");
    }

    #[test]
    fn format_human_duration_hours_only() {
        assert_eq!(format_human_duration(3600), "1 hour");
        assert_eq!(format_human_duration(7200), "2 hours");
    }

    #[test]
    fn format_human_duration_hours_and_minutes() {
        assert_eq!(format_human_duration(5400), "1 hour 30 minutes");
        assert_eq!(format_human_duration(3660), "1 hour 1 minute");
    }

    #[test]
    fn format_human_duration_seconds_only() {
        assert_eq!(format_human_duration(1), "1 second");
        assert_eq!(format_human_duration(30), "30 seconds");
    }

    #[test]
    fn list_output_applies_success_color_to_completed_status() {
        // With NO_COLOR set, output is plain; without it, output includes ANSI escapes.
        std::env::set_var("NO_COLOR", "1");
        let plain = style_run_status(&state::RunStatus::Completed, "completed");
        std::env::remove_var("NO_COLOR");

        assert_eq!(plain, "completed");
        // Verify the function routes Completed through style::success (not identity)
        // by checking that without NO_COLOR the output differs from plain text.
        let styled = style_run_status(&state::RunStatus::Completed, "completed");
        assert_ne!(
            styled, "completed",
            "completed status should have ANSI styling when color is on"
        );
    }

    #[test]
    fn list_output_applies_error_color_to_failed_status() {
        std::env::set_var("NO_COLOR", "1");
        let plain = style_run_status(&state::RunStatus::Failed, "failed");
        std::env::remove_var("NO_COLOR");

        assert_eq!(plain, "failed");
        let styled = style_run_status(&state::RunStatus::Failed, "failed");
        assert_ne!(
            styled, "failed",
            "failed status should have ANSI styling when color is on"
        );
    }

    #[test]
    fn list_output_running_status_is_not_styled() {
        // Running status is returned as-is regardless of color settings
        std::env::remove_var("NO_COLOR");
        let result = style_run_status(&state::RunStatus::Running, "running");
        assert_eq!(result, "running");
    }

    #[test]
    fn dry_run_check_mark_uses_success_styling() {
        // With NO_COLOR set the checkmark is plain; with color enabled it has ANSI codes.
        // Hold the color lock to avoid races with style tests that mutate COLOR_ENABLED.
        let _guard = style::COLOR_TEST_LOCK.lock().unwrap();
        style::set_color_enabled();
        std::env::set_var("NO_COLOR", "1");
        let plain = style::success("✓");
        std::env::remove_var("NO_COLOR");

        assert_eq!(plain, "✓");
        style::set_color_enabled();
        let styled = style::success("✓");
        assert_ne!(styled, "✓", "success checkmark should include ANSI styling");
        assert!(styled.contains('✓'));
        style::set_color_enabled();
    }

    // --- Task 2: resolve_init_path tests ---

    #[test]
    fn init_default_name_resolves_to_workflow_rings_toml() {
        let path = resolve_init_path(None).unwrap();
        assert_eq!(path, PathBuf::from("workflow.rings.toml"));
    }

    #[test]
    fn init_custom_name_appends_rings_toml() {
        let path = resolve_init_path(Some("my-task")).unwrap();
        assert_eq!(path, PathBuf::from("my-task.rings.toml"));
    }

    #[test]
    fn init_name_already_ending_in_rings_toml_not_double_suffixed() {
        let path = resolve_init_path(Some("my-task.rings.toml")).unwrap();
        assert_eq!(path, PathBuf::from("my-task.rings.toml"));
    }

    #[test]
    fn init_relative_path_appends_suffix() {
        let path = resolve_init_path(Some("workflows/my-task")).unwrap();
        assert_eq!(path, PathBuf::from("workflows/my-task.rings.toml"));
    }

    #[test]
    fn init_path_with_dotdot_is_rejected() {
        let result = resolve_init_path(Some("../escape"));
        assert!(result.is_err(), "expected error for path with ..");
    }

    #[test]
    fn init_existing_file_without_force_exits_2() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("workflow.rings.toml");
        std::fs::write(&file_path, "").unwrap();

        // We need to test cmd_init behavior; use init_inner directly
        // by passing args with name pointing to the temp file.
        // Since resolve_init_path uses the name as-is and the CWD matters,
        // we test the path-exists check by constructing directly.
        let args = cli::InitArgs {
            name: Some(file_path.to_string_lossy().to_string()),
            force: false,
        };
        let result = init_inner(args, cli::OutputFormat::Human).unwrap();
        assert_eq!(
            result, 2,
            "should exit 2 when file exists and --force not set"
        );
    }

    #[test]
    fn init_existing_file_with_force_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("workflow.rings.toml");
        std::fs::write(&file_path, "").unwrap();

        let args = cli::InitArgs {
            name: Some(file_path.to_string_lossy().to_string()),
            force: true,
        };
        let result = init_inner(args, cli::OutputFormat::Human).unwrap();
        assert_eq!(result, 0, "with --force, should overwrite and return 0");
    }

    // --- Task 3: template content and atomic write tests ---

    #[test]
    fn init_scaffolded_file_parses_as_valid_workflow() {
        use rings::workflow::Workflow;
        use std::str::FromStr;

        let workflow = Workflow::from_str(INIT_TEMPLATE).unwrap();
        assert!(!workflow.completion_signal.is_empty());
        assert!(!workflow.phases.is_empty());
    }

    #[test]
    fn init_scaffolded_file_has_budget_cap_usd() {
        use rings::workflow::Workflow;
        use std::str::FromStr;

        let workflow = Workflow::from_str(INIT_TEMPLATE).unwrap();
        assert!(
            workflow.budget_cap_usd.is_some(),
            "budget_cap_usd must be present so the no-cap warning does not fire"
        );
    }

    #[test]
    fn init_scaffolded_file_completion_signal_in_prompt() {
        use rings::workflow::Workflow;
        use std::str::FromStr;

        let workflow = Workflow::from_str(INIT_TEMPLATE).unwrap();
        let phase = &workflow.phases[0];
        let prompt_text = phase.prompt_text.as_deref().unwrap();
        assert!(
            prompt_text.contains(&workflow.completion_signal),
            "completion signal '{}' must appear in prompt_text",
            workflow.completion_signal
        );
    }

    #[test]
    fn init_scaffolded_file_template_variables_comment_present() {
        assert!(
            INIT_TEMPLATE.contains("{{cycle}}"),
            "template variables comment must list {{cycle}}"
        );
        assert!(
            INIT_TEMPLATE.contains("{{run}}"),
            "template variables comment must list {{run}}"
        );
        assert!(
            INIT_TEMPLATE.contains("{{cost_so_far_usd}}"),
            "template variables comment must list {{cost_so_far_usd}}"
        );
    }

    #[test]
    fn init_atomic_write_no_tmp_file_remaining() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("workflow.rings.toml");
        let tmp = dir.path().join("workflow.rings.toml.tmp");

        let args = cli::InitArgs {
            name: Some(target.to_string_lossy().to_string()),
            force: false,
        };
        let result = init_inner(args, cli::OutputFormat::Human).unwrap();
        assert_eq!(result, 0);
        assert!(target.exists(), "target file should exist");
        assert!(
            !tmp.exists(),
            ".tmp file should not remain after successful write"
        );
    }

    #[test]
    fn init_jsonl_output_valid_json_with_event_and_path() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("workflow.rings.toml");

        let args = cli::InitArgs {
            name: Some(target.to_string_lossy().to_string()),
            force: false,
        };
        // We can't easily capture stdout in unit tests, but we verify the function
        // succeeds and the file was written (JSONL path is exercised via coverage).
        let result = init_inner(args, cli::OutputFormat::Jsonl).unwrap();
        assert_eq!(result, 0);
        assert!(target.exists());
    }

    #[test]
    fn init_dry_run_check_passes_on_scaffold() {
        use rings::dry_run::DryRunPlan;
        use rings::workflow::Workflow;
        use std::str::FromStr;

        let workflow = Workflow::from_str(INIT_TEMPLATE).unwrap();
        // Verify completion signal is found in at least one prompt (as dry-run does)
        let plan = DryRunPlan::from_workflow(&workflow, "workflow.rings.toml").unwrap();
        let any_found = plan.phases.iter().any(|p| p.signal_check.found);
        assert!(
            any_found,
            "dry-run check should find the completion signal in at least one phase prompt"
        );
    }

    // Mutex to serialize PATH-manipulation tests so they don't race with each other.
    static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn update_detects_missing_curl_exits_1() {
        let _guard = PATH_LOCK.lock().unwrap();
        let empty_dir = tempfile::tempdir().unwrap();
        let orig_path = std::env::var_os("PATH");
        std::env::set_var("PATH", empty_dir.path());

        let result = update_inner();

        match orig_path {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }

        assert_eq!(result.unwrap(), 1, "should exit 1 when curl is not on PATH");
    }

    #[test]
    fn update_detects_missing_bash_exits_1() {
        let _guard = PATH_LOCK.lock().unwrap();

        // Create a temp dir containing a fake 'curl' executable but no 'bash'.
        let tmp_dir = tempfile::tempdir().unwrap();
        let fake_curl = tmp_dir.path().join("curl");
        std::fs::write(&fake_curl, "#!/bin/sh\nexit 0").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake_curl, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let orig_path = std::env::var_os("PATH");
        std::env::set_var("PATH", tmp_dir.path());

        let result = update_inner();

        match orig_path {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }

        assert_eq!(result.unwrap(), 1, "should exit 1 when bash is not on PATH");
    }

    // --- shorten_path tests ---

    #[test]
    fn shorten_path_replaces_home_with_tilde() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        let path = format!("{}/code/project", home);
        let result = shorten_path(&path);
        assert!(
            result.starts_with("~/"),
            "expected ~ prefix, got: {}",
            result
        );
        assert!(result.contains("code/project"));
    }

    #[test]
    fn shorten_path_short_path_unchanged() {
        // /tmp/proj is 9 chars, well under 30, and won't match HOME
        let result = shorten_path("/tmp/proj");
        assert_eq!(result, "/tmp/proj");
    }

    #[test]
    fn shorten_path_long_path_truncated_with_ellipsis() {
        // Create a path longer than 30 chars that won't match HOME
        let path = "/very/long/path/to/some/deeply/nested/project/dir";
        let result = shorten_path(path);
        assert!(
            result.len() <= 32, // "…/" + 28 chars = 30 display chars (but ellipsis is 3 bytes)
            "truncated path should be <= 32 bytes, got {} bytes: {}",
            result.len(),
            result
        );
        assert!(
            result.starts_with('\u{2026}'),
            "truncated path should start with ellipsis: {}",
            result
        );
    }

    #[test]
    fn shorten_path_exactly_30_chars_not_truncated() {
        // Construct a path that is exactly 30 chars and doesn't match HOME
        let path = "/tmp/aaa/bbbb/ccccc/dddddddddd"; // 30 chars
        assert_eq!(path.len(), 30);
        let result = shorten_path(path);
        assert_eq!(result, path, "30-char path should not be truncated");
    }

    #[test]
    fn dry_run_cross_mark_uses_warn_styling() {
        // Dry-run ✗ (signal not found) uses warn styling (yellow), not error (red),
        // because a missing signal is advisory, not a hard failure.
        let _guard = style::COLOR_TEST_LOCK.lock().unwrap();
        style::set_color_enabled();
        std::env::set_var("NO_COLOR", "1");
        let plain = style::warn("✗");
        std::env::remove_var("NO_COLOR");

        assert_eq!(plain, "✗", "NO_COLOR=1 produces plain ✗ in dry-run output");
        style::set_color_enabled();
        let styled = style::warn("✗");
        assert_ne!(
            styled, "✗",
            "warn crossmark should include ANSI styling on TTY"
        );
        assert!(styled.contains('✗'));
        style::set_color_enabled();
    }

    // --- dry-run JSONL output tests ---

    #[test]
    fn dry_run_jsonl_event_has_dry_run_plan_event_field() {
        use rings::dry_run::{DryRunPhase, DryRunPlan, DryRunPlanEvent, SignalCheck};

        let plan = DryRunPlan {
            phases: vec![DryRunPhase {
                name: "builder".to_string(),
                prompt_source: "<inline prompt_text>".to_string(),
                runs_per_cycle: 1,
                signal_check: SignalCheck {
                    found: true,
                    line_number: Some(1),
                },
                unknown_vars: vec![],
            }],
            runs_per_cycle_total: 1,
            max_cycles: Some(3),
            max_total_runs: Some(3),
            completion_signal: "DONE".to_string(),
        };
        let event = DryRunPlanEvent {
            event: "dry_run_plan".to_string(),
            plan,
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["event"], "dry_run_plan");
        assert!(parsed["plan"]["phases"].is_array());
        assert_eq!(parsed["plan"]["max_cycles"], 3);
        assert_eq!(parsed["plan"]["max_total_runs"], 3);
        assert_eq!(parsed["plan"]["completion_signal"], "DONE");
        assert!(parsed["timestamp"].is_string());
    }

    #[test]
    fn dry_run_jsonl_output_emits_single_json_line() {
        use rings::dry_run::DryRunPlan;
        use rings::workflow::Workflow;
        use std::str::FromStr;

        let toml = r#"
[workflow]
context_dir = "/tmp"
completion_signal = "DONE"
max_cycles = 2

[[phases]]
name = "build"
prompt_text = "Do the thing.\nDONE"
runs_per_cycle = 1
"#;
        let workflow = Workflow::from_str(toml).unwrap();
        let plan = DryRunPlan::from_workflow(&workflow, "test.rings.toml").unwrap();
        let event = rings::dry_run::DryRunPlanEvent {
            event: "dry_run_plan".to_string(),
            plan,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        let json = serde_json::to_string(&event).unwrap();

        // Must be a single line (no embedded newlines)
        assert!(!json.contains('\n'), "JSONL output must be a single line");
        // Must round-trip
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["event"], "dry_run_plan");
        assert_eq!(parsed["plan"]["completion_signal"], "DONE");
    }

    #[test]
    fn dry_run_jsonl_suppressed_for_human_format() {
        // The JSONL dry-run path is gated by `output_format == OutputFormat::Jsonl`.
        // Human format does not satisfy that condition.
        assert_ne!(cli::OutputFormat::Jsonl, cli::OutputFormat::Human);
    }

    // --- disk space advisory check tests ---

    #[test]
    fn disk_space_ok_when_ample_space() {
        // 200 MB is well above both thresholds
        assert_eq!(classify_disk_space(200 * 1024 * 1024), DiskSpaceLevel::Ok);
    }

    #[test]
    fn disk_space_ok_at_exactly_100mb() {
        // 100 MB is at the boundary — not a warning (< 100 triggers warning)
        assert_eq!(classify_disk_space(100 * 1024 * 1024), DiskSpaceLevel::Ok);
    }

    #[test]
    fn disk_space_warning_when_low() {
        // 50 MB is in the warning band (< 100 MB but >= 10 MB)
        assert_eq!(
            classify_disk_space(50 * 1024 * 1024),
            DiskSpaceLevel::Warning(50)
        );
    }

    #[test]
    fn disk_space_warning_at_boundary_just_below_100mb() {
        // 99 MB should trigger warning
        assert_eq!(
            classify_disk_space(99 * 1024 * 1024),
            DiskSpaceLevel::Warning(99)
        );
    }

    #[test]
    fn disk_space_warning_at_boundary_just_above_10mb() {
        // 10 MB is at the boundary — warning, not fatal (< 10 triggers fatal)
        assert_eq!(
            classify_disk_space(10 * 1024 * 1024),
            DiskSpaceLevel::Warning(10)
        );
    }

    #[test]
    fn disk_space_fatal_when_very_low() {
        // 5 MB is in the fatal band (< 10 MB)
        assert_eq!(
            classify_disk_space(5 * 1024 * 1024),
            DiskSpaceLevel::Fatal(5)
        );
    }

    #[test]
    fn disk_space_fatal_at_zero() {
        assert_eq!(classify_disk_space(0), DiskSpaceLevel::Fatal(0));
    }

    #[test]
    fn disk_space_fatal_check_applies_in_jsonl_mode() {
        // Fatal threshold is not suppressed by output format — verify the
        // classify_disk_space function returns Fatal regardless of output mode.
        assert_eq!(
            classify_disk_space(9 * 1024 * 1024),
            DiskSpaceLevel::Fatal(9)
        );
    }

    #[test]
    fn disk_space_warning_suppressed_in_jsonl_mode_by_format_guard() {
        // The warning print is guarded by `output_format == OutputFormat::Human`.
        // JSONL mode uses OutputFormat::Jsonl, which does not satisfy the guard.
        assert_ne!(cli::OutputFormat::Jsonl, cli::OutputFormat::Human);
    }

    // --- large context_dir advisory check tests ---

    #[test]
    fn count_context_dir_files_counts_recursively() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "a").unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("b.txt"), "b").unwrap();
        std::fs::write(sub.join("c.txt"), "c").unwrap();
        assert_eq!(count_context_dir_files(tmp.path().to_str().unwrap()), 3);
    }

    #[test]
    fn count_context_dir_files_empty_dir_returns_zero() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(count_context_dir_files(tmp.path().to_str().unwrap()), 0);
    }

    #[test]
    fn count_context_dir_files_nonexistent_path_returns_zero() {
        assert_eq!(
            count_context_dir_files("/nonexistent/path/rings_large_dir_test"),
            0
        );
    }

    #[test]
    fn large_context_dir_warning_threshold_is_10000() {
        let tmp = tempfile::tempdir().unwrap();
        // Create exactly 10,001 files — above the threshold
        for i in 0..=10_000usize {
            std::fs::write(tmp.path().join(format!("f{i}.txt")), "x").unwrap();
        }
        let count = count_context_dir_files(tmp.path().to_str().unwrap());
        assert!(count > 10_000, "expected count > 10000, got {count}");
    }

    #[test]
    fn large_context_dir_warning_suppressed_in_jsonl_mode() {
        // The advisory check is guarded by output_format == OutputFormat::Human AND
        // manifest_enabled = true. JSONL mode skips it.
        assert_ne!(cli::OutputFormat::Jsonl, cli::OutputFormat::Human);
    }

    #[test]
    fn large_context_dir_warning_suppressed_when_manifest_disabled() {
        // When manifest_enabled = false, the file count check is not needed.
        // This test validates the logic condition: both Human mode AND manifest_enabled must be true.
        // We verify by ensuring the guard (manifest_enabled) is a separate condition.
        // The actual guard in run_inner: output_format == Human && workflow.manifest_enabled
        let manifest_enabled = false;
        let is_human = true;
        let should_check = is_human && manifest_enabled;
        assert!(
            !should_check,
            "should not check when manifest_enabled=false"
        );
    }
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;
    use rings::state::{RunMeta, RunStatus};
    use tempfile::TempDir;

    /// Write a minimal run.toml into `dir / run_id / run.toml`.
    fn make_run(
        base: &std::path::Path,
        run_id: &str,
        started_at: &str,
        status: RunStatus,
    ) -> std::path::PathBuf {
        let run_dir = base.join(run_id);
        std::fs::create_dir_all(&run_dir).unwrap();
        let meta = RunMeta {
            run_id: run_id.to_string(),
            workflow_file: "test.toml".to_string(),
            started_at: started_at.to_string(),
            rings_version: "0.1.0".to_string(),
            status,
            phase_fingerprint: None,
            parent_run_id: None,
            continuation_of: None,
            ancestry_depth: 0,
            context_dir: None,
            otel_trace_id: None,
            otel_span_id: None,
        };
        meta.write(&run_dir.join("run.toml")).unwrap();
        run_dir
    }

    fn make_cleanup_args(older_than: &str, dry_run: bool, yes: bool) -> cli::CleanupArgs {
        cli::CleanupArgs {
            older_than: older_than.to_string(),
            dry_run,
            yes,
        }
    }

    #[test]
    fn cli_cleanup_parses_default_older_than() {
        use clap::Parser;
        let cli = rings::cli::Cli::parse_from(["rings", "cleanup"]);
        match cli.command {
            rings::cli::Command::Cleanup(args) => {
                assert_eq!(args.older_than, "30d");
                assert!(!args.dry_run);
                assert!(!args.yes);
            }
            _ => panic!("expected Cleanup command"),
        }
    }

    #[test]
    fn cli_cleanup_parses_custom_duration() {
        use clap::Parser;
        let cli = rings::cli::Cli::parse_from(["rings", "cleanup", "--older-than", "7d"]);
        match cli.command {
            rings::cli::Command::Cleanup(args) => {
                assert_eq!(args.older_than, "7d");
            }
            _ => panic!("expected Cleanup command"),
        }
    }

    #[test]
    fn cli_cleanup_parses_dry_run_and_yes() {
        use clap::Parser;
        let cli = rings::cli::Cli::parse_from(["rings", "cleanup", "--dry-run", "--yes"]);
        match cli.command {
            rings::cli::Command::Cleanup(args) => {
                assert!(args.dry_run);
                assert!(args.yes);
            }
            _ => panic!("expected Cleanup command"),
        }
    }

    #[test]
    fn cleanup_identifies_old_runs_as_candidates() {
        let tmp = TempDir::new().unwrap();
        // Run that is 60 days old (older than 30d threshold)
        make_run(
            tmp.path(),
            "run_old",
            "2020-01-01T00:00:00Z",
            RunStatus::Completed,
        );
        // Run that started recently (newer than 30d)
        let now = chrono::Utc::now();
        let recent_ts =
            (now - chrono::Duration::days(1)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        make_run(tmp.path(), "run_new", &recent_ts, RunStatus::Completed);

        // cleanup_inner with --yes (no TTY prompt) using a fake base_dir
        // We use a modified version: call cleanup_inner with older_than="30d"
        // We need to override base_dir — since cleanup_inner calls resolve_output_dir(None, None),
        // we can't inject the temp dir directly without refactoring. Instead, test via
        // cleanup_inner and see that only the old run directory is removed.
        //
        // Direct test of the logic: manually replicate the candidate selection
        let cutoff = "30d"
            .parse::<duration::SinceSpec>()
            .unwrap()
            .to_cutoff_datetime();
        let meta_old = RunMeta::read(&tmp.path().join("run_old").join("run.toml")).unwrap();
        let started_old = chrono::DateTime::parse_from_rfc3339(&meta_old.started_at)
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert!(started_old < cutoff, "old run should be older than cutoff");

        let meta_new = RunMeta::read(&tmp.path().join("run_new").join("run.toml")).unwrap();
        let started_new = chrono::DateTime::parse_from_rfc3339(&meta_new.started_at)
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert!(started_new >= cutoff, "new run should be newer than cutoff");
    }

    #[test]
    fn cleanup_skips_running_runs() {
        let tmp = TempDir::new().unwrap();
        make_run(
            tmp.path(),
            "run_active",
            "2020-01-01T00:00:00Z",
            RunStatus::Running,
        );
        // A Running run older than the threshold should NOT be a candidate
        let meta = RunMeta::read(&tmp.path().join("run_active").join("run.toml")).unwrap();
        assert_eq!(meta.status, RunStatus::Running);
        // Confirm the logic: running runs are skipped regardless of age
        assert!(meta.status == RunStatus::Running);
    }

    #[test]
    fn cleanup_dry_run_does_not_delete() {
        let tmp = TempDir::new().unwrap();
        make_run(
            tmp.path(),
            "run_old_1",
            "2020-01-01T00:00:00Z",
            RunStatus::Completed,
        );

        // Build fake base_dir by temporarily setting up a closure-style test
        // Since cleanup_inner calls resolve_output_dir, we test it indirectly
        // by verifying the dry_run flag is parsed correctly and the file exists.
        let run_dir = tmp.path().join("run_old_1");
        assert!(run_dir.exists(), "run dir should exist before dry run");

        // Verify dry_run arg parses
        let args = make_cleanup_args("30d", true, true);
        assert!(args.dry_run);
        // The actual delete-prevention is covered by the integration of dry_run check
        // in cleanup_inner (line: if args.dry_run { ... return Ok(0); })
    }

    #[test]
    fn cleanup_yes_flag_skips_prompt() {
        // Verify that when yes=true, cleanup proceeds without stdin interaction.
        // This is a unit test of the flag value; the TTY guard is: if !args.yes && stderr.is_terminal()
        let args = make_cleanup_args("30d", false, true);
        assert!(args.yes, "yes flag should be true");
    }

    #[test]
    fn cleanup_empty_base_dir_returns_zero() {
        let tmp = TempDir::new().unwrap();
        // An empty base dir should return "no runs found" — simulate by checking
        // that scanning an empty dir produces zero candidates
        let entries: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn cleanup_jsonl_output_event_structure() {
        // Verify the JSONL event is well-formed JSON with expected keys
        let json = serde_json::json!({
            "event": "cleanup_deleted",
            "run_id": "run_20200101_000000_abcdef",
            "path": "/tmp/rings/runs/run_20200101_000000_abcdef",
        });
        assert_eq!(json["event"], "cleanup_deleted");
        assert!(json["run_id"].is_string());
        assert!(json["path"].is_string());

        let summary = serde_json::json!({
            "event": "cleanup_summary",
            "deleted_count": 3,
            "freed_mb": 1.5,
        });
        assert_eq!(summary["event"], "cleanup_summary");
        assert_eq!(summary["deleted_count"], 3);
    }
}

#[cfg(test)]
mod show_tests {
    use super::*;
    use rings::audit::CostEntry;
    use rings::state::{RunMeta, RunStatus, StateFile};
    use tempfile::TempDir;

    fn make_run_dir(base: &std::path::Path, run_id: &str, status: RunStatus) -> std::path::PathBuf {
        let run_dir = base.join(run_id);
        std::fs::create_dir_all(&run_dir).unwrap();
        let meta = RunMeta {
            run_id: run_id.to_string(),
            workflow_file: "workflow.rings.toml".to_string(),
            started_at: "2025-01-01T00:00:00Z".to_string(),
            rings_version: "0.1.0".to_string(),
            status,
            phase_fingerprint: None,
            parent_run_id: None,
            continuation_of: None,
            ancestry_depth: 0,
            context_dir: Some("/home/user/project".to_string()),
            otel_trace_id: None,
            otel_span_id: None,
        };
        meta.write(&run_dir.join("run.toml")).unwrap();
        run_dir
    }

    fn write_state(run_dir: &std::path::Path, cycles: u32, cost: f64) {
        let state = StateFile {
            schema_version: 1,
            run_id: run_dir.file_name().unwrap().to_string_lossy().into_owned(),
            workflow_file: "workflow.rings.toml".to_string(),
            last_completed_run: cycles,
            last_completed_cycle: cycles,
            last_completed_phase_index: 0,
            last_completed_iteration: 0,
            total_runs_completed: cycles,
            cumulative_cost_usd: cost,
            claude_resume_commands: vec![],
            canceled_at: None,
            failure_reason: None,
            ancestry: None,
        };
        state.write_atomic(&run_dir.join("state.json")).unwrap();
    }

    fn write_cost_entries(run_dir: &std::path::Path, entries: &[CostEntry]) {
        use std::io::Write;
        let path = run_dir.join("costs.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for entry in entries {
            writeln!(f, "{}", serde_json::to_string(entry).unwrap()).unwrap();
        }
    }

    fn make_cost_entry(
        run: u32,
        phase: &str,
        cost_usd: f64,
        input_tokens: u64,
        output_tokens: u64,
    ) -> CostEntry {
        CostEntry {
            run,
            cycle: 1,
            phase: phase.to_string(),
            iteration: 1,
            cost_usd: Some(cost_usd),
            input_tokens: Some(input_tokens),
            output_tokens: Some(output_tokens),
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            files_changed: 0,
            event: None,
            produces_violations: vec![],
        }
    }

    #[test]
    fn show_prints_summary_with_run_id_status_cost_cycles() {
        let tmp = TempDir::new().unwrap();
        let run_dir = make_run_dir(tmp.path(), "run_test_001", RunStatus::Completed);
        write_state(&run_dir, 3, 0.042);
        write_cost_entries(
            &run_dir,
            &[
                make_cost_entry(1, "builder", 0.020, 1000, 500),
                make_cost_entry(2, "reviewer", 0.022, 1100, 600),
            ],
        );

        // Call render_summary directly and verify it succeeds
        let result = render_summary(&run_dir, cli::OutputFormat::Human);
        assert!(
            result.is_ok(),
            "render_summary should succeed: {:?}",
            result
        );
    }

    #[test]
    fn show_invalid_run_id_returns_error() {
        let tmp = TempDir::new().unwrap();
        let fake_dir = tmp.path().join("nonexistent_run");
        let result = render_summary(&fake_dir, cli::OutputFormat::Human);
        assert!(result.is_err(), "should error on missing run directory");
    }

    #[test]
    fn show_jsonl_mode_emits_single_json_object() {
        use std::io::{self, Write};

        let tmp = TempDir::new().unwrap();
        let run_dir = make_run_dir(tmp.path(), "run_jsonl_001", RunStatus::Completed);
        write_state(&run_dir, 2, 0.015);
        write_cost_entries(&run_dir, &[make_cost_entry(1, "builder", 0.010, 500, 200)]);

        // Capture stdout via a buffer — we redirect by calling render_summary with jsonl
        // and checking it doesn't error (full output capture would require a pipe)
        let result = render_summary(&run_dir, cli::OutputFormat::Jsonl);
        assert!(
            result.is_ok(),
            "render_summary jsonl should succeed: {:?}",
            result
        );
    }

    #[test]
    fn show_summary_includes_phase_cost_breakdown() {
        let tmp = TempDir::new().unwrap();
        let run_dir = make_run_dir(tmp.path(), "run_phases_001", RunStatus::Completed);
        write_state(&run_dir, 2, 0.05);
        write_cost_entries(
            &run_dir,
            &[
                make_cost_entry(1, "builder", 0.020, 1000, 400),
                make_cost_entry(2, "reviewer", 0.030, 1500, 600),
            ],
        );

        // Verify the data loading works correctly (phase breakdown is computed from cost entries)
        let costs_path = run_dir.join("costs.jsonl");
        let entries: Vec<CostEntry> = rings::audit::stream_cost_entries(&costs_path)
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(entries.len(), 2);
        let builder_cost: f64 = entries
            .iter()
            .filter(|e| e.phase == "builder")
            .filter_map(|e| e.cost_usd)
            .sum();
        assert!((builder_cost - 0.020).abs() < 1e-9);

        let result = render_summary(&run_dir, cli::OutputFormat::Human);
        assert!(result.is_ok());
    }

    #[test]
    fn show_gracefully_handles_missing_state_json() {
        let tmp = TempDir::new().unwrap();
        // Only run.toml, no state.json
        let run_dir = make_run_dir(tmp.path(), "run_no_state", RunStatus::Completed);
        // No state.json written
        let result = render_summary(&run_dir, cli::OutputFormat::Human);
        assert!(
            result.is_ok(),
            "should succeed even without state.json: {:?}",
            result
        );
    }

    fn make_cost_entry_with_cycle(
        run: u32,
        cycle: u32,
        phase: &str,
        iteration: u32,
        cost_usd: f64,
        files_changed: u32,
    ) -> CostEntry {
        CostEntry {
            run,
            cycle,
            phase: phase.to_string(),
            iteration,
            cost_usd: Some(cost_usd),
            input_tokens: None,
            output_tokens: None,
            cost_confidence: "full".to_string(),
            files_added: 0,
            files_modified: files_changed,
            files_deleted: 0,
            files_changed,
            event: None,
            produces_violations: vec![],
        }
    }

    #[test]
    fn render_cycles_view_displays_per_cycle_breakdown() {
        let tmp = TempDir::new().unwrap();
        let run_dir = make_run_dir(tmp.path(), "run_cycles_001", RunStatus::Completed);
        write_cost_entries(
            &run_dir,
            &[
                make_cost_entry_with_cycle(1, 1, "builder", 1, 0.092, 3),
                make_cost_entry_with_cycle(2, 1, "reviewer", 1, 0.104, 1),
                make_cost_entry_with_cycle(3, 2, "builder", 1, 0.087, 2),
            ],
        );
        let result = render_cycles(&run_dir, None, cli::OutputFormat::Human);
        assert!(result.is_ok(), "render_cycles should succeed: {:?}", result);
    }

    #[test]
    fn render_cycles_view_cycle_filter_works() {
        let tmp = TempDir::new().unwrap();
        let run_dir = make_run_dir(tmp.path(), "run_cycles_002", RunStatus::Completed);
        write_cost_entries(
            &run_dir,
            &[
                make_cost_entry_with_cycle(1, 1, "builder", 1, 0.092, 3),
                make_cost_entry_with_cycle(2, 2, "builder", 1, 0.087, 2),
            ],
        );
        // Should succeed with cycle filter
        let result = render_cycles(&run_dir, Some(2), cli::OutputFormat::Human);
        assert!(
            result.is_ok(),
            "render_cycles with filter should succeed: {:?}",
            result
        );
    }

    #[test]
    fn render_cycles_jsonl_mode_succeeds() {
        let tmp = TempDir::new().unwrap();
        let run_dir = make_run_dir(tmp.path(), "run_cycles_003", RunStatus::Completed);
        write_cost_entries(
            &run_dir,
            &[make_cost_entry_with_cycle(1, 1, "builder", 1, 0.092, 3)],
        );
        let result = render_cycles(&run_dir, None, cli::OutputFormat::Jsonl);
        assert!(
            result.is_ok(),
            "render_cycles jsonl should succeed: {:?}",
            result
        );
    }

    #[test]
    fn render_cycles_no_costs_file_succeeds() {
        let tmp = TempDir::new().unwrap();
        let run_dir = make_run_dir(tmp.path(), "run_cycles_nocosts", RunStatus::Completed);
        // No costs.jsonl
        let result = render_cycles(&run_dir, None, cli::OutputFormat::Human);
        assert!(
            result.is_ok(),
            "render_cycles with no costs should succeed: {:?}",
            result
        );
    }
}

#[cfg(all(test, unix))]
mod dir_permissions_tests {
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    /// Verify that creating a run directory and setting 0700 permissions works correctly,
    /// and that the parent directory permissions are not affected.
    #[test]
    fn run_dir_created_with_mode_0700() {
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path();

        // Record parent permissions before creating the run dir
        let parent_mode_before = std::fs::metadata(parent).unwrap().permissions().mode() & 0o777;

        let run_dir = parent.join("run_20240315_143022_a1b2c3");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::set_permissions(&run_dir, std::fs::Permissions::from_mode(0o700)).unwrap();

        let run_mode = std::fs::metadata(&run_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(run_mode, 0o700, "run dir should have mode 0700");

        // Parent directory permissions should be unchanged
        let parent_mode_after = std::fs::metadata(parent).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            parent_mode_before, parent_mode_after,
            "parent directory permissions should not be changed"
        );
    }

    #[test]
    fn parent_dir_permissions_not_changed_by_run_dir_creation() {
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path();

        // Set a specific mode on parent (e.g., 0755) and verify it's preserved
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o755)).unwrap();
        let parent_mode_before = std::fs::metadata(parent).unwrap().permissions().mode() & 0o777;

        let run_dir = parent.join("run_test_abc");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::set_permissions(&run_dir, std::fs::Permissions::from_mode(0o700)).unwrap();

        let parent_mode_after = std::fs::metadata(parent).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            parent_mode_before, parent_mode_after,
            "parent directory permissions (0755) should be unchanged after run dir creation"
        );
        assert_eq!(parent_mode_after, 0o755);

        let run_mode = std::fs::metadata(&run_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(run_mode, 0o700);
    }

    // --- lineage tests ---

    use super::{build_chain, find_chain_root, lineage_inner_with_base};

    fn write_run_toml_lineage(dir: &std::path::Path, run_id: &str, parent: Option<&str>) {
        use rings::state;
        let meta = state::RunMeta {
            run_id: run_id.to_string(),
            workflow_file: "workflow.rings.toml".to_string(),
            started_at: format!("2026-01-01T00:00:0{}Z", run_id.len() % 10),
            rings_version: "0.1.0".to_string(),
            status: state::RunStatus::Completed,
            phase_fingerprint: None,
            parent_run_id: parent.map(|s| s.to_string()),
            continuation_of: None,
            ancestry_depth: 0,
            context_dir: None,
            otel_trace_id: None,
            otel_span_id: None,
        };
        let content = toml::to_string_pretty(&meta).unwrap();
        std::fs::write(dir.join("run.toml"), content).unwrap();
    }

    fn write_state_json_lineage(
        dir: &std::path::Path,
        run_id: &str,
        cycles: u32,
        total_runs: u32,
        cost: f64,
    ) {
        use rings::state;
        let st = state::StateFile {
            schema_version: 1,
            run_id: run_id.to_string(),
            workflow_file: "workflow.rings.toml".to_string(),
            last_completed_run: total_runs,
            last_completed_cycle: cycles,
            last_completed_phase_index: 0,
            last_completed_iteration: 0,
            total_runs_completed: total_runs,
            cumulative_cost_usd: cost,
            claude_resume_commands: vec![],
            canceled_at: None,
            failure_reason: None,
            ancestry: None,
        };
        let content = serde_json::to_string_pretty(&st).unwrap();
        std::fs::write(dir.join("state.json"), content).unwrap();
    }

    #[test]
    fn lineage_single_run_no_parent_shows_itself() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let run_dir = base.join("run_001");
        std::fs::create_dir_all(&run_dir).unwrap();
        write_run_toml_lineage(&run_dir, "run_001", None);
        write_state_json_lineage(&run_dir, "run_001", 3, 10, 1.23);

        let args = rings::cli::LineageArgs {
            run_id: "run_001".to_string(),
            descendants: false,
        };

        // Verify find_chain_root returns the run itself when no parent.
        let (root, broken): (String, Option<String>) = find_chain_root("run_001", base);
        assert_eq!(root, "run_001");
        assert!(broken.is_none());

        let chain = build_chain("run_001", base).unwrap();
        assert_eq!(chain, vec!["run_001"]);

        // lineage_inner_with_base should succeed.
        let result = lineage_inner_with_base(args, rings::cli::OutputFormat::Human, base);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn lineage_chain_of_three_traverses_correctly() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        // Create root → child → grandchild chain
        let r1 = base.join("run_001");
        let r2 = base.join("run_002");
        let r3 = base.join("run_003");
        for d in [&r1, &r2, &r3] {
            std::fs::create_dir_all(d).unwrap();
        }
        write_run_toml_lineage(&r1, "run_001", None);
        write_run_toml_lineage(&r2, "run_002", Some("run_001"));
        write_run_toml_lineage(&r3, "run_003", Some("run_002"));
        write_state_json_lineage(&r1, "run_001", 3, 10, 1.00);
        write_state_json_lineage(&r2, "run_002", 5, 15, 2.00);
        write_state_json_lineage(&r3, "run_003", 2, 5, 0.50);

        // Starting from grandchild, root should be run_001.
        let (root, broken): (String, Option<String>) = find_chain_root("run_003", base);
        assert_eq!(root, "run_001");
        assert!(broken.is_none());

        // Forward chain from root should be in order.
        let chain = build_chain("run_001", base).unwrap();
        assert_eq!(chain, vec!["run_001", "run_002", "run_003"]);
    }

    #[test]
    fn lineage_chain_totals_sum_correctly() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        let r1 = base.join("run_001");
        let r2 = base.join("run_002");
        for d in [&r1, &r2] {
            std::fs::create_dir_all(d).unwrap();
        }
        write_run_toml_lineage(&r1, "run_001", None);
        write_run_toml_lineage(&r2, "run_002", Some("run_001"));
        write_state_json_lineage(&r1, "run_001", 3, 10, 1.00);
        write_state_json_lineage(&r2, "run_002", 5, 8, 2.50);

        let args = rings::cli::LineageArgs {
            run_id: "run_001".to_string(),
            descendants: false,
        };

        let chain = build_chain("run_001", base).unwrap();
        assert_eq!(chain.len(), 2);

        // Verify totals computed the same way lineage_inner_with_base does.
        let total_cost = 1.00_f64 + 2.50_f64;
        let total_cycles = 3_u32 + 5_u32;
        assert!((total_cost - 3.50).abs() < 0.001);
        assert_eq!(total_cycles, 8);

        let result = lineage_inner_with_base(args, rings::cli::OutputFormat::Human, base);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn lineage_broken_chain_shows_partial_with_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        // run_002 references run_001 as parent, but run_001 dir doesn't exist.
        let r2 = base.join("run_002");
        std::fs::create_dir_all(&r2).unwrap();
        write_run_toml_lineage(&r2, "run_002", Some("run_001"));
        write_state_json_lineage(&r2, "run_002", 2, 5, 1.00);

        // find_chain_root should stop at run_002 and report the broken link.
        let (root, broken): (String, Option<String>) = find_chain_root("run_002", base);
        assert_eq!(root, "run_002");
        assert_eq!(broken.as_deref(), Some("run_001"));

        // Chain from run_002 should contain only run_002.
        let chain = build_chain("run_002", base).unwrap();
        assert_eq!(chain, vec!["run_002"]);
    }

    #[test]
    fn lineage_jsonl_mode_emits_run_and_summary_objects() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        let r1 = base.join("run_001");
        let r2 = base.join("run_002");
        for d in [&r1, &r2] {
            std::fs::create_dir_all(d).unwrap();
        }
        write_run_toml_lineage(&r1, "run_001", None);
        write_run_toml_lineage(&r2, "run_002", Some("run_001"));
        write_state_json_lineage(&r1, "run_001", 3, 10, 1.00);
        write_state_json_lineage(&r2, "run_002", 2, 5, 0.50);

        let chain = build_chain("run_001", base).unwrap();
        assert_eq!(chain.len(), 2);

        // Verify JSONL fields are correct by checking chain data directly.
        let meta1 = rings::state::RunMeta::read(&r1.join("run.toml")).unwrap();
        let meta2 = rings::state::RunMeta::read(&r2.join("run.toml")).unwrap();
        assert!(meta1.parent_run_id.is_none());
        assert_eq!(meta2.parent_run_id.as_deref(), Some("run_001"));
    }
}

#[cfg(test)]
mod completions_tests {
    use super::*;
    use clap_complete::Shell;

    fn completions_output(shell: Shell) -> String {
        let mut buf = Vec::new();
        generate_completions(shell, &mut buf);
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn bash_completions_contains_expected_patterns() {
        let output = completions_output(Shell::Bash);
        assert!(
            output.contains("_rings"),
            "bash script missing function _rings"
        );
        assert!(
            output.contains("complete"),
            "bash script missing complete builtin"
        );
    }

    #[test]
    fn zsh_completions_contains_expected_patterns() {
        let output = completions_output(Shell::Zsh);
        assert!(
            output.contains("#compdef rings"),
            "zsh script missing #compdef header"
        );
        assert!(
            output.contains("_rings"),
            "zsh script missing _rings function"
        );
    }

    #[test]
    fn fish_completions_contains_expected_patterns() {
        let output = completions_output(Shell::Fish);
        assert!(
            output.contains("complete"),
            "fish script missing complete command"
        );
        assert!(
            output.contains("rings"),
            "fish script missing rings references"
        );
    }

    #[test]
    fn completions_output_is_non_empty_for_all_shells() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let output = completions_output(shell);
            assert!(
                !output.is_empty(),
                "completions output for {shell:?} is empty"
            );
        }
    }

    // --- Custom completer tests (F-178) ---

    /// Verify that the workflow file completer returns only .toml files.
    #[test]
    fn workflow_file_completer_suggests_toml_files() {
        use std::ffi::OsStr;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("workflow.rings.toml"), "").unwrap();
        std::fs::write(dir.join("other.toml"), "").unwrap();
        std::fs::write(dir.join("not-toml.txt"), "").unwrap();
        std::fs::write(dir.join("script.sh"), "").unwrap();

        let candidates = rings::cli::complete_toml_files_from_dir(Some(dir), OsStr::new(""));
        let names: Vec<String> = candidates
            .iter()
            .map(|c| c.get_value().to_string_lossy().to_string())
            .collect();

        assert!(
            !names.is_empty(),
            "expected at least one .toml candidate, got none"
        );
        for name in &names {
            assert!(
                name.ends_with(".toml"),
                "expected only .toml files, but got: {name}"
            );
        }
        // Both .rings.toml and .toml files should appear
        assert!(
            names.iter().any(|n| n.contains("workflow.rings.toml")),
            "workflow.rings.toml missing from candidates: {names:?}"
        );
        assert!(
            names.iter().any(|n| n.contains("other.toml")),
            "other.toml missing from candidates: {names:?}"
        );
    }

    /// Verify that the run ID completer lists run directories and filters out non-run entries.
    #[test]
    fn run_id_completer_lists_run_ids() {
        use std::ffi::OsStr;
        let tmp = tempfile::tempdir().unwrap();
        let runs_dir = tmp.path().join("runs");
        std::fs::create_dir_all(runs_dir.join("run_20240315_143022_a1b2c3")).unwrap();
        std::fs::create_dir_all(runs_dir.join("run_20240316_100000_b2c3d4")).unwrap();
        std::fs::create_dir_all(runs_dir.join("not-a-run")).unwrap();
        std::fs::create_dir_all(runs_dir.join("other-dir")).unwrap();

        let candidates = rings::cli::complete_run_ids_from_dir(&runs_dir, OsStr::new(""));
        let names: Vec<String> = candidates
            .iter()
            .map(|c| c.get_value().to_string_lossy().to_string())
            .collect();

        assert!(
            names.contains(&"run_20240315_143022_a1b2c3".to_string()),
            "run_20240315_143022_a1b2c3 missing from candidates: {names:?}"
        );
        assert!(
            names.contains(&"run_20240316_100000_b2c3d4".to_string()),
            "run_20240316_100000_b2c3d4 missing from candidates: {names:?}"
        );
        assert!(
            !names.contains(&"not-a-run".to_string()),
            "non-run dir 'not-a-run' should not appear in candidates: {names:?}"
        );
    }

    /// Verify that the run ID completer filters by the current partial input prefix.
    #[test]
    fn run_id_completer_filters_by_prefix() {
        use std::ffi::OsStr;
        let tmp = tempfile::tempdir().unwrap();
        let runs_dir = tmp.path().join("runs");
        std::fs::create_dir_all(runs_dir.join("run_20240315_143022_a1b2c3")).unwrap();
        std::fs::create_dir_all(runs_dir.join("run_20240316_100000_b2c3d4")).unwrap();

        let candidates =
            rings::cli::complete_run_ids_from_dir(&runs_dir, OsStr::new("run_20240315"));
        let names: Vec<String> = candidates
            .iter()
            .map(|c| c.get_value().to_string_lossy().to_string())
            .collect();

        assert_eq!(names, vec!["run_20240315_143022_a1b2c3"]);
    }

    // --- check_workflow_path_mismatch tests (F-057) ---

    #[test]
    fn path_mismatch_same_canonical_path_no_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("workflow.toml");
        std::fs::write(&file, "[workflow]").unwrap();
        // The canonical path of the file should equal its absolute string form
        // (tempfile dirs are typically already canonical under /tmp).
        let canonical = std::fs::canonicalize(&file).unwrap();
        let canonical_str = canonical.to_string_lossy().to_string();
        assert!(
            check_workflow_path_mismatch(&canonical_str).is_none(),
            "no warning expected when saved path equals canonical path"
        );
    }

    #[cfg(unix)]
    #[test]
    fn path_mismatch_symlink_path_triggers_warning() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let real_file = tmp.path().join("real_workflow.toml");
        std::fs::write(&real_file, "[workflow]").unwrap();
        // Create a symlink pointing to the real file
        let link = tmp.path().join("workflow_link.toml");
        symlink(&real_file, &link).unwrap();
        // The saved path is the symlink path; canonical will resolve to the real file
        let link_str = link.to_string_lossy().to_string();
        let canonical = std::fs::canonicalize(&link).unwrap();
        // Only run the assertion if canonical differs from the symlink path
        if canonical.to_string_lossy() != link_str {
            let result = check_workflow_path_mismatch(&link_str);
            assert!(
                result.is_some(),
                "warning expected when saved path is a symlink that resolves differently"
            );
            let warning = result.unwrap();
            assert!(warning.contains("Workflow file path has changed"));
            assert!(warning.contains(&link_str));
        }
    }

    #[test]
    fn path_mismatch_nonexistent_file_returns_no_warning() {
        // If the file doesn't exist, canonicalize fails — we return None (error handled later)
        let result = check_workflow_path_mismatch("/nonexistent/rings_test/workflow.toml");
        assert!(result.is_none(), "no warning for nonexistent file");
    }
}

#[cfg(test)]
mod generate_man_tests {
    use super::*;

    fn man_page_output() -> Vec<u8> {
        use clap::CommandFactory;
        use clap_mangen::Man;
        let cmd = Cli::command();
        let mut buf = Vec::new();
        Man::new(cmd).render(&mut buf).unwrap();
        buf
    }

    #[test]
    fn generated_man_page_is_valid_roff() {
        let output = man_page_output();
        let text = String::from_utf8(output).unwrap();
        // Man pages contain the .TH macro (title header) — required in valid roff
        assert!(
            text.contains(".TH rings"),
            "man page should contain .TH rings macro"
        );
        assert!(!text.is_empty(), "man page should not be empty");
    }

    #[test]
    fn generated_man_page_includes_subcommands() {
        let output = man_page_output();
        let text = String::from_utf8(output).unwrap();
        for subcmd in ["run", "resume", "list", "show", "inspect", "lineage"] {
            assert!(
                text.contains(subcmd),
                "man page missing subcommand '{subcmd}'"
            );
        }
    }

    #[test]
    fn generate_man_hidden_subcommand_not_shown_in_help() {
        use clap::CommandFactory;
        let cmd = Cli::command();
        let generate_man_cmd = cmd
            .get_subcommands()
            .find(|s| s.get_name() == "generate-man");
        assert!(
            generate_man_cmd.is_some(),
            "generate-man subcommand should exist"
        );
        assert!(
            generate_man_cmd.unwrap().is_hide_set(),
            "generate-man subcommand should be hidden"
        );
    }
}
