// Tests for --include-dir / -I flag (F-025)
use clap::Parser;
use rings::cancel::CancelState;
use rings::cli::{Cli, Command};
use rings::engine::{build_include_dir_preamble, run_workflow, EngineConfig};
use rings::executor::{ExecutorOutput, MockExecutor};
use rings::workflow::{CompiledErrorProfile, CompletionSignalMode, PhaseConfig, Workflow};
use std::sync::Arc;
use tempfile::tempdir;

// ─── CLI parsing tests ─────────────────────────────────────────────────────

#[test]
fn parses_single_include_dir() {
    let cli = Cli::try_parse_from(["rings", "run", "-I", "./specs", "workflow.toml"]).unwrap();
    match cli.command {
        Command::Run(args) => {
            assert_eq!(args.include_dir, vec!["./specs"]);
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn parses_multiple_include_dirs() {
    let cli = Cli::try_parse_from([
        "rings",
        "run",
        "-I",
        "./specs",
        "-I",
        "./docs",
        "workflow.toml",
    ])
    .unwrap();
    match cli.command {
        Command::Run(args) => {
            assert_eq!(args.include_dir, vec!["./specs", "./docs"]);
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn include_dir_long_form_parses() {
    let cli = Cli::try_parse_from([
        "rings",
        "run",
        "--include-dir",
        "./context",
        "workflow.toml",
    ])
    .unwrap();
    match cli.command {
        Command::Run(args) => {
            assert_eq!(args.include_dir, vec!["./context"]);
        }
        _ => panic!("expected Run"),
    }
}

// ─── Binary-level validation tests ────────────────────────────────────────

#[test]
fn nonexistent_include_dir_exits_2() {
    use std::io::Write;
    use tempfile::NamedTempFile;

    let mut wf = NamedTempFile::new().unwrap();
    write!(
        wf,
        r#"[workflow]
completion_signal = "DONE"
context_dir = "/tmp"
max_cycles = 1

[executor]
binary = "/usr/bin/true"

[[phases]]
name = "builder"
prompt_text = "do work"
runs_per_cycle = 1
"#
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rings"))
        .args([
            "run",
            "-I",
            "/nonexistent/path/that/does/not/exist",
            wf.path().to_str().unwrap(),
        ])
        .output()
        .expect("failed to spawn rings");

    assert_eq!(
        output.status.code(),
        Some(2),
        "should exit with code 2 for nonexistent include-dir"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--include-dir"),
        "stderr should mention --include-dir; got:\n{stderr}"
    );
}

// ─── Preamble builder unit tests ──────────────────────────────────────────

#[test]
fn preamble_lists_files_from_directory() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "content").unwrap();
    std::fs::write(dir.path().join("b.txt"), "content").unwrap();

    let preamble = build_include_dir_preamble(&[dir.path().to_path_buf()]);

    assert!(
        preamble.starts_with("The following context files are available for reference:"),
        "preamble should have the header"
    );
    assert!(preamble.contains("a.md"), "should list a.md");
    assert!(preamble.contains("b.txt"), "should list b.txt");
}

#[test]
fn preamble_empty_dir_has_no_file_entries() {
    let dir = tempdir().unwrap();

    let preamble = build_include_dir_preamble(&[dir.path().to_path_buf()]);

    // Should contain only the header line with no file entries
    let lines: Vec<&str> = preamble.lines().collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0],
        "The following context files are available for reference:"
    );
}

#[test]
fn preamble_is_non_recursive() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("top.md"), "top").unwrap();
    // Create a subdirectory with a file — should not appear in preamble
    let subdir = dir.path().join("sub");
    std::fs::create_dir(&subdir).unwrap();
    std::fs::write(subdir.join("nested.md"), "nested").unwrap();

    let preamble = build_include_dir_preamble(&[dir.path().to_path_buf()]);

    assert!(preamble.contains("top.md"), "should list top-level file");
    assert!(
        !preamble.contains("nested.md"),
        "should not list nested files"
    );
}

// ─── Engine integration: prompt receives preamble ─────────────────────────

fn default_compiled_error_profile() -> CompiledErrorProfile {
    CompiledErrorProfile {
        quota_regexes: vec![],
        auth_regexes: vec![],
    }
}

#[cfg(feature = "testing")]
#[test]
fn engine_prepends_preamble_when_include_dirs_set() {
    use std::sync::Mutex;

    let output_dir = tempdir().unwrap();
    let include_dir = tempdir().unwrap();
    std::fs::write(include_dir.path().join("context.md"), "context").unwrap();

    let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
    let captured_clone = Arc::clone(&captured);

    let workflow = Workflow {
        completion_signal: "DONE".to_string(),
        continue_signal: None,
        completion_signal_phases: vec![],
        completion_signal_mode: CompletionSignalMode::Substring,
        context_dir: ".".to_string(),
        max_cycles: 1,
        output_dir: None,
        delay_between_runs: 0,
        delay_between_cycles: 0,
        executor: None,
        budget_cap_usd: None,
        timeout_per_run_secs: None,
        compiled_error_profile: default_compiled_error_profile(),
        quota_backoff: false,
        quota_backoff_delay: 0,
        quota_backoff_max_retries: 0,
        manifest_enabled: false,
        manifest_ignore: vec![],
        manifest_mtime_optimization: false,
        snapshot_cycles: false,
        phases: vec![PhaseConfig {
            name: "builder".to_string(),
            prompt: None,
            prompt_text: Some("do work".to_string()),
            runs_per_cycle: 1,
            budget_cap_usd: None,
            timeout_per_run_secs: None,
            consumes: vec![],
            produces: vec![],
            produces_required: false,
            executor: None,
        }],
    };

    let executor = MockExecutor::with_side_effect(
        vec![ExecutorOutput {
            combined: "DONE".to_string(),
            exit_code: 0,
        }],
        move |inv| {
            captured_clone.lock().unwrap().push(inv.prompt.clone());
        },
    );

    let config = EngineConfig {
        output_dir: output_dir.path().to_path_buf(),
        run_id: "test-include-dir".to_string(),
        workflow_file: "test.rings.toml".to_string(),
        include_dirs: vec![include_dir.path().to_path_buf()],
        ..Default::default()
    };

    run_workflow(
        &workflow,
        &executor,
        &config,
        None,
        Some(Arc::new(rings::cancel::CancelState::new())),
    )
    .unwrap();

    let prompts = captured.lock().unwrap();
    assert_eq!(prompts.len(), 1);
    assert!(
        prompts[0].starts_with("The following context files are available for reference:"),
        "prompt should start with preamble header"
    );
    assert!(
        prompts[0].contains("context.md"),
        "preamble should list context.md"
    );
    assert!(
        prompts[0].contains("do work"),
        "original prompt should follow preamble"
    );
}
