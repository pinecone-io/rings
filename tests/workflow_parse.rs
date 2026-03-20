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

#[test]
fn error_profile_named_claude_code() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        [executor]
        binary = "claude"
        error_profile = "claude-code"
        [[phases]]
        name = "test"
        prompt_text = "x"
    "#;
    let workflow = Workflow::from_str(toml).unwrap();
    assert!(!workflow.compiled_error_profile.quota_regexes.is_empty());
    assert!(!workflow.compiled_error_profile.auth_regexes.is_empty());
}

#[test]
fn error_profile_named_none() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        [executor]
        binary = "claude"
        error_profile = "none"
        [[phases]]
        name = "test"
        prompt_text = "x"
    "#;
    let workflow = Workflow::from_str(toml).unwrap();
    assert!(workflow.compiled_error_profile.quota_regexes.is_empty());
    assert!(workflow.compiled_error_profile.auth_regexes.is_empty());
}

#[test]
fn error_profile_custom() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        [executor]
        binary = "claude"
        [executor.error_profile]
        quota_patterns = ["limit", "quota"]
        auth_patterns = ["unauthorized", "forbidden"]
        [[phases]]
        name = "test"
        prompt_text = "x"
    "#;
    let workflow = Workflow::from_str(toml).unwrap();
    assert_eq!(workflow.compiled_error_profile.quota_regexes.len(), 2);
    assert_eq!(workflow.compiled_error_profile.auth_regexes.len(), 2);
}

#[test]
fn error_profile_defaults_to_claude_code_when_no_executor() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        [[phases]]
        name = "test"
        prompt_text = "x"
    "#;
    let workflow = Workflow::from_str(toml).unwrap();
    assert!(!workflow.compiled_error_profile.quota_regexes.is_empty());
    assert!(!workflow.compiled_error_profile.auth_regexes.is_empty());
}

#[test]
fn error_profile_defaults_to_claude_code_when_no_profile_specified() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        [executor]
        binary = "claude"
        [[phases]]
        name = "test"
        prompt_text = "x"
    "#;
    let workflow = Workflow::from_str(toml).unwrap();
    assert!(!workflow.compiled_error_profile.quota_regexes.is_empty());
    assert!(!workflow.compiled_error_profile.auth_regexes.is_empty());
}

#[test]
fn new_workflow_fields_parsed_correctly() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        delay_between_cycles = 10
        quota_backoff = true
        quota_backoff_delay = 5
        quota_backoff_max_retries = 3
        manifest_enabled = true
        manifest_mtime_optimization = true
        snapshot_cycles = true
        manifest_ignore = ["*.tmp", "**/cache"]
        [[phases]]
        name = "test"
        prompt_text = "x"
    "#;
    let workflow = Workflow::from_str(toml).unwrap();
    assert_eq!(workflow.delay_between_cycles, 10);
    assert!(workflow.quota_backoff);
    assert_eq!(workflow.quota_backoff_delay, 5);
    assert_eq!(workflow.quota_backoff_max_retries, 3);
    assert!(workflow.manifest_enabled);
    assert!(workflow.manifest_mtime_optimization);
    assert!(workflow.snapshot_cycles);
    assert_eq!(workflow.manifest_ignore.len(), 2);
}

#[test]
fn cycle_delay_cli_override_takes_precedence_over_toml() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        delay_between_cycles = 30
        [[phases]]
        name = "test"
        prompt_text = "x"
    "#;
    let mut workflow = Workflow::from_str(toml).unwrap();
    assert_eq!(workflow.delay_between_cycles, 30);
    // Simulate what run_inner does when --cycle-delay is passed
    let cycle_delay: Option<u64> = Some(5);
    if let Some(cd) = cycle_delay {
        workflow.delay_between_cycles = cd;
    }
    assert_eq!(workflow.delay_between_cycles, 5);
}

#[test]
fn without_cycle_delay_flag_toml_value_is_preserved() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        delay_between_cycles = 30
        [[phases]]
        name = "test"
        prompt_text = "x"
    "#;
    let mut workflow = Workflow::from_str(toml).unwrap();
    // Simulate run_inner with no --cycle-delay flag
    let cycle_delay: Option<u64> = None;
    if let Some(cd) = cycle_delay {
        workflow.delay_between_cycles = cd;
    }
    assert_eq!(workflow.delay_between_cycles, 30);
}

#[test]
fn cycle_delay_zero_disables_cycle_delay() {
    let toml = r#"
        [workflow]
        completion_signal = "DONE"
        context_dir = "."
        max_cycles = 5
        delay_between_cycles = 30
        [[phases]]
        name = "test"
        prompt_text = "x"
    "#;
    let mut workflow = Workflow::from_str(toml).unwrap();
    assert_eq!(workflow.delay_between_cycles, 30);
    // --cycle-delay 0 disables the cycle delay
    let cycle_delay: Option<u64> = Some(0);
    if let Some(cd) = cycle_delay {
        workflow.delay_between_cycles = cd;
    }
    assert_eq!(workflow.delay_between_cycles, 0);
}
