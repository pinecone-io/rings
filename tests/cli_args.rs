use clap::Parser;
use rings::cli::{Cli, Command};

#[test]
fn parses_run_command() {
    let cli = Cli::try_parse_from(["rings", "run", "workflow.toml"]).unwrap();
    match cli.command {
        Command::Run(args) => {
            assert_eq!(args.workflow_file, "workflow.toml");
            assert!(!args.verbose);
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn parses_run_with_flags() {
    let cli = Cli::try_parse_from([
        "rings",
        "run",
        "workflow.toml",
        "--verbose",
        "--max-cycles",
        "5",
        "--delay",
        "10",
    ])
    .unwrap();
    match cli.command {
        Command::Run(args) => {
            assert!(args.verbose);
            assert_eq!(args.max_cycles, Some(5));
            assert_eq!(args.delay, Some(10));
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn parses_resume_command() {
    let cli = Cli::try_parse_from(["rings", "resume", "run_20240315_143022_a1b2c3"]).unwrap();
    match cli.command {
        Command::Resume(args) => {
            assert_eq!(args.run_id, "run_20240315_143022_a1b2c3");
        }
        _ => panic!("expected Resume"),
    }
}

#[test]
fn run_requires_workflow_file() {
    let result = Cli::try_parse_from(["rings", "run"]);
    assert!(result.is_err());
}
