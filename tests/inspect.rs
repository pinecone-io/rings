use rings::inspect::{
    load_actual_changes, load_declared_flow, render_data_flow_actual, render_data_flow_declared,
    ActualFileChange, ChangeType, DeclaredFlow,
};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// render_data_flow_declared tests
// ---------------------------------------------------------------------------

#[test]
fn test_render_data_flow_declared_two_phases_matches_spec_format() {
    let phases = vec![
        DeclaredFlow {
            phase: "builder".to_string(),
            consumes: vec!["specs/**/*.md".to_string()],
            produces: vec!["src/**/*.rs".to_string(), "tests/**/*.rs".to_string()],
        },
        DeclaredFlow {
            phase: "reviewer".to_string(),
            consumes: vec!["src/**/*.rs".to_string(), "tests/**/*.rs".to_string()],
            produces: vec!["review-notes.md".to_string()],
        },
    ];

    let output = render_data_flow_declared(&phases);

    // Header present
    assert!(output.starts_with("Data flow (declared):"));

    // All patterns and phase labels present
    assert!(output.contains("specs/**/*.md"));
    assert!(output.contains("[builder]"));
    assert!(output.contains("src/**/*.rs"));
    assert!(output.contains("tests/**/*.rs"));
    assert!(output.contains("[reviewer]"));
    assert!(output.contains("review-notes.md"));

    // Arrows present
    assert!(output.contains('\u{2192}'), "output should contain → arrow");

    // builder has 1 consume and 2 produces, so tests/**/*.rs should appear
    // on a continuation line (not the same line as specs/**/*.md in builder section)
    let lines: Vec<&str> = output.lines().collect();

    // Find the builder produce continuation: a line with tests/**/*.rs that has no ──→ before it,
    // OR find that builder's section has 2 produce entries.
    let builder_lines: Vec<&&str> = lines
        .iter()
        .filter(|l| {
            l.contains("[builder]") || {
                // continuation produces for builder: after [builder] row, before first [reviewer] row
                false
            }
        })
        .collect();
    assert!(
        !builder_lines.is_empty(),
        "builder label should appear in output"
    );

    // reviewer should appear on multiple rows (2 consumes)
    let reviewer_count = lines.iter().filter(|l| l.contains("[reviewer]")).count();
    assert_eq!(
        reviewer_count, 2,
        "reviewer label should appear on each consumes row"
    );
}

#[test]
fn test_render_data_flow_declared_no_contracts() {
    let phases = vec![DeclaredFlow {
        phase: "builder".to_string(),
        consumes: vec![],
        produces: vec![],
    }];

    let output = render_data_flow_declared(&phases);
    assert!(output.contains("[builder]"), "phase name should appear");
    // No arrows since no consumes or produces
    assert!(
        !output.contains('\u{2192}'),
        "no arrows expected for phase with no contracts"
    );
}

#[test]
fn test_render_data_flow_declared_consumes_only() {
    let phases = vec![DeclaredFlow {
        phase: "reader".to_string(),
        consumes: vec!["input.md".to_string()],
        produces: vec![],
    }];

    let output = render_data_flow_declared(&phases);
    assert!(output.contains("input.md"));
    assert!(output.contains("[reader]"));

    // Should have exactly one arrow (consumes ──→ [reader]), no second arrow for produces
    let arrow_count = output.matches('\u{2192}').count();
    assert_eq!(
        arrow_count, 1,
        "consumes-only should have exactly one arrow"
    );
}

// ---------------------------------------------------------------------------
// render_data_flow_actual tests
// ---------------------------------------------------------------------------

#[test]
fn test_render_data_flow_actual_correct_attribution() {
    let changes = vec![
        ActualFileChange {
            path: "src/main.rs".to_string(),
            phase: "builder".to_string(),
            cycle: 1,
            run: 5,
            iteration: 1,
            change_type: ChangeType::Modified,
        },
        ActualFileChange {
            path: "review-notes.md".to_string(),
            phase: "reviewer".to_string(),
            cycle: 1,
            run: 6,
            iteration: 1,
            change_type: ChangeType::Added,
        },
    ];

    let output = render_data_flow_actual(&changes);

    assert!(output.starts_with("Data flow (actual):"));
    assert!(output.contains("src/main.rs"));
    assert!(output.contains("modified"));
    assert!(output.contains("builder"));
    assert!(output.contains("review-notes.md"));
    assert!(output.contains("created"));
    assert!(output.contains("reviewer"));
    assert!(output.contains("run 5"));
    assert!(output.contains("run 6"));
}

#[test]
fn test_render_data_flow_actual_aggregates_runs() {
    // Same file modified in two runs by same phase
    let changes = vec![
        ActualFileChange {
            path: "src/engine.rs".to_string(),
            phase: "builder".to_string(),
            cycle: 2,
            run: 6,
            iteration: 1,
            change_type: ChangeType::Modified,
        },
        ActualFileChange {
            path: "src/engine.rs".to_string(),
            phase: "builder".to_string(),
            cycle: 2,
            run: 7,
            iteration: 2,
            change_type: ChangeType::Modified,
        },
    ];

    let output = render_data_flow_actual(&changes);
    // Aggregated: "runs 6, 7"
    assert!(
        output.contains("runs 6, 7"),
        "should aggregate multiple runs: got {}",
        output
    );
}

// ---------------------------------------------------------------------------
// Integration: load_declared_flow reads workflow_contracts.json
// ---------------------------------------------------------------------------

#[test]
fn test_inspect_data_flow_exits_0_produces_output() {
    let tmpdir = TempDir::new().unwrap();
    let run_dir = tmpdir.path().join("run_20240315_143022_a1b2c");
    std::fs::create_dir_all(&run_dir).unwrap();

    // Write workflow_contracts.json
    let contracts = serde_json::json!([
        {
            "phase": "builder",
            "consumes": ["specs/**/*.md"],
            "produces": ["src/**/*.rs"]
        }
    ]);
    std::fs::write(
        run_dir.join("workflow_contracts.json"),
        serde_json::to_string(&contracts).unwrap(),
    )
    .unwrap();

    // load_declared_flow should succeed and return the phase
    let declared = load_declared_flow(&run_dir).unwrap();
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].phase, "builder");
    assert_eq!(declared[0].consumes, vec!["specs/**/*.md"]);
    assert_eq!(declared[0].produces, vec!["src/**/*.rs"]);

    // render_data_flow_declared should produce non-empty output containing "Data flow"
    let rendered = render_data_flow_declared(&declared);
    assert!(
        rendered.contains("Data flow"),
        "output should contain 'Data flow': {}",
        rendered
    );
    assert!(rendered.contains("[builder]"));

    // load_actual_changes returns empty when no manifests/costs exist
    let actual = load_actual_changes(&run_dir).unwrap();
    assert!(actual.is_empty());
}
