/// Integration tests for the startup unknown template variable advisory warning (F-029).
///
/// The warning must appear on stderr in Human output mode when a prompt contains
/// an unrecognized `{{variable}}`. In JSONL mode the warning must be suppressed.
use std::io::Write;
use tempfile::NamedTempFile;

fn write_workflow(prompt_text: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    write!(
        f,
        r#"[workflow]
completion_signal = "RINGS_DONE"
context_dir = "/tmp"
max_cycles = 1

[executor]
binary = "/usr/bin/true"

[[phases]]
name = "builder"
prompt_text = {prompt_text:?}
runs_per_cycle = 1
"#
    )
    .unwrap();
    f
}

#[test]
fn unknown_var_triggers_stderr_warning() {
    let wf = write_workflow("Do work with {{unknown_var}} here. RINGS_DONE");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rings"))
        .args(["run", wf.path().to_str().unwrap()])
        .output()
        .expect("failed to spawn rings");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown template variable '{{unknown_var}}'"),
        "stderr must contain the unknown-var warning; got:\n{stderr}"
    );
    assert!(
        stderr.contains("phase \"builder\""),
        "warning must name the phase; got:\n{stderr}"
    );
}

#[test]
fn known_vars_only_produces_no_unknown_var_warning() {
    let wf = write_workflow(
        "Phase {{phase_name}} cycle {{cycle}} of {{max_cycles}} run {{run}}. RINGS_DONE",
    );

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rings"))
        .args(["run", wf.path().to_str().unwrap()])
        .output()
        .expect("failed to spawn rings");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Unknown template variable"),
        "no unknown-var warning expected; got:\n{stderr}"
    );
}

#[test]
fn jsonl_mode_suppresses_unknown_var_warning() {
    let wf = write_workflow("Do work with {{unknown_var}} here. RINGS_DONE");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rings"))
        .args([
            "run",
            wf.path().to_str().unwrap(),
            "--output-format",
            "jsonl",
        ])
        .output()
        .expect("failed to spawn rings");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Unknown template variable"),
        "JSONL mode must suppress the unknown-var warning; got stderr:\n{stderr}"
    );
}
