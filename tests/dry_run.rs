use rings::dry_run::DryRunPlan;
use rings::workflow::Workflow;
use std::fs;
use std::str::FromStr;
use tempfile::TempDir;

fn create_test_workflow(
    completion_signal: &str,
    signal_mode: Option<&str>,
    phases: Vec<(&str, u32, Option<&str>, Option<&str>)>,
) -> (Workflow, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let context_dir = temp_dir.path().join("context");
    fs::create_dir_all(&context_dir).unwrap();

    let mut toml = format!(
        r#"
[workflow]
completion_signal = "{}"
context_dir = "{}"
max_cycles = 10
completion_signal_mode = "{}"

"#,
        completion_signal,
        context_dir.display(),
        signal_mode.unwrap_or("substring")
    );

    for (_idx, (name, runs, prompt_file, prompt_text)) in phases.into_iter().enumerate() {
        if let Some(prompt_file) = prompt_file {
            let file_path = temp_dir.path().join(prompt_file);
            fs::create_dir_all(file_path.parent().unwrap()).ok();
            fs::write(&file_path, "").unwrap();
            toml.push_str(&format!(
                r#"[[phases]]
name = "{}"
prompt = "{}"
runs_per_cycle = {}

"#,
                name,
                file_path.display(),
                runs
            ));
        } else if let Some(prompt_text) = prompt_text {
            toml.push_str(&format!(
                r#"[[phases]]
name = "{}"
prompt_text = "{}"
runs_per_cycle = {}

"#,
                name, prompt_text, runs
            ));
        }
    }

    let workflow = Workflow::from_str(&toml).expect("Failed to parse workflow");
    (workflow, temp_dir)
}

#[test]
fn dry_run_plan_total_runs_per_cycle() {
    let (workflow, _temp) = create_test_workflow(
        "DONE",
        None,
        vec![
            ("builder", 3, Some("prompts/builder.md"), None),
            ("reviewer", 1, Some("prompts/reviewer.md"), None),
        ],
    );

    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();
    assert_eq!(plan.runs_per_cycle_total, 4);
}

#[test]
fn dry_run_plan_max_total_runs() {
    let (workflow, _temp) = create_test_workflow(
        "DONE",
        None,
        vec![("test", 2, Some("prompts/test.md"), None)],
    );

    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();
    assert_eq!(plan.max_cycles, Some(10));
    assert_eq!(plan.max_total_runs, Some(20));
}

#[test]
fn signal_found_in_file_prompt_with_line_number() {
    let temp_dir = TempDir::new().unwrap();
    let context_dir = temp_dir.path().join("context");
    fs::create_dir_all(&context_dir).unwrap();

    let prompt_file = temp_dir.path().join("prompts").join("test.md");
    fs::create_dir_all(prompt_file.parent().unwrap()).unwrap();
    fs::write(&prompt_file, "Line 1\nLine 2\nDONE\nLine 4").unwrap();

    let toml = format!(
        r#"
[workflow]
completion_signal = "DONE"
context_dir = "{}"
max_cycles = 10

[[phases]]
name = "test"
prompt = "{}"
runs_per_cycle = 1
"#,
        context_dir.display(),
        prompt_file.display()
    );

    let workflow = Workflow::from_str(&toml).unwrap();
    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();

    assert_eq!(plan.phases[0].signal_check.found, true);
    assert_eq!(plan.phases[0].signal_check.line_number, Some(3));
}

#[test]
fn signal_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let context_dir = temp_dir.path().join("context");
    fs::create_dir_all(&context_dir).unwrap();

    let prompt_file = temp_dir.path().join("prompts").join("test.md");
    fs::create_dir_all(prompt_file.parent().unwrap()).unwrap();
    fs::write(&prompt_file, "Line 1\nLine 2\nLine 3").unwrap();

    let toml = format!(
        r#"
[workflow]
completion_signal = "NOT_THERE"
context_dir = "{}"
max_cycles = 10

[[phases]]
name = "test"
prompt = "{}"
runs_per_cycle = 1
"#,
        context_dir.display(),
        prompt_file.display()
    );

    let workflow = Workflow::from_str(&toml).unwrap();
    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();

    assert_eq!(plan.phases[0].signal_check.found, false);
    assert_eq!(plan.phases[0].signal_check.line_number, None);
}

#[test]
fn inline_prompt_text_phase() {
    let (workflow, _temp) = create_test_workflow(
        "DONE",
        None,
        vec![(
            "inline",
            1,
            None,
            Some("This is inline {{phase_name}} DONE"),
        )],
    );

    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();
    assert_eq!(plan.phases[0].prompt_source, "<inline prompt_text>");
}

#[test]
fn unknown_variables_in_prompt() {
    let (workflow, _temp) = create_test_workflow(
        "DONE",
        None,
        vec![("test", 1, None, Some("Use {{badvar}} here"))],
    );

    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();
    assert!(plan.phases[0].unknown_vars.contains(&"badvar".to_string()));
}

#[test]
fn known_variables_not_reported() {
    let (workflow, _temp) = create_test_workflow(
        "DONE",
        None,
        vec![(
            "test",
            1,
            None,
            Some("Use {{phase_name}} {{cycle}} {{run}}"),
        )],
    );

    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();
    assert_eq!(plan.phases[0].unknown_vars.len(), 0);
}

#[test]
fn invalid_regex_pattern_fails() {
    let temp_dir = TempDir::new().unwrap();
    let context_dir = temp_dir.path().join("context");
    fs::create_dir_all(&context_dir).unwrap();

    let toml = format!(
        r#"
[workflow]
completion_signal = "["
context_dir = "{}"
max_cycles = 10
completion_signal_mode = "regex"

[[phases]]
name = "test"
prompt_text = "test"
runs_per_cycle = 1
"#,
        context_dir.display()
    );

    // Invalid regex is now rejected at workflow parse time, not dry-run time.
    let result = Workflow::from_str(&toml);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("invalid regex"));
}

#[test]
fn line_mode_signal_check() {
    let temp_dir = TempDir::new().unwrap();
    let context_dir = temp_dir.path().join("context");
    fs::create_dir_all(&context_dir).unwrap();

    let prompt_file = temp_dir.path().join("prompts").join("test.md");
    fs::create_dir_all(prompt_file.parent().unwrap()).unwrap();
    fs::write(&prompt_file, "This is some text\nDONE\nMore text").unwrap();

    let toml = format!(
        r#"
[workflow]
completion_signal = "DONE"
context_dir = "{}"
max_cycles = 10
completion_signal_mode = "line"

[[phases]]
name = "test"
prompt = "{}"
runs_per_cycle = 1
"#,
        context_dir.display(),
        prompt_file.display()
    );

    let workflow = Workflow::from_str(&toml).unwrap();
    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();

    assert!(plan.phases[0].signal_check.found);
    assert_eq!(plan.phases[0].signal_check.line_number, Some(2));
}

#[test]
fn line_mode_not_found_when_inline() {
    let temp_dir = TempDir::new().unwrap();
    let context_dir = temp_dir.path().join("context");
    fs::create_dir_all(&context_dir).unwrap();

    let prompt_file = temp_dir.path().join("prompts").join("test.md");
    fs::create_dir_all(prompt_file.parent().unwrap()).unwrap();
    fs::write(&prompt_file, "This contains DONE but not alone").unwrap();

    let toml = format!(
        r#"
[workflow]
completion_signal = "DONE"
context_dir = "{}"
max_cycles = 10
completion_signal_mode = "line"

[[phases]]
name = "test"
prompt = "{}"
runs_per_cycle = 1
"#,
        context_dir.display(),
        prompt_file.display()
    );

    let workflow = Workflow::from_str(&toml).unwrap();
    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();

    assert!(!plan.phases[0].signal_check.found);
}

#[test]
fn regex_mode_searches_for_literal_substring() {
    let temp_dir = TempDir::new().unwrap();
    let context_dir = temp_dir.path().join("context");
    fs::create_dir_all(&context_dir).unwrap();

    let prompt_file = temp_dir.path().join("prompts").join("test.md");
    fs::create_dir_all(prompt_file.parent().unwrap()).unwrap();
    fs::write(&prompt_file, "Line 1\nPattern: [a-z]+\nLine 3").unwrap();

    let toml = format!(
        r#"
[workflow]
completion_signal = "[a-z]+"
context_dir = "{}"
max_cycles = 10
completion_signal_mode = "regex"

[[phases]]
name = "test"
prompt = "{}"
runs_per_cycle = 1
"#,
        context_dir.display(),
        prompt_file.display()
    );

    let workflow = Workflow::from_str(&toml).unwrap();
    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();

    // Should find the literal string "[a-z]+", not match the regex
    assert!(plan.phases[0].signal_check.found);
    assert_eq!(plan.phases[0].signal_check.line_number, Some(2));
}

#[test]
fn multiple_phases_in_plan() {
    let (workflow, _temp) = create_test_workflow(
        "DONE",
        None,
        vec![
            ("phase1", 2, Some("p1.md"), None),
            ("phase2", 3, Some("p2.md"), None),
            ("phase3", 1, Some("p3.md"), None),
        ],
    );

    let plan = DryRunPlan::from_workflow(&workflow, "test.toml").unwrap();
    assert_eq!(plan.phases.len(), 3);
    assert_eq!(plan.phases[0].name, "phase1");
    assert_eq!(plan.phases[1].name, "phase2");
    assert_eq!(plan.phases[2].name, "phase3");
    assert_eq!(plan.runs_per_cycle_total, 6);
}
