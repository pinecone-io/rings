use clap::{Args, Parser, Subcommand, ValueEnum};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output (tables, text)
    #[value(name = "human")]
    Human,
    /// JSON Lines format (one JSON object per line)
    #[value(name = "jsonl")]
    Jsonl,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "human" => Ok(OutputFormat::Human),
            "jsonl" => Ok(OutputFormat::Jsonl),
            _ => Err(format!(
                "invalid output format: {}; expected 'human' or 'jsonl'",
                s
            )),
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Human => write!(f, "human"),
            OutputFormat::Jsonl => write!(f, "jsonl"),
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "rings",
    version,
    about = "Orchestrate iterative Claude Code workflows"
)]
pub struct Cli {
    /// Output format
    #[arg(long, global = true, alias = "format", default_value = "human")]
    pub output_format: OutputFormat,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start a new workflow run
    Run(RunArgs),
    /// Resume a canceled or failed run
    Resume(ResumeArgs),
    /// List recent workflow runs
    List(ListArgs),
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Path to the workflow TOML file
    pub workflow_file: String,

    /// Override output directory for this run
    #[arg(long)]
    pub output_dir: Option<String>,

    /// Override max_cycles from workflow file
    #[arg(long)]
    pub max_cycles: Option<u32>,

    /// Override delay_between_runs (seconds)
    #[arg(long)]
    pub delay: Option<u64>,

    /// Stream executor output live to terminal
    #[arg(long, short)]
    pub verbose: bool,

    /// Skip the startup warning if completion signal not found in prompts
    #[arg(long)]
    pub no_completion_check: bool,

    /// Stop execution when cumulative cost reaches this amount (USD)
    #[arg(long, value_name = "DOLLARS")]
    pub budget_cap: Option<f64>,

    /// Per-run timeout (e.g. 30s, 5m, 1h)
    #[arg(long, value_name = "DURATION")]
    pub timeout_per_run: Option<String>,

    /// Override an existing lock on context_dir
    #[arg(long)]
    pub force_lock: bool,

    /// Preview execution plan without running anything
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct ResumeArgs {
    /// Run ID to resume (e.g. run_20240315_143022_a1b2c3)
    pub run_id: String,

    /// Override output directory
    #[arg(long)]
    pub output_dir: Option<String>,

    /// Override max_cycles
    #[arg(long)]
    pub max_cycles: Option<u32>,

    /// Override delay
    #[arg(long)]
    pub delay: Option<u64>,

    /// Stream executor output live to terminal
    #[arg(long, short)]
    pub verbose: bool,

    /// Stop execution when cumulative cost reaches this amount (USD)
    #[arg(long, value_name = "DOLLARS")]
    pub budget_cap: Option<f64>,

    /// Per-run timeout (e.g. 30s, 5m, 1h)
    #[arg(long, value_name = "DURATION")]
    pub timeout_per_run: Option<String>,

    /// Override an existing lock on context_dir
    #[arg(long)]
    pub force_lock: bool,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Show runs modified since this date (YYYY-MM-DD) or duration (e.g., 7d, 2h)
    #[arg(long, value_parser = clap::value_parser!(String))]
    pub since: Option<String>,

    /// Filter by run status (running, completed, canceled, failed, incomplete, stopped)
    #[arg(long)]
    pub status: Option<String>,

    /// Filter by workflow name (substring match)
    #[arg(long)]
    pub workflow: Option<String>,

    /// Maximum number of runs to display
    #[arg(short = 'n', long, default_value = "20")]
    pub limit: usize,
}
