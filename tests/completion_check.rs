use rings::completion::{
    any_prompt_contains_signal, output_contains_signal, prompt_text_contains_signal,
};

#[test]
fn detects_signal_in_output() {
    assert!(output_contains_signal(
        "some output\nRINGS_DONE\nmore",
        "RINGS_DONE"
    ));
}

#[test]
fn superstring_of_signal_is_a_match() {
    // output_contains_signal uses substring matching, so a superstring of the signal
    // (e.g. "RINGS_DONE_EXTRA") is still a positive match — this is intentional.
    assert!(output_contains_signal("RINGS_DONE_EXTRA", "RINGS_DONE"));
}

#[test]
fn signal_not_present_returns_false() {
    assert!(!output_contains_signal(
        "all done but no signal",
        "RINGS_DONE"
    ));
}

#[test]
fn detects_signal_in_prompt_text() {
    let prompt = "When done, print RINGS_DONE";
    assert!(prompt_text_contains_signal(prompt, "RINGS_DONE"));
}

#[test]
fn signal_absent_from_prompt_text() {
    assert!(!prompt_text_contains_signal("do something", "RINGS_DONE"));
}

#[test]
fn any_prompt_contains_signal_true_when_one_matches() {
    let prompts = [
        "no signal here",
        "print RINGS_DONE when done",
        "also nothing",
    ];
    assert!(any_prompt_contains_signal(&prompts, "RINGS_DONE"));
}

#[test]
fn any_prompt_contains_signal_false_when_none_match() {
    let prompts = ["no signal", "also no signal"];
    assert!(!any_prompt_contains_signal(&prompts, "RINGS_DONE"));
}
