/// Returns true if `signal` appears as a substring anywhere in `output`.
pub fn output_contains_signal(output: &str, signal: &str) -> bool {
    output.contains(signal)
}

/// Returns true if `signal` appears alone on a trimmed line in `output`.
pub fn output_line_contains_signal(output: &str, signal: &str) -> bool {
    output.lines().any(|line| line.trim() == signal)
}

/// Returns true if `regex` matches anywhere in `output`.
pub fn output_regex_matches_signal(output: &str, regex: &regex::Regex) -> bool {
    regex.is_match(output)
}

/// Returns true if `signal` appears in the prompt text (used for startup advisory check).
pub fn prompt_text_contains_signal(prompt: &str, signal: &str) -> bool {
    prompt.contains(signal)
}

/// Scan a list of prompt texts for the signal. Returns true if any contains it.
pub fn any_prompt_contains_signal(prompts: &[&str], signal: &str) -> bool {
    prompts
        .iter()
        .any(|p| prompt_text_contains_signal(p, signal))
}
