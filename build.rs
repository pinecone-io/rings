use clap::{Arg, ArgAction, Command};
use clap_mangen::Man;
use std::path::PathBuf;

fn build_app() -> Command {
    Command::new("rings")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Orchestrate iterative Claude Code workflows")
        .arg(
            Arg::new("output-format")
                .long("output-format")
                .global(true)
                .help("Output format (human, jsonl) [default: human]"),
        )
        .arg(
            Arg::new("no-color")
                .long("no-color")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Disable color output (also respected via NO_COLOR env var)"),
        )
        .subcommand(
            Command::new("run")
                .about("Start a new workflow run")
                .arg(
                    Arg::new("workflow-file")
                        .required(true)
                        .help("Path to the workflow TOML file"),
                )
                .arg(
                    Arg::new("output-dir")
                        .long("output-dir")
                        .help("Override output directory for this run"),
                )
                .arg(
                    Arg::new("max-cycles")
                        .long("max-cycles")
                        .help("Override max_cycles from workflow file"),
                )
                .arg(
                    Arg::new("delay")
                        .long("delay")
                        .help("Override delay_between_runs (seconds)"),
                )
                .arg(
                    Arg::new("cycle-delay")
                        .long("cycle-delay")
                        .help("Override delay_between_cycles (seconds)"),
                )
                .arg(
                    Arg::new("verbose")
                        .long("verbose")
                        .short('v')
                        .action(ArgAction::SetTrue)
                        .help("Stream executor output live to terminal"),
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue)
                        .help("Preview execution plan without running anything"),
                )
                .arg(
                    Arg::new("budget-cap")
                        .long("budget-cap")
                        .value_name("DOLLARS")
                        .help("Stop execution when cumulative cost reaches this amount (USD)"),
                )
                .arg(
                    Arg::new("timeout-per-run")
                        .long("timeout-per-run")
                        .value_name("DURATION")
                        .help("Per-run timeout (e.g. 30s, 5m, 1h)"),
                )
                .arg(
                    Arg::new("force-lock")
                        .long("force-lock")
                        .action(ArgAction::SetTrue)
                        .help("Override an existing lock on context_dir"),
                )
                .arg(
                    Arg::new("parent-run")
                        .long("parent-run")
                        .help("Parent run ID for ancestry tracking"),
                )
                .arg(
                    Arg::new("quota-backoff")
                        .long("quota-backoff")
                        .action(ArgAction::SetTrue)
                        .help("Enable quota-based automatic retries on executor failure"),
                )
                .arg(
                    Arg::new("no-contract-check")
                        .long("no-contract-check")
                        .action(ArgAction::SetTrue)
                        .help("Skip consumes/produces contract checks"),
                )
                .arg(
                    Arg::new("step")
                        .long("step")
                        .action(ArgAction::SetTrue)
                        .help("Pause after every run for interactive inspection"),
                )
                .arg(
                    Arg::new("step-cycles")
                        .long("step-cycles")
                        .action(ArgAction::SetTrue)
                        .help("Pause only at cycle boundaries"),
                )
                .arg(
                    Arg::new("strict-parsing")
                        .long("strict-parsing")
                        .action(ArgAction::SetTrue)
                        .help("Treat cost parse failures as hard errors"),
                )
                .arg(
                    Arg::new("include-dir")
                        .short('I')
                        .long("include-dir")
                        .value_name("DIR")
                        .action(ArgAction::Append)
                        .help("Prepend a file listing preamble to each prompt"),
                ),
        )
        .subcommand(
            Command::new("resume")
                .about("Resume a canceled or failed run")
                .arg(
                    Arg::new("run-id")
                        .required(true)
                        .help("Run ID to resume (e.g. run_20240315_143022_a1b2c3)"),
                )
                .arg(
                    Arg::new("output-dir")
                        .long("output-dir")
                        .help("Override output directory"),
                )
                .arg(
                    Arg::new("max-cycles")
                        .long("max-cycles")
                        .help("Override max_cycles"),
                )
                .arg(
                    Arg::new("delay")
                        .long("delay")
                        .help("Override delay_between_runs (seconds)"),
                )
                .arg(
                    Arg::new("verbose")
                        .long("verbose")
                        .short('v')
                        .action(ArgAction::SetTrue)
                        .help("Stream executor output live to terminal"),
                )
                .arg(
                    Arg::new("budget-cap")
                        .long("budget-cap")
                        .value_name("DOLLARS")
                        .help("Stop execution when cumulative cost reaches this amount (USD)"),
                )
                .arg(
                    Arg::new("timeout-per-run")
                        .long("timeout-per-run")
                        .value_name("DURATION")
                        .help("Per-run timeout (e.g. 30s, 5m, 1h)"),
                )
                .arg(
                    Arg::new("force-lock")
                        .long("force-lock")
                        .action(ArgAction::SetTrue)
                        .help("Override an existing lock on context_dir"),
                )
                .arg(
                    Arg::new("no-contract-check")
                        .long("no-contract-check")
                        .action(ArgAction::SetTrue)
                        .help("Skip consumes/produces contract checks"),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("List recent workflow runs")
                .arg(
                    Arg::new("since")
                        .long("since")
                        .help("Show runs modified since this date (YYYY-MM-DD) or duration (e.g., 7d, 2h)"),
                )
                .arg(
                    Arg::new("status")
                        .long("status")
                        .help("Filter by run status (running, completed, canceled, failed, incomplete, stopped)"),
                )
                .arg(
                    Arg::new("workflow")
                        .long("workflow")
                        .help("Filter by workflow name (substring match)"),
                )
                .arg(
                    Arg::new("dir")
                        .long("dir")
                        .help("Filter by context directory (substring match)"),
                )
                .arg(
                    Arg::new("limit")
                        .short('n')
                        .long("limit")
                        .default_value("20")
                        .help("Maximum number of runs to display"),
                ),
        )
        .subcommand(
            Command::new("show")
                .about("Show a summary of a completed run (shorthand for inspect --show summary)")
                .arg(
                    Arg::new("run-id")
                        .required(true)
                        .help("Run ID to show (e.g. run_20240315_143022_a1b2c3)"),
                ),
        )
        .subcommand(
            Command::new("inspect")
                .about("Inspect run details with customizable views")
                .arg(
                    Arg::new("run-id")
                        .required(true)
                        .help("Run ID to inspect (e.g. run_20240315_143022_a1b2c3)"),
                )
                .arg(
                    Arg::new("show")
                        .long("show")
                        .action(ArgAction::Append)
                        .help("Views to display: summary, cycles, costs, files-changed, data-flow, claude-output"),
                )
                .arg(
                    Arg::new("cycle")
                        .long("cycle")
                        .help("Filter by specific cycle number"),
                )
                .arg(
                    Arg::new("phase")
                        .long("phase")
                        .help("Filter by specific phase name"),
                ),
        )
        .subcommand(
            Command::new("lineage")
                .about("Show lineage of a run (ancestor/descendant chain)")
                .arg(
                    Arg::new("run-id")
                        .required(true)
                        .help("Run ID to trace ancestry for (e.g. run_20240315_143022_a1b2c3)"),
                )
                .arg(
                    Arg::new("descendants")
                        .long("descendants")
                        .action(ArgAction::SetTrue)
                        .help("Show descendants instead of ancestors"),
                ),
        )
        .subcommand(
            Command::new("init")
                .about("Scaffold a new workflow TOML file")
                .arg(
                    Arg::new("name")
                        .help("Base name for the workflow file (produces <NAME>.rings.toml). Defaults to \"workflow\"."),
                )
                .arg(
                    Arg::new("force")
                        .long("force")
                        .action(ArgAction::SetTrue)
                        .help("Overwrite the target file if it already exists"),
                ),
        )
        .subcommand(
            Command::new("update")
                .about("Update rings to the latest nightly release"),
        )
        .subcommand(
            Command::new("cleanup")
                .about("Remove run data for old runs to free disk space")
                .arg(
                    Arg::new("older-than")
                        .long("older-than")
                        .default_value("30d")
                        .value_name("DURATION")
                        .help("Remove runs older than this duration (e.g. 7d, 30d, 90d, 24h). Default: 30d."),
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue)
                        .help("Show what would be deleted without deleting anything"),
                )
                .arg(
                    Arg::new("yes")
                        .long("yes")
                        .short('y')
                        .action(ArgAction::SetTrue)
                        .help("Skip confirmation prompt (for scripting)"),
                ),
        )
        .subcommand(
            Command::new("completions")
                .about("Generate shell completions")
                .hide(true)
                .arg(
                    Arg::new("shell")
                        .required(true)
                        .help("Shell type (bash, zsh, fish, powershell, elvish)"),
                ),
        )
}

fn main() {
    println!("cargo:rerun-if-changed=src/cli.rs");

    let cmd = build_app();
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let man_dir = manifest_dir.join("target").join("man");

    if let Err(e) = std::fs::create_dir_all(&man_dir) {
        eprintln!("Warning: could not create man directory: {e}");
        return;
    }

    // Generate top-level man page
    let mut buf = Vec::new();
    if Man::new(cmd.clone()).render(&mut buf).is_ok() {
        let _ = std::fs::write(man_dir.join("rings.1"), &buf);
    }

    // Generate subcommand man pages (skip hidden subcommands)
    for subcmd in cmd.get_subcommands() {
        if subcmd.is_hide_set() {
            continue;
        }
        let subname = format!("rings-{}", subcmd.get_name());
        // `Command::name` requires `'static` lifetime; leak is acceptable in a build script.
        let static_name: &'static str = Box::leak(subname.clone().into_boxed_str());
        let mut buf = Vec::new();
        if Man::new(subcmd.clone().name(static_name))
            .render(&mut buf)
            .is_ok()
        {
            let _ = std::fs::write(man_dir.join(format!("{subname}.1")), &buf);
        }
    }
}
