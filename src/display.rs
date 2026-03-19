use std::io::{IsTerminal, Write};

use crate::engine::RunSpec;
use crate::style;

/// Returns true if stderr is an interactive terminal.
fn is_stderr_tty() -> bool {
    std::io::stderr().is_terminal()
}

/// Format the animated status line shown while a run is in progress.
///
/// Format: `⠹  Cycle 3/10  │  builder  2/3  │  $1.47 total  │  02:34`
fn format_status_line(
    run_spec: &RunSpec,
    max_cycles: u32,
    cumulative_cost: f64,
    tick: usize,
    elapsed_secs: u64,
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

    format!(
        "{}  {}  {}  {}  {}  {}  {}  {}",
        frame, cycle_str, sep, phase_str, iter_str, sep, cost_str, elapsed_str
    )
}

/// Print an in-progress indicator before the executor is spawned.
/// On a TTY, prints without a trailing newline so later calls can overwrite it.
/// On non-TTY, prints a static status line with a newline (no animation).
pub fn print_run_start(run_spec: &RunSpec, max_cycles: u32, cumulative_cost: f64, tick: usize) {
    let line = format_status_line(run_spec, max_cycles, cumulative_cost, tick, 0);
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
) {
    if is_stderr_tty() {
        let line = format_status_line(run_spec, max_cycles, cumulative_cost, tick, elapsed_secs);
        eprint!("\r\x1b[K{line}");
        let _ = std::io::stderr().flush();
    }
    // Non-TTY: suppressed — static line was already printed by print_run_start
}

/// Print the run header shown at workflow start.
pub fn print_run_header(run_id: &str, workflow_file: &str) {
    eprintln!("● rings  {workflow_file}");
    eprintln!("  Run ID: {run_id}");
    eprintln!();
}

/// Print the cycle separator line.
pub fn print_cycle_header(cycle: u32, max_cycles: u32) {
    let divider = "─".repeat(45);
    eprintln!("  Cycle {cycle}/{max_cycles} {divider}");
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
) {
    eprintln!("✓  Completed on cycle {cycle}, run {run_number} (phase: {phase_name})");
    eprintln!(
        "   Total cost: ${total_cost_usd:.3}  ·  {total_runs} runs  ·  elapsed: {}m{}s",
        elapsed_secs / 60,
        elapsed_secs % 60,
    );
    eprintln!("   Audit log: {output_dir}/");
}

/// Print the cancellation summary.
pub fn print_cancellation(
    run_id: &str,
    cycle: u32,
    phase_name: &str,
    total_cost_usd: f64,
    resume_commands: &[String],
) {
    eprintln!();
    eprintln!("✗  Canceled (cycle {cycle}, phase: {phase_name})");
    eprintln!("   Cost so far: ${total_cost_usd:.3}");
    eprintln!("   To resume: rings resume {run_id}");
    if !resume_commands.is_empty() {
        eprintln!("   Claude sessions to resume manually:");
        for cmd in resume_commands {
            eprintln!("     {cmd}");
        }
    }
}

/// Print the max-cycles-reached summary.
pub fn print_max_cycles(max_cycles: u32, total_cost_usd: f64, total_runs: u32, run_id: &str) {
    eprintln!("⚠  max_cycles ({max_cycles}) reached without completion signal.");
    eprintln!("   Total cost: ${total_cost_usd:.3}  ·  {total_runs} runs");
    eprintln!("   To resume: rings resume {run_id}");
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
    eprintln!("✗  Executor hit a usage limit on run {run_number} (cycle {cycle}, {phase_name}).");
    eprintln!();
    eprintln!("   This is likely a quota or rate limit. No further runs will be attempted.");
    eprintln!();
    eprintln!("   Progress saved. To resume after your quota resets:");
    eprintln!("     rings resume {run_id}");
    eprintln!();
    eprintln!("   Cost so far: ${cumulative_cost:.3}");
    eprintln!("   Audit log:   {log_path}");
}

/// Print authentication error summary.
pub fn print_auth_error(
    run_number: u32,
    cycle: u32,
    phase_name: &str,
    run_id: &str,
    log_path: &str,
) {
    eprintln!("✗  Executor encountered an authentication error on run {run_number} (cycle {cycle}, {phase_name}).");
    eprintln!();
    eprintln!("   This is likely an invalid or expired API key / session.");
    eprintln!("   This error is not recoverable by waiting — fix credentials before resuming.");
    eprintln!();
    eprintln!("   To fix: verify authentication for your executor, then:");
    eprintln!("     rings resume {run_id}");
    eprintln!();
    eprintln!("   Audit log: {log_path}");
}

/// Print executor error summary (unknown error class).
pub fn print_executor_error(run_number: u32, exit_code: i32, run_id: &str, log_path: &str) {
    eprintln!("✗  Executor exited with code {exit_code} on run {run_number}.");
    eprintln!("   Cause unknown.");
    eprintln!();
    eprintln!("   Progress saved. If the error is transient, you may resume:");
    eprintln!("     rings resume {run_id}");
    eprintln!();
    eprintln!("   Full output: {log_path}");
}

/// Print budget cap reached message.
pub fn print_budget_cap_reached(cap_usd: f64, spent_usd: f64) {
    eprintln!("Error: Budget cap of ${cap_usd:.2} reached (spent ${spent_usd:.2}).");
    eprintln!("rings is stopping. Resume is available.");
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
                "⚠  Low-confidence cost parse: Run {} (cycle {}, phase {}): cost could not be parsed",
                w.run_number, w.cycle, w.phase
            );
        } else if let Some(ref snippet) = w.raw_match {
            eprintln!(
                "⚠  Low-confidence cost parse: Run {} (cycle {}, phase {}): {}",
                w.run_number, w.cycle, w.phase, snippet
            );
        }
    }

    if warnings.len() > 10 {
        let remaining = warnings.len() - 10;
        eprintln!(
            "⚠  ... and {} more low-confidence cost parse warnings.",
            remaining
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::RunSpec;

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
        crate::style::set_no_color();
        std::env::remove_var("NO_COLOR");

        let run_spec = make_run_spec();
        let line = format_status_line(&run_spec, 10, 1.47, 2, 154);

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
        crate::style::set_no_color();
        std::env::remove_var("NO_COLOR");

        let line0 = format_status_line(&run_spec, 10, 0.0, 0, 0);
        let line1 = format_status_line(&run_spec, 10, 0.0, 1, 0);

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

    #[test]
    fn non_tty_print_run_elapsed_suppresses_carriage_return() {
        // In test environments stderr is not a TTY, so print_run_elapsed is a no-op.
        // We verify by ensuring no panic and the TTY check works.
        let run_spec = make_run_spec();
        // This should not panic regardless of TTY status
        // On non-TTY (test environment), print_run_elapsed does nothing
        print_run_elapsed(&run_spec, 30, 10, 0.5, 3);
        // If we reach here without panicking, the non-TTY path works
    }
}
