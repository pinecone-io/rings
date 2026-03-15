// Run with: cargo test --features testing
use rings::engine::{run_workflow, EngineConfig};
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::workflow::Workflow;
use tempfile::tempdir;

fn make_workflow(signal: &str, phases: &[(&str, u32)], max_cycles: u32) -> Workflow {
    use rings::workflow::PhaseConfig;
    Workflow {
        completion_signal: signal.to_string(),
        context_dir: ".".to_string(),
        max_cycles,
        output_dir: None,
        delay_between_runs: 0,
        phases: phases
            .iter()
            .map(|(name, runs)| PhaseConfig {
                name: name.to_string(),
                prompt: None,
                prompt_text: Some(format!("do work, signal={signal}")),
                runs_per_cycle: *runs,
            })
            .collect(),
    }
}

#[test]
fn engine_exits_zero_on_completion_signal() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("RINGS_DONE", &[("builder", 1)], 10);
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "working...".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "RINGS_DONE".to_string(),
            exit_code: 0,
        },
    ]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        verbose: false,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.completed_cycles, 2);
}

#[test]
fn engine_exits_one_when_max_cycles_reached() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("RINGS_DONE", &[("builder", 1)], 3);
    // Never emit signal
    let outputs: Vec<_> = (0..3)
        .map(|_| ExecutorOutput {
            combined: "no signal".to_string(),
            exit_code: 0,
        })
        .collect();
    let executor = MockExecutor::new(outputs);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        verbose: false,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 1);
}

#[test]
fn engine_writes_run_logs() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "DONE".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        verbose: false,
    };

    run_workflow(&workflow, &executor, &config, None, None).unwrap();

    // Log file for run 1 must exist
    let log_path = dir.path().join("runs").join("001.log");
    assert!(
        log_path.exists(),
        "run log not written: {}",
        log_path.display()
    );
}

#[test]
fn engine_writes_costs_jsonl() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "Cost: $0.05\nDONE".to_string(),
        exit_code: 0,
    }]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        verbose: false,
    };

    run_workflow(&workflow, &executor, &config, None, None).unwrap();

    let costs_path = dir.path().join("costs.jsonl");
    assert!(costs_path.exists());
    let content = std::fs::read_to_string(&costs_path).unwrap();
    assert!(content.contains("\"cost_usd\""));
}

#[test]
fn engine_classifies_nonzero_exit_as_error() {
    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 5);
    let executor = MockExecutor::new(vec![ExecutorOutput {
        combined: "quota exceeded".to_string(),
        exit_code: 1,
    }]);
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        verbose: false,
    };

    let result = run_workflow(&workflow, &executor, &config, None, None).unwrap();
    assert_eq!(result.exit_code, 3);
}

#[test]
fn engine_saves_state_and_exits_130_on_cancel() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let workflow = make_workflow("DONE", &[("builder", 1)], 10);
    // First run succeeds, second run triggers cancellation
    let executor = MockExecutor::new(vec![
        ExecutorOutput {
            combined: "run 1 output".to_string(),
            exit_code: 0,
        },
        ExecutorOutput {
            combined: "run 2 output".to_string(),
            exit_code: 0,
        },
    ]);
    let canceled = Arc::new(AtomicBool::new(false));
    let canceled_clone = canceled.clone();
    let config = EngineConfig {
        output_dir: dir.path().to_path_buf(),
        verbose: false,
    };

    // Set the cancel flag immediately (test simplicity)
    canceled_clone.store(true, Ordering::SeqCst);

    let result = run_workflow(&workflow, &executor, &config, None, Some(canceled)).unwrap();
    assert_eq!(
        result.exit_code, 130,
        "exit code should be 130 on cancellation"
    );

    // state.json must exist
    let state_path = dir.path().join("state.json");
    assert!(state_path.exists(), "state.json must be saved on cancel");
}
