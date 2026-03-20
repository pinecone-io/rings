use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

/// Compiled cost parser profile — the resolved, ready-to-use form stored on `Workflow`.
#[derive(Debug, Clone)]
pub enum CompiledCostParser {
    /// Use the built-in claude-code cascade of patterns (default).
    ClaudeCode,
    /// Skip cost extraction entirely; all fields are null.
    None,
    /// Use a custom regex with named captures: `cost_usd` (required),
    /// `input_tokens` and `output_tokens` (optional).
    Custom(regex::Regex),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParseConfidence {
    Full,    // matched full pattern with dollar amount + token counts
    Partial, // matched dollar amount, no token counts
    Low,     // matched generic fallback pattern only
    None,    // no match at all
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunCost {
    pub cost_usd: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub confidence: ParseConfidence,
    pub raw_match: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseWarning {
    pub run_number: u32,
    pub cycle: u32,
    pub phase: String,
    pub confidence: ParseConfidence,
    pub raw_match: Option<String>,
}

impl Default for RunCost {
    fn default() -> Self {
        Self {
            cost_usd: None,
            input_tokens: None,
            output_tokens: None,
            confidence: ParseConfidence::None,
            raw_match: None,
        }
    }
}

lazy_static! {
    // Pattern 1 (Full confidence): Cost: $X.XX (N,NNN input tokens, M,MMM output tokens)
    static ref RE_FULL: regex::Regex = regex::Regex::new(
        r"Cost: \$(\d+\.\d+)\s*\(([0-9,]+) input tokens,\s*([0-9,]+) output tokens\)"
    ).unwrap(); // Safe: compile-time constant regex

    // Pattern 2 (Partial confidence): Cost: $X.XX
    static ref RE_SIMPLE: regex::Regex = regex::Regex::new(r"Cost: \$(\d+\.\d+)").unwrap();

    // Pattern 3 (Partial confidence): Total cost: $X.XX
    static ref RE_TOTAL: regex::Regex = regex::Regex::new(r"[Tt]otal cost: \$(\d+\.\d+)").unwrap();

    // Pattern 4 (Low confidence): any $X.XX
    static ref RE_GENERIC: regex::Regex = regex::Regex::new(r"\$(\d+\.\d+)").unwrap();
}

/// Returns `true` if `cost` is a valid (non-negative, finite) cost value.
fn is_valid_cost(cost: f64) -> bool {
    cost.is_finite() && cost >= 0.0
}

/// Wraps a parsed cost value: returns `Some(cost)` when valid, `None` otherwise.
/// The returned `Option<String>` carries a diagnostic raw_match string for invalid values.
fn validated_cost(cost: f64, source: &str) -> (Option<f64>, Option<String>) {
    if is_valid_cost(cost) {
        (Some(cost), Some(source.to_string()))
    } else {
        (
            None,
            Some(format!(
                "invalid cost value ({cost}) rejected from: {source}"
            )),
        )
    }
}

pub fn parse_cost_from_output(output: &str) -> RunCost {
    let parse_tokens = |s: &str| -> Option<u64> { s.replace(',', "").parse().ok() };

    // Try JSON output format first (`--output-format json` emits a single JSON object
    // with `total_cost_usd` and a nested `usage` object).
    // Scan line-by-line so that stderr appended after the JSON line doesn't break parsing.
    for line in output.lines() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            if let Some(cost_usd) = v.get("total_cost_usd").and_then(|c| c.as_f64()) {
                let (validated, raw_match) =
                    validated_cost(cost_usd, &format!("total_cost_usd:{cost_usd}"));
                if validated.is_none() {
                    return RunCost {
                        cost_usd: None,
                        input_tokens: None,
                        output_tokens: None,
                        confidence: ParseConfidence::None,
                        raw_match,
                    };
                }
                let usage = v.get("usage");
                let input_tokens = usage
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(|t| t.as_u64());
                let output_tokens = usage
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(|t| t.as_u64());
                return RunCost {
                    cost_usd: validated,
                    input_tokens,
                    output_tokens,
                    confidence: ParseConfidence::Full,
                    raw_match,
                };
            }
        }
    }

    // Try patterns in order, use last match of highest-confidence pattern found
    if let Some(caps) = RE_FULL.captures_iter(output).last() {
        let raw = caps[0].to_string();
        let parsed: Option<f64> = caps[1].parse().ok();
        let (cost_usd, raw_match) = match parsed {
            Some(v) => validated_cost(v, &raw),
            None => (None, Some(raw)),
        };
        let confidence = if cost_usd.is_some() {
            ParseConfidence::Full
        } else {
            ParseConfidence::None
        };
        return RunCost {
            cost_usd,
            input_tokens: caps.get(2).and_then(|m| parse_tokens(m.as_str())),
            output_tokens: caps.get(3).and_then(|m| parse_tokens(m.as_str())),
            confidence,
            raw_match,
        };
    }

    if let Some(caps) = RE_SIMPLE.captures_iter(output).last() {
        let raw = caps[0].to_string();
        let parsed: Option<f64> = caps[1].parse().ok();
        let (cost_usd, raw_match) = match parsed {
            Some(v) => validated_cost(v, &raw),
            None => (None, Some(raw)),
        };
        let confidence = if cost_usd.is_some() {
            ParseConfidence::Partial
        } else {
            ParseConfidence::None
        };
        return RunCost {
            cost_usd,
            input_tokens: None,
            output_tokens: None,
            confidence,
            raw_match,
        };
    }

    if let Some(caps) = RE_TOTAL.captures_iter(output).last() {
        let raw = caps[0].to_string();
        let parsed: Option<f64> = caps[1].parse().ok();
        let (cost_usd, raw_match) = match parsed {
            Some(v) => validated_cost(v, &raw),
            None => (None, Some(raw)),
        };
        let confidence = if cost_usd.is_some() {
            ParseConfidence::Partial
        } else {
            ParseConfidence::None
        };
        return RunCost {
            cost_usd,
            input_tokens: None,
            output_tokens: None,
            confidence,
            raw_match,
        };
    }

    if let Some(caps) = RE_GENERIC.captures_iter(output).last() {
        let raw = caps[0].to_string();
        let parsed: Option<f64> = caps[1].parse().ok();
        let (cost_usd, raw_match) = match parsed {
            Some(v) => validated_cost(v, &raw),
            None => (None, Some(raw)),
        };
        let confidence = if cost_usd.is_some() {
            ParseConfidence::Low
        } else {
            ParseConfidence::None
        };
        return RunCost {
            cost_usd,
            input_tokens: None,
            output_tokens: None,
            confidence,
            raw_match,
        };
    }

    RunCost {
        cost_usd: None,
        input_tokens: None,
        output_tokens: None,
        confidence: ParseConfidence::None,
        raw_match: None,
    }
}

/// Parse cost from executor output using the specified `CompiledCostParser`.
///
/// - `ClaudeCode` → delegates to the built-in cascade (`parse_cost_from_output`)
/// - `None` → returns a zero/null `RunCost` without any parsing or warnings
/// - `Custom(re)` → tries the regex once; named captures `cost_usd` (required),
///   `input_tokens` and `output_tokens` (optional). No fallthrough to built-in patterns.
pub fn parse_cost_from_output_with_profile(output: &str, parser: &CompiledCostParser) -> RunCost {
    match parser {
        CompiledCostParser::ClaudeCode => parse_cost_from_output(output),
        CompiledCostParser::None => RunCost::default(),
        CompiledCostParser::Custom(re) => {
            let parse_tokens = |s: &str| -> Option<u64> { s.replace(',', "").parse().ok() };

            if let Some(caps) = re.captures(output) {
                let parsed_cost: Option<f64> =
                    caps.name("cost_usd").and_then(|m| m.as_str().parse().ok());
                // Apply the same validation used by the built-in parser: reject
                // negative, NaN, and infinite values.
                let cost_usd = parsed_cost.filter(|&v| is_valid_cost(v));
                let input_tokens = caps
                    .name("input_tokens")
                    .and_then(|m| parse_tokens(m.as_str()));
                let output_tokens = caps
                    .name("output_tokens")
                    .and_then(|m| parse_tokens(m.as_str()));
                let raw_match = caps.get(0).map(|m| m.as_str().to_string());

                if cost_usd.is_none() {
                    return RunCost {
                        cost_usd: None,
                        input_tokens: None,
                        output_tokens: None,
                        confidence: ParseConfidence::None,
                        raw_match,
                    };
                }

                let confidence = if input_tokens.is_some() && output_tokens.is_some() {
                    ParseConfidence::Full
                } else {
                    ParseConfidence::Partial
                };

                RunCost {
                    cost_usd,
                    input_tokens,
                    output_tokens,
                    confidence,
                    raw_match,
                }
            } else {
                // No match at all
                RunCost::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negative_cost_rejected() {
        // JSON path with negative value
        let output =
            r#"{"total_cost_usd": -10.0, "usage": {"input_tokens": 100, "output_tokens": 50}}"#;
        let result = parse_cost_from_output(output);
        assert_eq!(result.cost_usd, None);
        assert_eq!(result.confidence, ParseConfidence::None);
        assert!(result
            .raw_match
            .unwrap_or_default()
            .contains("invalid cost value"));
    }

    #[test]
    fn test_nan_cost_rejected() {
        // NaN cannot appear in valid JSON, but test the validator directly
        assert!(!is_valid_cost(f64::NAN));
        assert!(!is_valid_cost(f64::INFINITY));
        assert!(!is_valid_cost(f64::NEG_INFINITY));
        assert!(!is_valid_cost(-10.0));
        assert!(!is_valid_cost(-0.001));
    }

    #[test]
    fn test_zero_cost_valid() {
        let output =
            r#"{"total_cost_usd": 0.0, "usage": {"input_tokens": 10, "output_tokens": 5}}"#;
        let result = parse_cost_from_output(output);
        assert_eq!(result.cost_usd, Some(0.0));
        assert_eq!(result.confidence, ParseConfidence::Full);
    }

    #[test]
    fn test_standard_cost_line_still_works() {
        let output = "Cost: $1.23 (100 input tokens, 50 output tokens)";
        let result = parse_cost_from_output(output);
        assert_eq!(result.cost_usd, Some(1.23));
        assert_eq!(result.confidence, ParseConfidence::Full);
    }

    #[test]
    fn test_zero_cost_text_format() {
        let output = "Cost: $0.00 (5 input tokens, 3 output tokens)";
        let result = parse_cost_from_output(output);
        assert_eq!(result.cost_usd, Some(0.0));
        assert_eq!(result.confidence, ParseConfidence::Full);
    }

    #[test]
    fn test_no_match_returns_none() {
        let output = "No cost information here.";
        let result = parse_cost_from_output(output);
        assert_eq!(result.cost_usd, None);
        assert_eq!(result.confidence, ParseConfidence::None);
    }

    #[test]
    fn test_json_positive_cost_accepted() {
        let output =
            r#"{"total_cost_usd": 2.50, "usage": {"input_tokens": 200, "output_tokens": 100}}"#;
        let result = parse_cost_from_output(output);
        assert_eq!(result.cost_usd, Some(2.50));
        assert_eq!(result.confidence, ParseConfidence::Full);
        assert_eq!(result.input_tokens, Some(200));
        assert_eq!(result.output_tokens, Some(100));
    }

    // ─── Custom cost parser tests ──────────────────────────────────────────────

    #[test]
    fn custom_parser_extracts_cost_usd_only() {
        let re = regex::Regex::new(r"Total: \$(?P<cost_usd>[\d.]+)").unwrap();
        let parser = CompiledCostParser::Custom(re);
        let result = parse_cost_from_output_with_profile("Total: $1.23", &parser);
        assert_eq!(result.cost_usd, Some(1.23));
        assert_eq!(result.confidence, ParseConfidence::Partial);
        assert_eq!(result.input_tokens, None);
        assert_eq!(result.output_tokens, None);
    }

    #[test]
    fn custom_parser_extracts_all_named_groups() {
        let re = regex::Regex::new(
            r"Cost: \$(?P<cost_usd>[\d.]+) \((?P<input_tokens>[\d,]+) input, (?P<output_tokens>[\d,]+) output\)",
        )
        .unwrap();
        let parser = CompiledCostParser::Custom(re);
        let result =
            parse_cost_from_output_with_profile("Cost: $2.50 (1,000 input, 500 output)", &parser);
        assert_eq!(result.cost_usd, Some(2.50));
        assert_eq!(result.input_tokens, Some(1000));
        assert_eq!(result.output_tokens, Some(500));
        assert_eq!(result.confidence, ParseConfidence::Full);
    }

    #[test]
    fn custom_parser_no_match_returns_none() {
        let re = regex::Regex::new(r"Total: \$(?P<cost_usd>[\d.]+)").unwrap();
        let parser = CompiledCostParser::Custom(re);
        let result = parse_cost_from_output_with_profile("No cost info here.", &parser);
        assert_eq!(result.cost_usd, None);
        assert_eq!(result.confidence, ParseConfidence::None);
    }

    #[test]
    fn none_parser_returns_zero_cost_no_parsing() {
        let parser = CompiledCostParser::None;
        // Even if built-in patterns would match, None skips parsing
        let result = parse_cost_from_output_with_profile(
            "Cost: $1.23 (100 input tokens, 50 output tokens)",
            &parser,
        );
        assert_eq!(result.cost_usd, None);
        assert_eq!(result.input_tokens, None);
        assert_eq!(result.output_tokens, None);
        assert_eq!(result.confidence, ParseConfidence::None);
    }

    #[test]
    fn claude_code_parser_uses_builtin_cascade() {
        let parser = CompiledCostParser::ClaudeCode;
        let result = parse_cost_from_output_with_profile(
            "Cost: $0.75 (200 input tokens, 100 output tokens)",
            &parser,
        );
        assert_eq!(result.cost_usd, Some(0.75));
        assert_eq!(result.confidence, ParseConfidence::Full);
    }

    #[test]
    fn custom_parser_negative_cost_rejected() {
        let re = regex::Regex::new(r"Total: \$(?P<cost_usd>[-\d.]+)").unwrap();
        let parser = CompiledCostParser::Custom(re);
        let result = parse_cost_from_output_with_profile("Total: $-5.00", &parser);
        assert_eq!(result.cost_usd, None);
        assert_eq!(result.confidence, ParseConfidence::None);
    }

    #[test]
    fn custom_parser_nan_cost_rejected() {
        let re = regex::Regex::new(r"Total: (?P<cost_usd>\S+)").unwrap();
        let parser = CompiledCostParser::Custom(re);
        let result = parse_cost_from_output_with_profile("Total: NaN", &parser);
        assert_eq!(result.cost_usd, None);
        assert_eq!(result.confidence, ParseConfidence::None);
    }

    #[test]
    fn custom_parser_valid_cost_still_works() {
        let re = regex::Regex::new(r"Total: \$(?P<cost_usd>[-\d.]+)").unwrap();
        let parser = CompiledCostParser::Custom(re);
        let result = parse_cost_from_output_with_profile("Total: $1.23", &parser);
        assert_eq!(result.cost_usd, Some(1.23));
        assert_eq!(result.confidence, ParseConfidence::Partial);
    }

    #[test]
    fn custom_parser_no_match_does_not_fall_through_to_builtin() {
        // The custom pattern does NOT match, but the built-in pattern would.
        // Per spec: no fallthrough — result must be None.
        let re = regex::Regex::new(r"CUSTOM: \$(?P<cost_usd>[\d.]+)").unwrap();
        let parser = CompiledCostParser::Custom(re);
        let result = parse_cost_from_output_with_profile(
            "Cost: $1.23 (100 input tokens, 50 output tokens)",
            &parser,
        );
        assert_eq!(result.cost_usd, None);
        assert_eq!(result.confidence, ParseConfidence::None);
    }
}
