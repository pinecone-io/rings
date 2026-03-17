fn create_test_workflow(phases: &[(&str, u32)]) -> String {
    let mut phase_toml = String::new();
    for (name, runs_per_cycle) in phases {
        phase_toml.push_str(&format!(
            r#"
[[phases]]
name = "{}"
prompt_text = "test prompt for {}"
runs_per_cycle = {}
"#,
            name, name, runs_per_cycle
        ));
    }

    format!(
        r#"[workflow]
completion_signal = "DONE"
context_dir = "."
max_cycles = 5
{}
"#,
        phase_toml
    )
}

#[test]
fn test_structural_fingerprint() {
    // Test that structural_fingerprint returns phase names in order
    let workflow_toml = create_test_workflow(&[("phase_a", 1), ("phase_b", 2), ("phase_c", 1)]);
    let workflow: rings::workflow::Workflow = workflow_toml.parse().unwrap();
    let fingerprint = workflow.structural_fingerprint();
    assert_eq!(fingerprint, vec!["phase_a", "phase_b", "phase_c"]);
}

#[test]
fn test_runmeta_with_phase_fingerprint() {
    // Test that RunMeta can store and serialize phase_fingerprint
    let meta = rings::state::RunMeta {
        run_id: "test_run".to_string(),
        workflow_file: "/path/to/workflow.toml".to_string(),
        started_at: "2024-01-01T00:00:00Z".to_string(),
        rings_version: "0.1.0".to_string(),
        status: "running".to_string(),
        phase_fingerprint: Some(vec!["phase_a".to_string(), "phase_b".to_string()]),
    };

    let toml_str = toml::to_string(&meta).unwrap();
    let parsed: rings::state::RunMeta = toml::from_str(&toml_str).unwrap();
    assert_eq!(
        parsed.phase_fingerprint,
        Some(vec!["phase_a".to_string(), "phase_b".to_string()])
    );
}

#[test]
fn test_runmeta_without_phase_fingerprint() {
    // Test that old RunMeta without phase_fingerprint deserializes correctly
    let toml_str = r#"
run_id = "test_run"
workflow_file = "/path/to/workflow.toml"
started_at = "2024-01-01T00:00:00Z"
rings_version = "0.1.0"
status = "running"
"#;
    let parsed: rings::state::RunMeta = toml::from_str(toml_str).unwrap();
    assert_eq!(parsed.phase_fingerprint, None);
}

#[test]
fn test_identical_fingerprints_no_change() {
    // Test that identical fingerprints indicate no structural changes
    let phases = vec![("phase_a", 1), ("phase_b", 2)];
    let workflow_toml = create_test_workflow(&phases);
    let workflow: rings::workflow::Workflow = workflow_toml.parse().unwrap();
    let fingerprint1 = workflow.structural_fingerprint();

    let workflow_toml2 = create_test_workflow(&phases);
    let workflow2: rings::workflow::Workflow = workflow_toml2.parse().unwrap();
    let fingerprint2 = workflow2.structural_fingerprint();

    assert_eq!(fingerprint1, fingerprint2);
}

#[test]
fn test_added_phase_structural_change() {
    // Test detection of added phase
    let workflow_toml1 = create_test_workflow(&[("phase_a", 1), ("phase_b", 2)]);
    let workflow1: rings::workflow::Workflow = workflow_toml1.parse().unwrap();
    let fingerprint1 = workflow1.structural_fingerprint();

    let workflow_toml2 = create_test_workflow(&[("phase_a", 1), ("phase_b", 2), ("phase_c", 1)]);
    let workflow2: rings::workflow::Workflow = workflow_toml2.parse().unwrap();
    let fingerprint2 = workflow2.structural_fingerprint();

    assert!(fingerprint1 != fingerprint2);
    assert_eq!(fingerprint1.len(), 2);
    assert_eq!(fingerprint2.len(), 3);
}

#[test]
fn test_removed_phase_structural_change() {
    // Test detection of removed phase
    let workflow_toml1 = create_test_workflow(&[("phase_a", 1), ("phase_b", 2), ("phase_c", 1)]);
    let workflow1: rings::workflow::Workflow = workflow_toml1.parse().unwrap();
    let fingerprint1 = workflow1.structural_fingerprint();

    let workflow_toml2 = create_test_workflow(&[("phase_a", 1), ("phase_c", 1)]);
    let workflow2: rings::workflow::Workflow = workflow_toml2.parse().unwrap();
    let fingerprint2 = workflow2.structural_fingerprint();

    assert!(fingerprint1 != fingerprint2);
    assert_eq!(fingerprint1.len(), 3);
    assert_eq!(fingerprint2.len(), 2);
}

#[test]
fn test_reordered_phases_structural_change() {
    // Test detection of reordered phases
    let workflow_toml1 = create_test_workflow(&[("phase_a", 1), ("phase_b", 2), ("phase_c", 1)]);
    let workflow1: rings::workflow::Workflow = workflow_toml1.parse().unwrap();
    let fingerprint1 = workflow1.structural_fingerprint();

    let workflow_toml2 = create_test_workflow(&[("phase_c", 1), ("phase_a", 1), ("phase_b", 2)]);
    let workflow2: rings::workflow::Workflow = workflow_toml2.parse().unwrap();
    let fingerprint2 = workflow2.structural_fingerprint();

    assert!(fingerprint1 != fingerprint2);
    assert_eq!(fingerprint1.len(), 3);
    assert_eq!(fingerprint2.len(), 3);
    // Different order
    assert_eq!(fingerprint1, vec!["phase_a", "phase_b", "phase_c"]);
    assert_eq!(fingerprint2, vec!["phase_c", "phase_a", "phase_b"]);
}

#[test]
fn test_runs_per_cycle_non_structural_change() {
    // Test that runs_per_cycle change doesn't affect structural fingerprint
    let workflow_toml1 = create_test_workflow(&[("phase_a", 1), ("phase_b", 2)]);
    let workflow1: rings::workflow::Workflow = workflow_toml1.parse().unwrap();
    let fingerprint1 = workflow1.structural_fingerprint();

    let workflow_toml2 = create_test_workflow(&[("phase_a", 3), ("phase_b", 4)]);
    let workflow2: rings::workflow::Workflow = workflow_toml2.parse().unwrap();
    let fingerprint2 = workflow2.structural_fingerprint();

    // Fingerprints should be identical (runs_per_cycle is non-structural)
    assert_eq!(fingerprint1, fingerprint2);
    // But the phase objects themselves differ
    assert_ne!(
        workflow1.phases[0].runs_per_cycle,
        workflow2.phases[0].runs_per_cycle
    );
}
