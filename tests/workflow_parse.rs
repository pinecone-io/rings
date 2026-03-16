use rings::workflow::{Workflow, WorkflowError};
use std::str::FromStr;

fn valid_phase() -> &'static str {
    r#"
        [[phases]]
        name = "builder"
        prompt_text = "x"
    "#
}

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

#[test]
fn rejects_nonexistent_context_dir() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "/nonexistent/path/xyz_does_not_exist"
        max_cycles = 5
        [[phases]]
        name = "builder"
        prompt_text = "x"
    "#;
    assert!(matches!(
        Workflow::from_str(toml),
        Err(WorkflowError::ContextDirNotFound(_))
    ));
}

#[test]
fn rejects_zero_budget_cap() {
    let toml = format!(
        r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        budget_cap_usd = 0.0
        {}
        "#,
        valid_phase()
    );
    assert!(matches!(
        Workflow::from_str(&toml),
        Err(WorkflowError::InvalidBudgetCap)
    ));
}

#[test]
fn rejects_negative_budget_cap() {
    let toml = format!(
        r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        budget_cap_usd = -5.0
        {}
        "#,
        valid_phase()
    );
    assert!(matches!(
        Workflow::from_str(&toml),
        Err(WorkflowError::InvalidBudgetCap)
    ));
}

#[test]
fn accepts_valid_budget_cap() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        budget_cap_usd = 10.0
        [[phases]]
        name = "builder"
        prompt_text = "x"
    "#;
    let w = Workflow::from_str(toml).unwrap();
    assert_eq!(w.budget_cap_usd, Some(10.0));
}

#[test]
fn resolves_timeout_string_to_secs() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        timeout_per_run_secs = "5m"
        [[phases]]
        name = "builder"
        prompt_text = "x"
    "#;
    let w = Workflow::from_str(toml).unwrap();
    assert_eq!(w.timeout_per_run_secs, Some(300));
}

#[test]
fn resolves_timeout_integer_to_secs() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        timeout_per_run_secs = 120
        [[phases]]
        name = "builder"
        prompt_text = "x"
    "#;
    let w = Workflow::from_str(toml).unwrap();
    assert_eq!(w.timeout_per_run_secs, Some(120));
}

#[test]
fn rejects_zero_timeout() {
    let toml = format!(
        r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        timeout_per_run_secs = "0s"
        {}
        "#,
        valid_phase()
    );
    assert!(matches!(
        Workflow::from_str(&toml),
        Err(WorkflowError::InvalidDuration { .. })
    ));
}

#[test]
fn rejects_phase_zero_budget_cap() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        [[phases]]
        name = "builder"
        prompt_text = "x"
        budget_cap_usd = 0.0
    "#;
    assert!(matches!(
        Workflow::from_str(toml),
        Err(WorkflowError::InvalidBudgetCap)
    ));
}

#[test]
fn rejects_phase_invalid_timeout() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        [[phases]]
        name = "builder"
        prompt_text = "x"
        timeout_per_run_secs = "0s"
    "#;
    assert!(matches!(
        Workflow::from_str(toml),
        Err(WorkflowError::InvalidDuration { .. })
    ));
}
