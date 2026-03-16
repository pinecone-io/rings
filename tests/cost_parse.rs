use rings::cost::{parse_cost_from_output, ParseConfidence};

// Reproduces bug: "Cost always displays as $0.000 in run output"
//
// The actual `claude --dangerously-skip-permissions -p -` binary does NOT output
// a "Cost: $..." line in its default (plain text) mode. Cost data is only available
// via `--output-format json`, which emits a JSON object with a `total_cost_usd` field.
//
// The current regex patterns only handle the plain-text "Cost: $X.XX" format;
// they cannot parse the JSON output format, so cost_usd is always None → $0.000.
#[test]
fn parses_cost_from_claude_json_output() {
    // Actual output from: echo "hello" | claude --dangerously-skip-permissions -p - --output-format json
    let output = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":3423,"duration_api_ms":3311,"num_turns":1,"result":"Hello!","stop_reason":"end_turn","session_id":"e452fc0b-2472-4e09-a3fb-704abe8d6df9","total_cost_usd":0.017429999999999998,"usage":{"input_tokens":2,"cache_creation_input_tokens":3290,"cache_read_input_tokens":16155,"output_tokens":16}}"#;
    let cost = parse_cost_from_output(output);
    // This currently fails: cost_usd is None because no regex matches the JSON field.
    assert!(
        cost.cost_usd.is_some(),
        "cost_usd should be parsed from JSON total_cost_usd field, got None"
    );
    assert!((cost.cost_usd.unwrap() - 0.01743).abs() < 1e-4);
}

// Reproduces the production bug: executor combines stdout+stderr with a newline,
// so `output.combined` is "{json}\n{stderr_text}" — not valid JSON.
// `serde_json::from_str` fails on the combined string, falling through to regex
// patterns that cannot match JSON field names, yielding cost_usd=None, confidence=None.
#[test]
fn parses_cost_from_combined_stdout_stderr() {
    // Simulate what ClaudeRunHandle.wait() produces:
    //   combined = format!("{stdout_str}\n{stderr_str}")
    // where stdout is the claude JSON response and stderr has diagnostic text.
    let stdout_json = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":3423,"duration_api_ms":3311,"num_turns":1,"result":"Hello!","stop_reason":"end_turn","session_id":"e452fc0b-2472-4e09-a3fb-704abe8d6df9","total_cost_usd":0.017429999999999998,"usage":{"input_tokens":2,"cache_creation_input_tokens":3290,"cache_read_input_tokens":16155,"output_tokens":16}}"#;
    let stderr_content = "API response received\n";
    let combined = format!("{stdout_json}\n{stderr_content}");

    let cost = parse_cost_from_output(&combined);
    assert!(
        cost.cost_usd.is_some(),
        "cost_usd should be parsed from JSON total_cost_usd even when stderr is present; got None (confidence={:?})",
        cost.confidence
    );
    assert!((cost.cost_usd.unwrap() - 0.01743).abs() < 1e-4);
    assert_eq!(cost.input_tokens, Some(2));
    assert_eq!(cost.output_tokens, Some(16));
}

#[test]
fn parses_standard_cost_line() {
    let output = "Some output\nCost: $0.0234 (1,234 input tokens, 567 output tokens)\nDone";
    let cost = parse_cost_from_output(output);
    assert!((cost.cost_usd.unwrap() - 0.0234).abs() < 1e-6);
    assert_eq!(cost.input_tokens, Some(1234));
    assert_eq!(cost.output_tokens, Some(567));
}

#[test]
fn returns_none_confidence_when_no_cost_line() {
    let result = parse_cost_from_output("No cost info here");
    assert_eq!(result.confidence, ParseConfidence::None);
    assert!(result.cost_usd.is_none());
}

#[test]
fn handles_large_numbers_with_commas() {
    let output = "Cost: $1.2345 (1,234,567 input tokens, 890,123 output tokens)";
    let cost = parse_cost_from_output(output);
    assert_eq!(cost.input_tokens, Some(1_234_567));
    assert_eq!(cost.output_tokens, Some(890_123));
}

#[test]
fn handles_cost_line_without_token_counts() {
    let output = "Cost: $0.50";
    let cost = parse_cost_from_output(output);
    assert!((cost.cost_usd.unwrap() - 0.50).abs() < 1e-6);
    assert_eq!(cost.input_tokens, None);
    assert_eq!(cost.output_tokens, None);
}

#[test]
fn uses_last_cost_line_when_multiple_present() {
    let output = "Cost: $0.01\nCost: $0.99";
    let cost = parse_cost_from_output(output);
    assert!((cost.cost_usd.unwrap() - 0.99).abs() < 1e-6);
}

#[test]
fn parses_total_cost_label() {
    let output = "Total cost: $0.75";
    let cost = parse_cost_from_output(output);
    assert!((cost.cost_usd.unwrap() - 0.75).abs() < 1e-6);
    assert_eq!(cost.confidence, ParseConfidence::Partial);
}

#[test]
fn falls_back_to_generic_dollar_pattern() {
    let output = "spent $0.12 on this run";
    let cost = parse_cost_from_output(output);
    assert!((cost.cost_usd.unwrap() - 0.12).abs() < 1e-6);
    assert_eq!(cost.confidence, ParseConfidence::Low);
}

#[test]
fn full_confidence_result_includes_token_counts() {
    let output = "Cost: $0.05 (1,000 input tokens, 500 output tokens)";
    let cost = parse_cost_from_output(output);
    assert_eq!(cost.confidence, ParseConfidence::Full);
    assert_eq!(cost.input_tokens, Some(1000));
}

#[test]
fn full_pattern_takes_priority_over_simple_pattern() {
    // Both Pattern 1 and Pattern 2 would match this line.
    // Pattern 1 (Full) should win.
    let output = "Cost: $0.05 (100 input tokens, 50 output tokens)";
    let result = parse_cost_from_output(output);
    assert_eq!(result.confidence, ParseConfidence::Full);
    assert_eq!(result.input_tokens, Some(100));
}
