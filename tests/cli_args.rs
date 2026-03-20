use clap::Parser;
use rings::cli::{Cli, Command, InitArgs, ListArgs};

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

#[test]
fn parses_init_with_no_args() {
    let cli = Cli::try_parse_from(["rings", "init"]).unwrap();
    match cli.command {
        Command::Init(InitArgs { name, force }) => {
            assert!(name.is_none());
            assert!(!force);
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parses_init_with_name() {
    let cli = Cli::try_parse_from(["rings", "init", "my-task"]).unwrap();
    match cli.command {
        Command::Init(InitArgs { name, .. }) => {
            assert_eq!(name.as_deref(), Some("my-task"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parses_init_with_force_flag() {
    let cli = Cli::try_parse_from(["rings", "init", "--force"]).unwrap();
    match cli.command {
        Command::Init(InitArgs { force, .. }) => {
            assert!(force);
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parses_run_with_cycle_delay() {
    let cli =
        Cli::try_parse_from(["rings", "run", "workflow.toml", "--cycle-delay", "60"]).unwrap();
    match cli.command {
        Command::Run(args) => {
            assert_eq!(args.cycle_delay, Some(60));
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn parses_run_cycle_delay_zero() {
    let cli = Cli::try_parse_from(["rings", "run", "workflow.toml", "--cycle-delay", "0"]).unwrap();
    match cli.command {
        Command::Run(args) => {
            assert_eq!(args.cycle_delay, Some(0));
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn parses_run_without_cycle_delay_uses_none() {
    let cli = Cli::try_parse_from(["rings", "run", "workflow.toml"]).unwrap();
    match cli.command {
        Command::Run(args) => {
            assert!(args.cycle_delay.is_none());
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn parses_update_command() {
    let cli = Cli::try_parse_from(["rings", "update"]).unwrap();
    match cli.command {
        Command::Update => {}
        _ => panic!("expected Update"),
    }
}

#[test]
fn parses_list_with_dir_flag() {
    let cli = Cli::try_parse_from(["rings", "list", "--dir", "/foo/bar"]).unwrap();
    match cli.command {
        Command::List(ListArgs { dir, .. }) => {
            assert_eq!(dir.as_deref(), Some("/foo/bar"));
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn parses_list_without_dir_flag() {
    let cli = Cli::try_parse_from(["rings", "list"]).unwrap();
    match cli.command {
        Command::List(ListArgs { dir, .. }) => {
            assert!(dir.is_none());
        }
        _ => panic!("expected List"),
    }
}
