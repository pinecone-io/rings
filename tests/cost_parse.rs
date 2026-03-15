use rings::cost::{parse_cost_from_output, ParseConfidence};

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
