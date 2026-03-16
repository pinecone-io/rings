use crate::engine::RunSpec;

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
pub fn print_run_result(run_spec: &RunSpec, cost_usd: f64, elapsed_secs: u64) {
    eprintln!(
        "  ↻  {:<12} {}/{}   ${:.3}   [{:02}:{:02}]",
        run_spec.phase_name,
        run_spec.phase_iteration,
        run_spec.phase_total_iterations,
        cost_usd,
        elapsed_secs / 60,
        elapsed_secs % 60,
    );
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

/// Print executor error summary.
pub fn print_executor_error(run_number: u32, exit_code: i32, run_id: &str, log_path: &str) {
    eprintln!("✗  Executor exited with code {exit_code} on run {run_number}.");
    eprintln!("   State saved. To resume: rings resume {run_id}");
    eprintln!("   Log: {log_path}");
}

/// Print budget cap reached message.
pub fn print_budget_cap_reached(cap_usd: f64, spent_usd: f64) {
    eprintln!("Error: Budget cap of ${cap_usd:.2} reached (spent ${spent_usd:.2}).");
    eprintln!("rings is stopping. Resume is available.");
}
