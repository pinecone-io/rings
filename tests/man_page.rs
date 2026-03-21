/// Tests for man page generation via clap_mangen.
///
/// These tests generate the man page from the actual `Cli::command()` at test time,
/// verifying that the output is valid roff and contains expected content.
use clap::CommandFactory;
use clap_mangen::Man;
use rings::cli::Cli;

fn generate_man_page(cmd: clap::Command) -> String {
    let mut buf = Vec::new();
    Man::new(cmd)
        .render(&mut buf)
        .expect("man page render failed");
    String::from_utf8(buf).expect("man page is not valid UTF-8")
}

/// Unescape roff hyphens (`\-` → `-`) so we can search for flag names literally.
fn unescape_roff(s: &str) -> String {
    s.replace("\\-", "-")
}

#[test]
fn man_page_is_valid_roff() {
    let cmd = Cli::command();
    let page = generate_man_page(cmd);

    // roff man pages contain a .TH macro (may not be the very first line — clap_mangen
    // emits compatibility preamble before it)
    assert!(
        page.contains(".TH"),
        "expected man page to contain .TH macro"
    );

    // Must contain at least one section header
    assert!(
        page.contains(".SH"),
        "expected man page to contain .SH section headers"
    );
}

#[test]
fn man_page_contains_top_level_name() {
    let cmd = Cli::command();
    let page = generate_man_page(cmd);

    assert!(
        page.contains("rings"),
        "expected man page to contain the binary name 'rings'"
    );
}

#[test]
fn man_page_includes_all_subcommands() {
    let cmd = Cli::command();
    let page = generate_man_page(cmd);

    for sub in &["run", "resume", "list", "show", "inspect", "lineage"] {
        assert!(
            page.contains(sub),
            "expected man page to mention subcommand '{sub}'"
        );
    }
}

#[test]
fn man_page_includes_global_flags() {
    let cmd = Cli::command();
    let page = unescape_roff(&generate_man_page(cmd));

    assert!(
        page.contains("output-format") || page.contains("output_format"),
        "expected man page to mention the output-format flag"
    );
    assert!(
        page.contains("no-color") || page.contains("no_color"),
        "expected man page to mention the no-color flag"
    );
}

#[test]
fn subcommand_run_man_page_is_valid() {
    let parent = Cli::command();
    let run_sub = parent
        .get_subcommands()
        .find(|s| s.get_name() == "run")
        .expect("run subcommand not found")
        .clone();

    let raw = generate_man_page(run_sub.name("rings-run"));
    let page = unescape_roff(&raw);

    // The page must be valid roff
    assert!(raw.contains(".TH"), "rings-run man page missing .TH macro");

    // Key flags from the run subcommand should appear (after unescaping roff hyphens)
    for flag in &["max-cycles", "verbose", "budget-cap", "dry-run"] {
        assert!(
            page.contains(flag) || page.contains(&flag.replace('-', "_")),
            "expected rings-run man page to mention flag '{flag}'"
        );
    }
}
