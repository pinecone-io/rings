use anyhow::{bail, Result};
use serde::Deserialize;

/// Parses a duration string into seconds.
///
/// Accepts:
/// - Integer-only strings (e.g., `"300"`) — treated as seconds
/// - Single-char suffix strings: `"30s"` (seconds), `"5m"` (minutes), `"1h"` (hours)
///
/// Returns `Err` for zero values, unrecognized formats, and arithmetic overflow.
pub fn parse_duration_secs(s: &str) -> Result<u64> {
    let s = s.trim();
    if s.is_empty() {
        bail!("duration string must not be empty");
    }

    // Try plain integer (no suffix)
    if let Ok(n) = s.parse::<u64>() {
        if n == 0 {
            bail!("duration must be greater than zero");
        }
        return Ok(n);
    }

    // Must have exactly one suffix character at the end
    let (digits, suffix) = s.split_at(s.len() - 1);
    let n: u64 = digits
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid duration: {:?}", s))?;

    if n == 0 {
        bail!("duration must be greater than zero");
    }

    let multiplier: u64 = match suffix {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        _ => bail!(
            "invalid duration suffix {:?} in {:?}; expected s, m, or h",
            suffix,
            s
        ),
    };

    n.checked_mul(multiplier)
        .ok_or_else(|| anyhow::anyhow!("duration overflow: {:?}", s))
}

/// Supports both integer and string TOML values for duration fields.
///
/// ```toml
/// timeout_per_run_secs = 300      # DurationField::Secs(300)
/// timeout_per_run_secs = "5m"     # DurationField::Str("5m")
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum DurationField {
    Secs(u64),
    Str(String),
}

impl DurationField {
    /// Resolves the field to a concrete number of seconds.
    pub fn to_secs(&self) -> Result<u64> {
        match self {
            DurationField::Secs(n) => {
                if *n == 0 {
                    bail!("duration must be greater than zero");
                }
                Ok(*n)
            }
            DurationField::Str(s) => parse_duration_secs(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seconds_suffix() {
        assert_eq!(parse_duration_secs("30s").unwrap(), 30);
    }

    #[test]
    fn test_minutes_suffix() {
        assert_eq!(parse_duration_secs("5m").unwrap(), 300);
    }

    #[test]
    fn test_hours_suffix() {
        assert_eq!(parse_duration_secs("1h").unwrap(), 3600);
    }

    #[test]
    fn test_zero_no_suffix_is_err() {
        assert!(parse_duration_secs("0").is_err());
    }

    #[test]
    fn test_zero_with_suffix_is_err() {
        assert!(parse_duration_secs("0s").is_err());
    }

    #[test]
    fn test_multi_char_suffix_is_err() {
        assert!(parse_duration_secs("5min").is_err());
    }

    #[test]
    fn test_compound_duration_is_err() {
        assert!(parse_duration_secs("1h30m").is_err());
    }

    #[test]
    fn test_empty_string_is_err() {
        assert!(parse_duration_secs("").is_err());
    }

    #[test]
    fn test_whitespace_only_is_err() {
        assert!(parse_duration_secs("   ").is_err());
    }

    #[test]
    fn test_trim_leading_trailing_spaces() {
        assert_eq!(parse_duration_secs("  30s  ").unwrap(), 30);
    }

    #[test]
    fn test_uppercase_suffix_is_err() {
        assert!(parse_duration_secs("30S").is_err());
    }

    #[test]
    fn test_overflow() {
        // u64::MAX / 3600 ≈ 5_124_095_576_030_431; anything above that overflows
        assert!(parse_duration_secs("9999999999999999h").is_err());
    }

    #[test]
    fn test_toml_integer_deserialization() {
        #[derive(Deserialize)]
        struct Config {
            timeout_per_run_secs: DurationField,
        }
        let cfg: Config = toml::from_str("timeout_per_run_secs = 300").unwrap();
        match cfg.timeout_per_run_secs {
            DurationField::Secs(300) => {}
            other => panic!("expected Secs(300), got {:?}", other),
        }
    }

    #[test]
    fn test_toml_string_deserialization() {
        #[derive(Deserialize)]
        struct Config {
            timeout_per_run_secs: DurationField,
        }
        let cfg: Config = toml::from_str(r#"timeout_per_run_secs = "5m""#).unwrap();
        match &cfg.timeout_per_run_secs {
            DurationField::Str(s) => assert_eq!(s, "5m"),
            other => panic!("expected Str(\"5m\"), got {:?}", other),
        }
    }

    #[test]
    fn test_duration_field_to_secs_integer() {
        let f = DurationField::Secs(120);
        assert_eq!(f.to_secs().unwrap(), 120);
    }

    #[test]
    fn test_duration_field_to_secs_string() {
        let f = DurationField::Str("2m".to_string());
        assert_eq!(f.to_secs().unwrap(), 120);
    }
}
