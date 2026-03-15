use rings::workflow::{Workflow, WorkflowError};
use std::str::FromStr;

#[test]
fn parses_valid_workflow() {
    let toml = std::fs::read_to_string("tests/fixtures/valid.rings.toml").unwrap();
    let w = Workflow::from_str(&toml).unwrap();
    assert_eq!(w.completion_signal, "RINGS_DONE");
    assert_eq!(w.max_cycles, 10);
    assert_eq!(w.phases.len(), 2);
    assert_eq!(w.phases[0].name, "builder");
    assert_eq!(w.phases[0].runs_per_cycle, 3);
    assert_eq!(w.phases[1].runs_per_cycle, 1); // default
}

#[test]
fn rejects_empty_completion_signal() {
    let toml = r#"
        [workflow]
        completion_signal = ""
        context_dir = "."
        max_cycles = 5
        [[phases]]
        name = "a"
        prompt_text = "x"
    "#;
    assert!(matches!(
        Workflow::from_str(toml),
        Err(WorkflowError::EmptyCompletionSignal)
    ));
}

#[test]
fn rejects_duplicate_phase_names() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        [[phases]]
        name = "builder"
        prompt_text = "x"
        [[phases]]
        name = "builder"
        prompt_text = "y"
    "#;
    assert!(matches!(
        Workflow::from_str(toml),
        Err(WorkflowError::DuplicatePhaseName(_))
    ));
}

#[test]
fn rejects_zero_runs_per_cycle() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        [[phases]]
        name = "builder"
        prompt_text = "x"
        runs_per_cycle = 0
    "#;
    assert!(matches!(
        Workflow::from_str(toml),
        Err(WorkflowError::InvalidRunsPerCycle(_))
    ));
}

#[test]
fn rejects_no_phases() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
    "#;
    assert!(matches!(
        Workflow::from_str(toml),
        Err(WorkflowError::NoPhases)
    ));
}

#[test]
fn rejects_phase_with_both_prompt_and_prompt_text() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        [[phases]]
        name = "builder"
        prompt = "file.md"
        prompt_text = "inline"
    "#;
    assert!(matches!(
        Workflow::from_str(toml),
        Err(WorkflowError::AmbiguousPrompt(_))
    ));
}

#[test]
fn rejects_phase_with_no_prompt() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        [[phases]]
        name = "builder"
    "#;
    assert!(matches!(
        Workflow::from_str(toml),
        Err(WorkflowError::MissingPrompt(_))
    ));
}
