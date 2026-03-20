// Integration tests for run scheduling (RunSchedule iterator)
use rings::engine::{RunSchedule, RunSpec};
use rings::workflow::PhaseConfig;

fn phases(specs: &[(&str, u32)]) -> Vec<PhaseConfig> {
    specs
        .iter()
        .map(|(name, runs)| PhaseConfig {
            name: name.to_string(),
            prompt: None,
            prompt_text: Some("p".to_string()),
            runs_per_cycle: *runs,
            budget_cap_usd: None,
            timeout_per_run_secs: None,
            consumes: vec![],
            produces: vec![],
            produces_required: false,
            executor: None,
        })
        .collect()
}

#[test]
fn two_phases_one_run_each_alternates() {
    let p = phases(&[("A", 1), ("B", 1)]);
    let names: Vec<_> = RunSchedule::new(&p, 3)
        .map(|r| r.phase_name.clone())
        .collect();
    assert_eq!(names, ["A", "B", "A", "B", "A", "B"]);
}

#[test]
fn three_runs_then_one_pattern() {
    let p = phases(&[("A", 3), ("B", 1)]);
    let names: Vec<_> = RunSchedule::new(&p, 2)
        .map(|r| r.phase_name.clone())
        .collect();
    assert_eq!(names, ["A", "A", "A", "B", "A", "A", "A", "B"]);
}

#[test]
fn run_fields_are_correct() {
    let p = phases(&[("builder", 2), ("reviewer", 1)]);
    let runs: Vec<RunSpec> = RunSchedule::new(&p, 2).collect();
    assert_eq!(runs[0].cycle, 1);
    assert_eq!(runs[0].phase_name, "builder");
    assert_eq!(runs[0].phase_iteration, 1);
    assert_eq!(runs[0].phase_total_iterations, 2);
    assert_eq!(runs[0].global_run_number, 1);
    assert_eq!(runs[2].phase_name, "reviewer");
    assert_eq!(runs[3].cycle, 2);
    assert_eq!(runs[3].phase_name, "builder");
}

#[test]
fn resume_skips_already_completed_runs() {
    // AAAB × 2 cycles = A1 A2 A3 B1 | A1 A2 A3 B1 (global runs 1–8)
    // last_completed_run = 3 → resume should start at global run 4 (B, cycle 1)
    let p = phases(&[("A", 3), ("B", 1)]);
    let resumed: Vec<RunSpec> = RunSchedule::resume_from(&p, 2, 3).collect();
    assert_eq!(resumed[0].phase_name, "B");
    assert_eq!(resumed[0].global_run_number, 4);
    assert_eq!(resumed[0].cycle, 1);
}

#[test]
fn total_run_count_is_phases_times_cycles() {
    let p = phases(&[("A", 2), ("B", 3)]);
    let count = RunSchedule::new(&p, 4).count();
    assert_eq!(count, (2 + 3) * 4);
}
