use std::io::{IsTerminal, Write};

use crate::engine::RunSpec;
use crate::style;

/// Returns true if stderr is an interactive terminal.
fn is_stderr_tty() -> bool {
    std::io::stderr().is_terminal()
}

/// Format a token count for display: plain integer below 1000, `k` suffix for thousands,
/// `M` suffix for millions.
///
/// Examples: 0 → `"0"`, 999 → `"999"`, 1000 → `"1.0k"`, 18200 → `"18.2k"`, 1100000 → `"1.1M"`
pub fn format_token_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

/// Format a number with comma separators (e.g., 18204 → "18,204").
pub fn format_number_with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Format the animated status line shown while a run is in progress.
///
/// Format: `⠹  Cycle 3/10  │  builder  2/3  │  $1.47 total  │  02:34  │  18.2k in · 4.1k out`
fn format_status_line(
    run_spec: &RunSpec,
    max_cycles: u32,
    cumulative_cost: f64,
    tick: usize,
    elapsed_secs: u64,
    cumulative_input_tokens: u64,
    cumulative_output_tokens: u64,
) -> String {
    let frame = style::spinner_frame(tick);
    let sep = style::dim("│");
    let cycle_part = format!("Cycle {}/{}", run_spec.cycle, max_cycles);
    let cycle_str = style::bold(&cycle_part);
    let phase_str = style::bold(&run_spec.phase_name);
    let iter_str = format!(
        "{}/{}",
        run_spec.phase_iteration, run_spec.phase_total_iterations
    );
    let cost_part = format!("${:.2} total", cumulative_cost);
    let cost_str = style::accent(&cost_part);
    let elapsed_part = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);
    let elapsed_str = style::muted(&elapsed_part);

    let token_segment = if cumulative_input_tokens > 0 || cumulative_output_tokens > 0 {
        let in_str = format_token_count(cumulative_input_tokens);
        let out_str = format_token_count(cumulative_output_tokens);
        let token_text = format!("{} in · {} out", in_str, out_str);
        format!("  {}  {}", sep, style::dim(&token_text))
    } else {
        String::new()
    };

    format!(
        "{}  {}  {}  {}  {}  {}  {}  {}{}",
        frame, cycle_str, sep, phase_str, iter_str, sep, cost_str, elapsed_str, token_segment
    )
}

/// Print an in-progress indicator before the executor is spawned.
/// On a TTY, prints without a trailing newline so later calls can overwrite it.
/// On non-TTY, prints a static status line with a newline (no animation).
pub fn print_run_start(
    run_spec: &RunSpec,
    max_cycles: u32,
    cumulative_cost: f64,
    tick: usize,
    cumulative_input_tokens: u64,
    cumulative_output_tokens: u64,
) {
    let line = format_status_line(
        run_spec,
        max_cycles,
        cumulative_cost,
        tick,
        0,
        cumulative_input_tokens,
        cumulative_output_tokens,
    );
    if is_stderr_tty() {
        eprint!("{line}");
        let _ = std::io::stderr().flush();
    } else {
        eprintln!("{line}");
    }
}

/// Overwrite the current in-progress line with an updated spinner and elapsed time.
/// Only has effect on a TTY. Called every 100ms from the executor poll loop.
pub fn print_run_elapsed(
    run_spec: &RunSpec,
    elapsed_secs: u64,
    max_cycles: u32,
    cumulative_cost: f64,
    tick: usize,
    cumulative_input_tokens: u64,
    cumulative_output_tokens: u64,
) {
    if is_stderr_tty() {
        let line = format_status_line(
            run_spec,
            max_cycles,
            cumulative_cost,
            tick,
            elapsed_secs,
            cumulative_input_tokens,
            cumulative_output_tokens,
        );
        eprint!("\r\x1b[K{line}");
        let _ = std::io::stderr().flush();
    }
    // Non-TTY: suppressed — static line was already printed by print_run_start
}

/// Parameters for the styled startup header.
pub struct RunHeaderParams<'a> {
    pub workflow_file: &'a str,
    pub context_dir: &'a str,
    /// Phase names and their runs_per_cycle, in declaration order.
    pub phases: &'a [(String, u32)],
    pub max_cycles: u32,
    pub budget_cap_usd: Option<f64>,
    pub output_dir: &'a str,
    pub version: &'a str,
    /// Detected model name, or None to show "(default)".
    pub model: Option<&'a str>,
}

/// Format the styled startup header as a string (for testing).
fn format_run_header(params: &RunHeaderParams<'_>) -> String {
    let lw = 8usize; // "Workflow" is 8 chars — widest label
    let version_line = style::bold(&format!("rings v{}", params.version));
    let mut lines = vec![version_line, String::new()];

    lines.push(format!(
        "  {}  {}",
        style::dim(&format!("{:<lw$}", "Workflow")),
        params.workflow_file
    ));
    lines.push(format!(
        "  {}  {}",
        style::dim(&format!("{:<lw$}", "Context")),
        params.context_dir
    ));

    let phases_str = params
        .phases
        .iter()
        .map(|(name, runs)| format!("{name} \u{00D7}{runs}"))
        .collect::<Vec<_>>()
        .join(", ");
    lines.push(format!(
        "  {}  {}",
        style::dim(&format!("{:<lw$}", "Phases")),
        phases_str
    ));

    match params.model {
        Some(name) => lines.push(format!(
            "  {}  {}",
            style::dim(&format!("{:<lw$}", "Model")),
            name
        )),
        None => lines.push(format!(
            "  {}  {}",
            style::dim(&format!("{:<lw$}", "Model")),
            style::dim("(default)")
        )),
    }

    let total_runs_per_cycle: u32 = params.phases.iter().map(|(_, r)| r).sum();
    let max_total_runs = params.max_cycles * total_runs_per_cycle;
    lines.push(format!(
        "  {}  {} cycles \u{00B7} {} runs",
        style::dim(&format!("{:<lw$}", "Max")),
        params.max_cycles,
        max_total_runs
    ));

    if let Some(cap) = params.budget_cap_usd {
        lines.push(format!(
            "  {}  {}",
            style::dim(&format!("{:<lw$}", "Budget")),
            style::accent(&format!("${cap:.2}"))
        ));
    }

    lines.push(format!(
        "  {}  {}",
        style::dim(&format!("{:<lw$}", "Output")),
        style::muted(params.output_dir)
    ));

    lines.join("\n")
}

/// Print the styled startup header shown at workflow start.
///
/// Example (no color):
/// ```text
/// rings v0.1.0
///
///   Workflow   my-task.rings.toml
///   Context    ./src
///   Phases     builder ×10, reviewer ×1
///   Max        50 cycles · 550 runs
///   Budget     $5.00
///   Output     ~/.local/share/rings/runs/run_...
/// ```
pub fn print_run_header(params: &RunHeaderParams<'_>) {
    eprintln!("{}", format_run_header(params));
    eprintln!();
}

/// Format the styled cycle boundary line shown between cycles.
///
/// First cycle: `── Cycle 1 ──────────────────────────────────────────`
/// Subsequent:  `── Cycle 2 ────────────────────────── $0.14 prev ──`
fn format_cycle_boundary(cycle: u32, prev_cycle_cost: Option<f64>) -> String {
    const BOUNDARY_WIDTH: usize = 54;

    let cycle_str = cycle.to_string();
    // Visible length of "── Cycle N ": 2+1+5+1+len(N)+1 = 10+len(N)
    let prefix_visible_len = 10 + cycle_str.len();
    let prefix = format!("{} Cycle {} ", style::dim("──"), style::bold(&cycle_str));

    match prev_cycle_cost {
        None => {
            let fill_len = BOUNDARY_WIDTH.saturating_sub(prefix_visible_len);
            format!("{}{}", prefix, style::dim(&"─".repeat(fill_len)))
        }
        Some(cost) => {
            let cost_str = format!("${:.2}", cost);
            // Visible length of " $X.XX prev ──": 1+len(cost_str)+8
            let suffix_visible_len = 9 + cost_str.len();
            let fill_len = BOUNDARY_WIDTH.saturating_sub(prefix_visible_len + suffix_visible_len);
            let suffix = format!(" {} prev {}", style::accent(&cost_str), style::dim("──"));
            format!("{}{}{}", prefix, style::dim(&"─".repeat(fill_len)), suffix)
        }
    }
}

/// Print the styled cycle boundary line.
///
/// Called at the start of each cycle. When `prev_cycle_cost` is Some, the previous
/// cycle's cost is embedded in the divider. A blank line is printed after the boundary
/// to visually separate it from the first run of the new cycle.
pub fn print_cycle_boundary(cycle: u32, prev_cycle_cost: Option<f64>) {
    if prev_cycle_cost.is_some() {
        eprintln!();
    }
    eprintln!("{}", format_cycle_boundary(cycle, prev_cycle_cost));
    eprintln!();
}

/// Print a single run result line.
/// On a TTY, overwrites the in-progress spinner line. On non-TTY, prints a plain line.
pub fn print_run_result(
    run_spec: &RunSpec,
    cost_usd: f64,
    elapsed_secs: u64,
    max_cycles: u32,
    cumulative_cost: f64,
) {
    let sep = style::dim("│");
    let cycle_part = format!("Cycle {}/{}", run_spec.cycle, max_cycles);
    let cycle_str = style::bold(&cycle_part);
    let phase_str = style::bold(&run_spec.phase_name);
    let iter_str = format!(
        "{}/{}",
        run_spec.phase_iteration, run_spec.phase_total_iterations
    );
    let run_cost_part = format!("${:.3}", cost_usd);
    let run_cost_str = style::accent(&run_cost_part);
    let total_cost_part = format!("${:.2} total", cumulative_cost);
    let total_cost_str = style::accent(&total_cost_part);
    let elapsed_part = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);
    let elapsed_str = style::muted(&elapsed_part);

    let line = format!(
        "↻  {}  {}  {}  {}  {}  {}  {}  {}",
        cycle_str, sep, phase_str, iter_str, sep, run_cost_str, total_cost_str, elapsed_str
    );

    if is_stderr_tty() {
        eprintln!("\r\x1b[K{line}");
    } else {
        eprintln!("{line}");
    }
}

/// Print the cycle cost subtotal.
pub fn print_cycle_cost(cycle_cost_usd: f64) {
    eprintln!("  Cycle cost: ${cycle_cost_usd:.3}");
    eprintln!();
}

/// Render a proportional bar chart for phase costs.
///
/// Each entry is `(phase_name, cost_usd, run_count)`. Phases are in declaration order.
/// Returns one line per phase; returns an empty vec if `items` is empty.
pub fn render_bar_chart(items: &[(String, f64, u32)], max_width: usize) -> Vec<String> {
    if items.is_empty() {
        return vec![];
    }
    let total_cost: f64 = items.iter().map(|(_, c, _)| c).sum();
    let max_name_len = items.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
    items
        .iter()
        .map(|(name, cost, runs)| {
            let bar_width = if total_cost > 0.0 {
                ((cost / total_cost) * max_width as f64).round() as usize
            } else {
                0
            };
            let bar_width = bar_width.min(max_width);
            let bar = "█".repeat(bar_width);
            let padding = " ".repeat(max_width - bar_width);
            let cost_str = style::accent(&format!("${:.2}", cost));
            format!(
                "   {:<name_width$}  {}{}  {}  ({} runs)",
                name,
                bar,
                padding,
                cost_str,
                runs,
                name_width = max_name_len,
            )
        })
        .collect()
}

/// Render a budget consumption gauge.
///
/// Format: `████████████░░░░░░░░  $1.10 / $5.00  (22%)`
/// Color: green < 60%, yellow 60–85%, red > 85%.
pub fn render_budget_gauge(spent: f64, cap: f64, width: usize) -> String {
    let pct = if cap > 0.0 {
        (spent / cap).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = (pct * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    let pct_int = (pct * 100.0).round() as u32;

    let pct_display = pct_int as f64 / 100.0;
    let colored_bar = if pct_display < 0.60 {
        style::success(&bar)
    } else if pct_display <= 0.85 {
        style::warn(&bar)
    } else {
        style::error(&bar)
    };
    let cost_str = style::accent(&format!("${:.2} / ${:.2}", spent, cap));
    let pct_str = if pct_display < 0.60 {
        style::success(&format!("{}%", pct_int))
    } else if pct_display <= 0.85 {
        style::warn(&format!("{}%", pct_int))
    } else {
        style::error(&format!("{}%", pct_int))
    };
    format!("{}  {}  ({})", colored_bar, cost_str, pct_str)
}

/// Format the completion summary as a string (for testing).
#[allow(clippy::too_many_arguments)]
fn format_completion(
    cycle: u32,
    run_number: u32,
    phase_name: &str,
    total_cost_usd: f64,
    total_runs: u32,
    elapsed_secs: u64,
    output_dir: &str,
    phase_costs: &[(String, f64, u32)],
    budget_cap_usd: Option<f64>,
    total_input_tokens: u64,
    total_output_tokens: u64,
) -> String {
    let check = style::success("✓");
    let completed = style::bold("Completed");
    let lw = 10usize; // "Total cost" is 10 chars — widest label

    let mut lines = vec![format!(
        "{}  {} — cycle {}, run {} ({})",
        check,
        completed,
        style::bold(&cycle.to_string()),
        run_number,
        phase_name
    )];
    lines.push(String::new());

    let mins = elapsed_secs / 60;
    let secs = elapsed_secs % 60;
    let duration_val = if mins > 0 {
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    };
    lines.push(format!(
        "   {}  {}",
        style::dim(&format!("{:<lw$}", "Duration")),
        duration_val
    ));

    let cost_val = style::accent(&format!("${:.2}", total_cost_usd));
    lines.push(format!(
        "   {}  {}  ({} runs)",
        style::dim(&format!("{:<lw$}", "Total cost")),
        cost_val,
        total_runs
    ));

    if total_input_tokens > 0 || total_output_tokens > 0 {
        let token_text = format!(
            "{} input · {} output",
            format_number_with_commas(total_input_tokens),
            format_number_with_commas(total_output_tokens)
        );
        lines.push(format!(
            "   {}  {}",
            style::dim(&format!("{:<lw$}", "Tokens")),
            style::dim(&token_text)
        ));
    }

    // Phase bar chart
    if !phase_costs.is_empty() {
        lines.push(String::new());
        for line in render_bar_chart(phase_costs, 20) {
            lines.push(line);
        }
    }

    // Budget gauge
    if let Some(cap) = budget_cap_usd {
        lines.push(String::new());
        let gauge = render_budget_gauge(total_cost_usd, cap, 20);
        lines.push(format!(
            "   {}  {}",
            style::dim(&format!("{:<lw$}", "Budget")),
            gauge
        ));
    }

    lines.push(String::new());
    lines.push(format!(
        "   {}  {}",
        style::dim(&format!("{:<lw$}", "Audit logs")),
        style::muted(&format!("{}/", output_dir))
    ));

    lines.join("\n")
}

/// Print the completion summary.
#[allow(clippy::too_many_arguments)]
pub fn print_completion(
    cycle: u32,
    run_number: u32,
    phase_name: &str,
    total_cost_usd: f64,
    total_runs: u32,
    elapsed_secs: u64,
    output_dir: &str,
    phase_costs: &[(String, f64, u32)],
    budget_cap_usd: Option<f64>,
    total_input_tokens: u64,
    total_output_tokens: u64,
) {
    eprintln!(
        "{}",
        format_completion(
            cycle,
            run_number,
            phase_name,
            total_cost_usd,
            total_runs,
            elapsed_secs,
            output_dir,
            phase_costs,
            budget_cap_usd,
            total_input_tokens,
            total_output_tokens,
        )
    );
}

/// Print the cancellation summary.
#[allow(clippy::too_many_arguments)]
pub fn print_cancellation(
    run_id: &str,
    cycle: u32,
    phase_name: &str,
    total_cost_usd: f64,
    total_runs: u32,
    phase_costs: &[(String, f64, u32)],
    resume_commands: &[String],
    output_dir: &str,
    total_input_tokens: u64,
    total_output_tokens: u64,
) {
    let marker = style::error("✗");
    let label_interrupted = style::bold("Interrupted");
    let lw = 10usize;

    eprintln!();
    eprintln!("{}  {}", marker, label_interrupted);
    eprintln!();
    eprintln!(
        "   {}  {}",
        style::dim(&format!("{:<lw$}", "Run ID")),
        style::muted(run_id)
    );
    eprintln!(
        "   {}  cycle {}, {} ({} runs)",
        style::dim(&format!("{:<lw$}", "Progress")),
        cycle,
        phase_name,
        total_runs,
    );
    eprintln!(
        "   {}  {}",
        style::dim(&format!("{:<lw$}", "Cost")),
        style::accent(&format!("${:.2}", total_cost_usd))
    );

    if total_input_tokens > 0 || total_output_tokens > 0 {
        let token_text = format!(
            "{} input · {} output",
            format_number_with_commas(total_input_tokens),
            format_number_with_commas(total_output_tokens)
        );
        eprintln!(
            "   {}  {}",
            style::dim(&format!("{:<lw$}", "Tokens")),
            style::dim(&token_text)
        );
    }

    if !phase_costs.is_empty() {
        eprintln!();
        for line in render_bar_chart(phase_costs, 20) {
            eprintln!("{line}");
        }
    }

    eprintln!();
    eprintln!("   To resume:");
    eprintln!(
        "     {}",
        style::bold(&style::accent(&format!("rings resume {run_id}")))
    );

    if !resume_commands.is_empty() {
        eprintln!();
        eprintln!("   Partial sessions:");
        for cmd in resume_commands {
            eprintln!("     {}", style::muted(cmd));
        }
    }

    eprintln!();
    eprintln!(
        "   {}  {}",
        style::dim(&format!("{:<lw$}", "Audit logs")),
        style::muted(&format!("{}/", output_dir))
    );
}

/// Print the max-cycles-reached summary.
pub fn print_max_cycles(max_cycles: u32, total_cost_usd: f64, total_runs: u32, run_id: &str) {
    eprintln!(
        "{}  max_cycles ({}) reached without completion signal.",
        style::warn("⚠"),
        max_cycles
    );
    eprintln!(
        "   Total cost: {}  ·  {} runs",
        style::accent(&format!("${:.2}", total_cost_usd)),
        total_runs
    );
    eprintln!(
        "   To resume: {}",
        style::bold(&style::accent(&format!("rings resume {run_id}")))
    );
}

/// Print quota error summary.
pub fn print_quota_error(
    run_number: u32,
    cycle: u32,
    phase_name: &str,
    run_id: &str,
    cumulative_cost: f64,
    log_path: &str,
) {
    eprintln!(
        "{}  Executor hit a usage limit on run {} (cycle {}, {}).",
        style::error("✗"),
        run_number,
        cycle,
        phase_name
    );
    eprintln!();
    eprintln!(
        "   {}",
        style::dim("This is likely a quota or rate limit. No further runs will be attempted.")
    );
    eprintln!();
    eprintln!("   Progress saved. To resume after your quota resets:");
    eprintln!(
        "     {}",
        style::bold(&style::accent(&format!("rings resume {run_id}")))
    );
    eprintln!();
    eprintln!(
        "   Cost so far: {}",
        style::accent(&format!("${:.2}", cumulative_cost))
    );
    eprintln!("   Audit log:   {}", style::muted(log_path));
}

/// Print authentication error summary.
pub fn print_auth_error(
    run_number: u32,
    cycle: u32,
    phase_name: &str,
    run_id: &str,
    log_path: &str,
) {
    eprintln!(
        "{}  Executor encountered an authentication error on run {} (cycle {}, {}).",
        style::error("✗"),
        run_number,
        cycle,
        phase_name
    );
    eprintln!();
    eprintln!(
        "   {}",
        style::dim("This is likely an invalid or expired API key / session.")
    );
    eprintln!(
        "   {}",
        style::dim("This error is not recoverable by waiting — fix credentials before resuming.")
    );
    eprintln!();
    eprintln!("   To fix: verify authentication for your executor, then:");
    eprintln!(
        "     {}",
        style::bold(&style::accent(&format!("rings resume {run_id}")))
    );
    eprintln!();
    eprintln!("   Audit log: {}", style::muted(log_path));
}

/// Print executor error summary (unknown error class).
pub fn print_executor_error(run_number: u32, exit_code: i32, run_id: &str, log_path: &str) {
    eprintln!(
        "{}  Executor exited with code {} on run {}.",
        style::error("✗"),
        exit_code,
        run_number
    );
    eprintln!("   {}", style::dim("Cause unknown."));
    eprintln!();
    eprintln!("   Progress saved. If the error is transient, you may resume:");
    eprintln!(
        "     {}",
        style::bold(&style::accent(&format!("rings resume {run_id}")))
    );
    eprintln!();
    eprintln!("   Full output: {}", style::muted(log_path));
}

/// Print budget cap reached message.
pub fn print_budget_cap_reached(cap_usd: f64, spent_usd: f64) {
    eprintln!(
        "{}  Budget cap reached: {}",
        style::error("✗"),
        render_budget_gauge(spent_usd, cap_usd, 20)
    );
    eprintln!("   rings is stopping. Resume is available.");
}

/// Print low-confidence cost parse warnings (up to 10, then summary).
pub fn print_parse_warnings(warnings: &[crate::cost::ParseWarning]) {
    if warnings.is_empty() {
        return;
    }

    eprintln!();
    let display_count = std::cmp::min(warnings.len(), 10);
    for w in warnings.iter().take(display_count) {
        if w.confidence == crate::cost::ParseConfidence::None {
            eprintln!(
                "{}  Low-confidence cost parse: Run {} (cycle {}, phase {}): cost could not be parsed",
                style::warn("⚠"),
                w.run_number, w.cycle, w.phase
            );
        } else if let Some(ref snippet) = w.raw_match {
            eprintln!(
                "{}  Low-confidence cost parse: Run {} (cycle {}, phase {}): {}",
                style::warn("⚠"),
                w.run_number,
                w.cycle,
                w.phase,
                snippet
            );
        }
    }

    if warnings.len() > 10 {
        let remaining = warnings.len() - 10;
        eprintln!(
            "{}  ... and {} more low-confidence cost parse warnings.",
            style::warn("⚠"),
            remaining
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::RunSpec;
    use std::sync::Mutex;

    // Serialize tests that mutate global color state to prevent races.
    static COLOR_LOCK: Mutex<()> = Mutex::new(());

    fn make_run_spec() -> RunSpec {
        RunSpec {
            global_run_number: 7,
            cycle: 3,
            phase_name: "builder".to_string(),
            phase_index: 0,
            phase_iteration: 2,
            phase_total_iterations: 3,
            prompt_text: None,
        }
    }

    #[test]
    fn status_line_contains_expected_segments() {
        // Disable color so we can match plain text
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        std::env::remove_var("NO_COLOR");

        let run_spec = make_run_spec();
        let line = format_status_line(&run_spec, 10, 1.47, 2, 154, 0, 0);

        // Check cycle segment
        assert!(line.contains("Cycle 3/10"), "missing cycle: {line}");
        // Check phase name and iteration
        assert!(line.contains("builder"), "missing phase name: {line}");
        assert!(line.contains("2/3"), "missing iteration: {line}");
        // Check cost
        assert!(line.contains("$1.47 total"), "missing cost: {line}");
        // Check elapsed (154s = 2m34s)
        assert!(line.contains("02:34"), "missing elapsed: {line}");
        // Check separator
        assert!(line.contains("│"), "missing separator: {line}");

        crate::style::set_color_enabled();
    }

    #[test]
    fn spinner_frame_advances_on_successive_ticks() {
        let run_spec = make_run_spec();
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        std::env::remove_var("NO_COLOR");

        let line0 = format_status_line(&run_spec, 10, 0.0, 0, 0, 0, 0);
        let line1 = format_status_line(&run_spec, 10, 0.0, 1, 0, 0, 0);

        // Different ticks should produce different spinner frames at the start
        let frame0 = crate::style::SPINNER_FRAMES[0];
        let frame1 = crate::style::SPINNER_FRAMES[1];
        assert!(
            line0.starts_with(frame0),
            "tick 0 should use frame 0: {line0}"
        );
        assert!(
            line1.starts_with(frame1),
            "tick 1 should use frame 1: {line1}"
        );

        crate::style::set_color_enabled();
    }

    fn make_header_params() -> (Vec<(String, u32)>, String) {
        let phases = vec![
            ("builder".to_string(), 10u32),
            ("reviewer".to_string(), 1u32),
        ];
        let output = "/home/user/.local/share/rings/runs/run_abc".to_string();
        (phases, output)
    }

    #[test]
    fn run_header_contains_expected_labels() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let (phases, output) = make_header_params();
        let params = RunHeaderParams {
            workflow_file: "my-task.rings.toml",
            context_dir: "./src",
            phases: &phases,
            max_cycles: 50,
            budget_cap_usd: None,
            output_dir: &output,
            version: "0.1.0",
            model: None,
        };
        let s = format_run_header(&params);
        assert!(s.contains("Workflow"), "missing Workflow: {s}");
        assert!(
            s.contains("my-task.rings.toml"),
            "missing workflow file: {s}"
        );
        assert!(s.contains("Context"), "missing Context: {s}");
        assert!(s.contains("./src"), "missing context_dir: {s}");
        assert!(s.contains("Phases"), "missing Phases: {s}");
        assert!(s.contains("builder"), "missing phase name: {s}");
        assert!(s.contains("Max"), "missing Max: {s}");
        assert!(s.contains("50 cycles"), "missing cycles: {s}");
        assert!(s.contains("550 runs"), "missing total runs: {s}");
        assert!(s.contains("Output"), "missing Output: {s}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn run_header_budget_line_present_when_cap_set() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let (phases, output) = make_header_params();
        let params = RunHeaderParams {
            workflow_file: "my-task.rings.toml",
            context_dir: "./src",
            phases: &phases,
            max_cycles: 50,
            budget_cap_usd: Some(5.0),
            output_dir: &output,
            version: "0.1.0",
            model: None,
        };
        let s = format_run_header(&params);
        assert!(
            s.contains("Budget"),
            "Budget line missing when cap set: {s}"
        );
        assert!(s.contains("$5.00"), "Budget value missing: {s}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn run_header_budget_line_absent_when_no_cap() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let (phases, output) = make_header_params();
        let params = RunHeaderParams {
            workflow_file: "my-task.rings.toml",
            context_dir: "./src",
            phases: &phases,
            max_cycles: 50,
            budget_cap_usd: None,
            output_dir: &output,
            version: "0.1.0",
            model: None,
        };
        let s = format_run_header(&params);
        assert!(
            !s.contains("Budget"),
            "Budget line present when cap is None: {s}"
        );
        crate::style::set_color_enabled();
    }

    #[test]
    fn run_header_no_ansi_when_color_disabled() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let (phases, output) = make_header_params();
        let params = RunHeaderParams {
            workflow_file: "my-task.rings.toml",
            context_dir: "./src",
            phases: &phases,
            max_cycles: 50,
            budget_cap_usd: Some(5.0),
            output_dir: &output,
            version: "0.1.0",
            model: None,
        };
        let s = format_run_header(&params);
        assert!(
            !s.contains('\x1b'),
            "ANSI escapes present when color disabled: {s:?}"
        );
        crate::style::set_color_enabled();
    }

    #[test]
    fn run_header_shows_model_name_when_detected() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let (phases, output) = make_header_params();
        let params = RunHeaderParams {
            workflow_file: "my-task.rings.toml",
            context_dir: "./src",
            phases: &phases,
            max_cycles: 50,
            budget_cap_usd: None,
            output_dir: &output,
            version: "0.1.0",
            model: Some("claude-sonnet-4-5"),
        };
        let s = format_run_header(&params);
        assert!(s.contains("Model"), "Model label missing: {s}");
        assert!(s.contains("claude-sonnet-4-5"), "model name missing: {s}");
        assert!(!s.contains("(default)"), "should not show (default): {s}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn run_header_shows_default_when_no_model_detected() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let (phases, output) = make_header_params();
        let params = RunHeaderParams {
            workflow_file: "my-task.rings.toml",
            context_dir: "./src",
            phases: &phases,
            max_cycles: 50,
            budget_cap_usd: None,
            output_dir: &output,
            version: "0.1.0",
            model: None,
        };
        let s = format_run_header(&params);
        assert!(s.contains("Model"), "Model label missing: {s}");
        assert!(s.contains("(default)"), "(default) missing: {s}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn non_tty_print_run_elapsed_suppresses_carriage_return() {
        // In test environments stderr is not a TTY, so print_run_elapsed is a no-op.
        // We verify by ensuring no panic and the TTY check works.
        let run_spec = make_run_spec();
        // This should not panic regardless of TTY status
        // On non-TTY (test environment), print_run_elapsed does nothing
        print_run_elapsed(&run_spec, 30, 10, 0.5, 3, 0, 0);
        // If we reach here without panicking, the non-TTY path works
    }

    #[test]
    fn cycle_boundary_first_cycle_no_cost_suffix() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let s = format_cycle_boundary(1, None);
        assert!(s.contains("Cycle 1"), "missing cycle number: {s}");
        assert!(s.contains("──"), "missing divider: {s}");
        assert!(
            !s.contains("prev"),
            "first cycle should have no cost suffix: {s}"
        );
        assert!(!s.contains('$'), "first cycle should have no cost: {s}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn cycle_boundary_subsequent_cycle_shows_prev_cost() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let s = format_cycle_boundary(2, Some(0.14));
        assert!(s.contains("Cycle 2"), "missing cycle number: {s}");
        assert!(s.contains("$0.14"), "missing cost: {s}");
        assert!(s.contains("prev"), "missing 'prev' label: {s}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn cycle_boundary_format_matches_spec_pattern() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        // Without color, the format should be: "── Cycle N <dashes>"
        // or "── Cycle N <dashes> $X.XX prev ──"
        let s1 = format_cycle_boundary(1, None);
        assert!(s1.starts_with("── Cycle 1 "), "unexpected prefix: {s1}");
        // Should end with dashes (no cost suffix)
        assert!(
            s1.ends_with('─'),
            "first cycle should end with dashes: {s1}"
        );

        let s2 = format_cycle_boundary(2, Some(0.14));
        assert!(s2.starts_with("── Cycle 2 "), "unexpected prefix: {s2}");
        // Should end with "── " trail dashes
        assert!(
            s2.ends_with("──"),
            "subsequent cycle should end with ──: {s2}"
        );
        crate::style::set_color_enabled();
    }

    #[test]
    fn bar_chart_full_single_phase_is_full_bar() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let items = vec![("builder".to_string(), 1.0, 10u32)];
        let lines = render_bar_chart(&items, 20);
        assert_eq!(lines.len(), 1);
        // Full bar: 20 '█' chars, no spaces in bar portion
        assert!(
            lines[0].contains(&"█".repeat(20)),
            "expected full bar: {}",
            lines[0]
        );
        assert!(!lines[0].contains("░"), "no remainder chars expected");
        crate::style::set_color_enabled();
    }

    #[test]
    fn bar_chart_fifty_fifty_equal_bars() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let items = vec![
            ("builder".to_string(), 0.5, 5u32),
            ("reviewer".to_string(), 0.5, 5u32),
        ];
        let lines = render_bar_chart(&items, 20);
        assert_eq!(lines.len(), 2);
        // Each should have 10 blocks
        assert!(
            lines[0].contains(&"█".repeat(10)),
            "expected 10 blocks: {}",
            lines[0]
        );
        assert!(
            lines[1].contains(&"█".repeat(10)),
            "expected 10 blocks: {}",
            lines[1]
        );
        crate::style::set_color_enabled();
    }

    #[test]
    fn bar_chart_includes_phase_name_cost_runs() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let items = vec![("builder".to_string(), 0.89, 10u32)];
        let lines = render_bar_chart(&items, 20);
        assert!(
            lines[0].contains("builder"),
            "missing phase name: {}",
            lines[0]
        );
        assert!(lines[0].contains("$0.89"), "missing cost: {}", lines[0]);
        assert!(
            lines[0].contains("10 runs"),
            "missing run count: {}",
            lines[0]
        );
        crate::style::set_color_enabled();
    }

    #[test]
    fn budget_gauge_zero_spent_all_empty() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let gauge = render_budget_gauge(0.0, 5.0, 20);
        assert!(
            gauge.contains(&"░".repeat(20)),
            "expected all empty: {gauge}"
        );
        assert!(!gauge.contains('█'), "no filled blocks expected: {gauge}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn budget_gauge_full_all_filled() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let gauge = render_budget_gauge(5.0, 5.0, 20);
        assert!(
            gauge.contains(&"█".repeat(20)),
            "expected all filled: {gauge}"
        );
        assert!(!gauge.contains('░'), "no empty blocks expected: {gauge}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn budget_gauge_shows_cost_and_percentage() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        // 22% spent
        let gauge = render_budget_gauge(1.1, 5.0, 20);
        assert!(
            gauge.contains("$1.10 / $5.00"),
            "missing cost values: {gauge}"
        );
        assert!(gauge.contains("22%"), "missing percentage: {gauge}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn completion_output_contains_expected_fields() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let phase_costs = vec![
            ("builder".to_string(), 0.89, 10u32),
            ("reviewer".to_string(), 0.21, 2u32),
        ];
        let s = format_completion(
            2,
            12,
            "builder",
            1.10,
            12,
            494,
            "/tmp/run",
            &phase_costs,
            Some(5.0),
            0,
            0,
        );
        assert!(s.contains("Completed"), "missing Completed: {s}");
        assert!(s.contains("cycle 2"), "missing cycle: {s}");
        assert!(s.contains("run 12"), "missing run: {s}");
        assert!(s.contains("builder"), "missing phase: {s}");
        assert!(s.contains("Duration"), "missing Duration: {s}");
        assert!(s.contains("Total cost"), "missing Total cost: {s}");
        assert!(s.contains("$1.10"), "missing total cost value: {s}");
        assert!(s.contains("12 runs"), "missing run count: {s}");
        assert!(s.contains("Budget"), "missing Budget gauge: {s}");
        assert!(s.contains("Audit logs"), "missing Audit logs: {s}");
        assert!(s.contains("/tmp/run"), "missing output dir: {s}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn completion_output_no_budget_when_no_cap() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let phase_costs: Vec<(String, f64, u32)> = vec![];
        let s = format_completion(
            1,
            5,
            "builder",
            0.50,
            5,
            120,
            "/tmp/run",
            &phase_costs,
            None,
            0,
            0,
        );
        assert!(
            !s.contains("Budget"),
            "Budget should be absent when no cap: {s}"
        );
        crate::style::set_color_enabled();
    }

    #[test]
    fn format_token_count_various() {
        assert_eq!(format_token_count(0), "0");
        assert_eq!(format_token_count(999), "999");
        assert_eq!(format_token_count(1000), "1.0k");
        assert_eq!(format_token_count(18200), "18.2k");
        assert_eq!(format_token_count(1_100_000), "1.1M");
    }

    #[test]
    fn status_line_includes_token_segment_when_tokens_nonzero() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let run_spec = make_run_spec();
        let line = format_status_line(&run_spec, 10, 1.47, 2, 154, 18200, 4100);
        assert!(line.contains("18.2k in"), "missing input tokens: {line}");
        assert!(line.contains("4.1k out"), "missing output tokens: {line}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn status_line_omits_token_segment_when_both_zero() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let run_spec = make_run_spec();
        let line = format_status_line(&run_spec, 10, 1.47, 2, 154, 0, 0);
        assert!(
            !line.contains(" in "),
            "token segment should be absent: {line}"
        );
        assert!(
            !line.contains(" out"),
            "token segment should be absent: {line}"
        );
        crate::style::set_color_enabled();
    }

    #[test]
    fn completion_output_contains_checkmark() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let phase_costs: Vec<(String, f64, u32)> = vec![];
        let s = format_completion(
            1,
            5,
            "builder",
            0.50,
            5,
            120,
            "/tmp/run",
            &phase_costs,
            None,
            0,
            0,
        );
        assert!(s.contains('✓'), "missing checkmark: {s}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn completion_output_includes_token_line_when_tokens_nonzero() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let phase_costs: Vec<(String, f64, u32)> = vec![];
        let s = format_completion(
            1,
            5,
            "builder",
            0.50,
            5,
            120,
            "/tmp/run",
            &phase_costs,
            None,
            18204,
            4102,
        );
        assert!(s.contains("Tokens"), "missing Tokens label: {s}");
        assert!(s.contains("18,204 input"), "missing input tokens: {s}");
        assert!(s.contains("4,102 output"), "missing output tokens: {s}");
        crate::style::set_color_enabled();
    }

    #[test]
    fn completion_output_omits_token_line_when_both_zero() {
        let _guard = COLOR_LOCK.lock().unwrap();
        crate::style::set_no_color();
        let phase_costs: Vec<(String, f64, u32)> = vec![];
        let s = format_completion(
            1,
            5,
            "builder",
            0.50,
            5,
            120,
            "/tmp/run",
            &phase_costs,
            None,
            0,
            0,
        );
        assert!(
            !s.contains("Tokens"),
            "Tokens line should be absent when both zero: {s}"
        );
        crate::style::set_color_enabled();
    }

    #[test]
    fn format_number_with_commas_various() {
        assert_eq!(format_number_with_commas(0), "0");
        assert_eq!(format_number_with_commas(999), "999");
        assert_eq!(format_number_with_commas(1000), "1,000");
        assert_eq!(format_number_with_commas(18204), "18,204");
        assert_eq!(format_number_with_commas(1_100_000), "1,100,000");
    }
}
