use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::engine::{
    ArgValueCompleter, CompletionCandidate, PathCompleter, ValueCompleter,
};
pub use clap_complete::Shell;
use std::ffi::OsStr;
use std::str::FromStr;

/// Complete `.toml` and `.rings.toml` files relative to the current directory.
pub fn complete_toml_files(current: &OsStr) -> Vec<CompletionCandidate> {
    complete_toml_files_from_dir(None, current)
}

/// Like [`complete_toml_files`] but searches `dir` instead of the process current directory.
/// Used by tests to avoid mutating the process working directory.
pub fn complete_toml_files_from_dir(
    dir: Option<&std::path::Path>,
    current: &OsStr,
) -> Vec<CompletionCandidate> {
    let mut completer = PathCompleter::file().filter(|p| {
        p.extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
    });
    if let Some(d) = dir {
        completer = completer.current_dir(d);
    }
    completer.complete(current)
}

/// Complete run IDs from the default rings output directory.
pub fn complete_run_ids(current: &OsStr) -> Vec<CompletionCandidate> {
    let output_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("rings")
        .join("runs");
    complete_run_ids_from_dir(&output_dir, current)
}

/// Like [`complete_run_ids`] but searches `dir` instead of the default output directory.
/// Used by tests to supply a controlled directory.
pub fn complete_run_ids_from_dir(
    dir: &std::path::Path,
    current: &OsStr,
) -> Vec<CompletionCandidate> {
    let current_str = current.to_str().unwrap_or("");
    let mut candidates = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("run_") && name_str.starts_with(current_str) {
                candidates.push(CompletionCandidate::new(name_str.as_ref()));
            }
        }
    }
    candidates
}

fn validate_run_id(s: &str) -> Result<String, String> {
    if s.starts_with("run_") {
        Ok(s.to_string())
    } else {
        Err(format!(
            "invalid run ID: {}; expected format 'run_<timestamp>_<hash>'",
            s
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
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

    /// Disable color output (also respected via NO_COLOR env var)
    #[arg(long, global = true)]
    pub no_color: bool,

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
    /// Show a summary of a completed run (shorthand for inspect --show summary)
    Show(ShowArgs),
    /// Inspect run details with customizable views
    Inspect(InspectArgs),
    /// Show lineage of a run (ancestor/descendant chain)
    Lineage(LineageArgs),
    /// Generate shell completions
    Completions(CompletionsArgs),
    /// Scaffold a new workflow TOML file
    Init(InitArgs),
    /// Update rings to the latest nightly release
    Update,
    /// Remove run data for old runs to free disk space
    Cleanup(CleanupArgs),
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Path to the workflow TOML file
    #[arg(add = ArgValueCompleter::new(complete_toml_files))]
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

    /// Override delay_between_cycles (seconds)
    #[arg(long)]
    pub cycle_delay: Option<u64>,

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

    /// Parent run ID for ancestry tracking (e.g. run_20240315_143022_a1b2c3)
    #[arg(long, value_parser = validate_run_id)]
    pub parent_run: Option<String>,

    /// Enable quota-based automatic retries on executor failure
    #[arg(long)]
    pub quota_backoff: bool,

    /// Delay in seconds before retrying after a quota error
    #[arg(long, requires = "quota_backoff")]
    pub quota_backoff_delay: Option<u64>,

    /// Maximum number of quota backoff retries
    #[arg(long, requires = "quota_backoff")]
    pub quota_backoff_max_retries: Option<u32>,

    /// Skip consumes/produces contract checks
    #[arg(long)]
    pub no_contract_check: bool,

    /// Pause after every run for interactive inspection (ignored in non-TTY contexts)
    #[arg(long)]
    pub step: bool,

    /// Pause only at cycle boundaries (ignored in non-TTY contexts)
    #[arg(long)]
    pub step_cycles: bool,

    /// Treat cost parse failures as hard errors. When cost parsing confidence is Low or None,
    /// halt execution, save state, and exit with code 2. Default: off.
    #[arg(long)]
    pub strict_parsing: bool,

    /// Skip the startup warning if context_dir contains files matching credential patterns
    #[arg(long)]
    pub no_sensitive_files_check: bool,

    /// Prepend a file listing preamble to each prompt (may be specified multiple times)
    #[arg(short = 'I', long, value_name = "DIR")]
    pub include_dir: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ResumeArgs {
    /// Run ID to resume (e.g. run_20240315_143022_a1b2c3)
    #[arg(add = ArgValueCompleter::new(complete_run_ids))]
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

    /// Skip consumes/produces contract checks
    #[arg(long)]
    pub no_contract_check: bool,
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

    /// Filter by context directory (substring match on stored context_dir)
    #[arg(long)]
    pub dir: Option<String>,

    /// Maximum number of runs to display
    #[arg(short = 'n', long, default_value = "20")]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct ShowArgs {
    /// Run ID to show (e.g. run_20240315_143022_a1b2c3)
    #[arg(add = ArgValueCompleter::new(complete_run_ids))]
    pub run_id: String,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum InspectView {
    /// Summary overview of the run
    #[value(name = "summary")]
    Summary,
    /// Cycle-by-cycle breakdown
    #[value(name = "cycles")]
    Cycles,
    /// Cost details and breakdown
    #[value(name = "costs")]
    Costs,
    /// File changes across the run
    #[value(name = "files-changed")]
    FilesChanged,
    /// Data flow and phase contracts
    #[value(name = "data-flow")]
    DataFlow,
    /// Raw claude output from each run (equivalent to cat runs/*.log)
    #[value(name = "claude-output")]
    ClaudeOutput,
}

#[derive(Args, Debug)]
pub struct InspectArgs {
    /// Run ID to inspect (e.g. run_20240315_143022_a1b2c3)
    #[arg(add = ArgValueCompleter::new(complete_run_ids))]
    pub run_id: String,

    /// Views to display (can be specified multiple times)
    #[arg(long, action = clap::ArgAction::Append)]
    pub show: Vec<InspectView>,

    /// Filter by specific cycle number
    #[arg(long)]
    pub cycle: Option<u32>,

    /// Filter by specific phase name
    #[arg(long)]
    pub phase: Option<String>,
}

#[derive(Args, Debug)]
pub struct LineageArgs {
    /// Run ID to trace ancestry for (e.g. run_20240315_143022_a1b2c3)
    #[arg(add = ArgValueCompleter::new(complete_run_ids))]
    pub run_id: String,

    /// Show descendants instead of ancestors
    #[arg(long)]
    pub descendants: bool,
}

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Shell type (bash, zsh, fish, powershell, elvish)
    pub shell: Shell,
}

#[derive(Args, Debug)]
pub struct CleanupArgs {
    /// Remove runs older than this duration (e.g. 7d, 30d, 90d, 24h). Default: 30d.
    #[arg(long, default_value = "30d", value_name = "DURATION")]
    pub older_than: String,

    /// Show what would be deleted without deleting anything
    #[arg(long)]
    pub dry_run: bool,

    /// Skip confirmation prompt (for scripting)
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Base name for the workflow file (produces <NAME>.rings.toml). Defaults to "workflow".
    pub name: Option<String>,

    /// Overwrite the target file if it already exists
    #[arg(long)]
    pub force: bool,
}
