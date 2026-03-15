use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

pub fn parse_cost_from_output(output: &str) -> RunCost {
    let parse_tokens = |s: &str| -> Option<u64> { s.replace(',', "").parse().ok() };

    // Try patterns in order, use last match of highest-confidence pattern found
    if let Some(caps) = RE_FULL.captures_iter(output).last() {
        let raw = caps[0].to_string();
        return RunCost {
            cost_usd: caps[1].parse().ok(),
            input_tokens: caps.get(2).and_then(|m| parse_tokens(m.as_str())),
            output_tokens: caps.get(3).and_then(|m| parse_tokens(m.as_str())),
            confidence: ParseConfidence::Full,
            raw_match: Some(raw),
        };
    }

    if let Some(caps) = RE_SIMPLE.captures_iter(output).last() {
        let raw = caps[0].to_string();
        return RunCost {
            cost_usd: caps[1].parse().ok(),
            input_tokens: None,
            output_tokens: None,
            confidence: ParseConfidence::Partial,
            raw_match: Some(raw),
        };
    }

    if let Some(caps) = RE_TOTAL.captures_iter(output).last() {
        let raw = caps[0].to_string();
        return RunCost {
            cost_usd: caps[1].parse().ok(),
            input_tokens: None,
            output_tokens: None,
            confidence: ParseConfidence::Partial,
            raw_match: Some(raw),
        };
    }

    if let Some(caps) = RE_GENERIC.captures_iter(output).last() {
        let raw = caps[0].to_string();
        return RunCost {
            cost_usd: caps[1].parse().ok(),
            input_tokens: None,
            output_tokens: None,
            confidence: ParseConfidence::Low,
            raw_match: Some(raw),
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
