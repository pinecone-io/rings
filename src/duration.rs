use anyhow::{bail, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::Deserialize;
use std::str::FromStr;

/// Parses a duration string into seconds.
///
/// Accepts:
/// - Integer-only strings (e.g., `"300"`) — treated as seconds
/// - Single-char suffix strings: `"30s"` (seconds), `"5m"` (minutes), `"1h"` (hours), `"1d"` (days)
/// - Combined strings: `"1h30m"` (1 hour and 30 minutes = 5400 seconds)
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

    // Parse combined or single-suffix duration string.
    // Supported tokens: <digits><suffix> where suffix is one of h, m, s, d.
    // Tokens must appear in a known order and each may appear at most once.
    let total =
        parse_combined_duration(s).map_err(|_| anyhow::anyhow!("invalid duration: {:?}", s))?;

    if total == 0 {
        bail!("duration must be greater than zero");
    }
    Ok(total)
}

/// Parses a combined duration string like "1h30m" or "30s" into total seconds.
/// Returns Err if the string is not a valid sequence of <digits><suffix> tokens.
fn parse_combined_duration(s: &str) -> Result<u64> {
    // Each suffix may appear at most once, and must appear in descending order: d, h, m, s.
    const SUFFIXES: &[(&str, u64)] = &[("d", 86400), ("h", 3600), ("m", 60), ("s", 1)];

    let mut remaining = s;
    let mut total: u64 = 0;
    let mut last_suffix_index: Option<usize> = None;
    let mut matched_any = false;

    while !remaining.is_empty() {
        // Find the index of the next suffix character.
        let suffix_pos = remaining
            .find(['d', 'h', 'm', 's'])
            .ok_or_else(|| anyhow::anyhow!("no suffix found in {:?}", s))?;

        let digits = &remaining[..suffix_pos];
        let suffix_char = &remaining[suffix_pos..suffix_pos + 1];
        remaining = &remaining[suffix_pos + 1..];

        if digits.is_empty() {
            bail!("missing digits before suffix {:?}", suffix_char);
        }
        let n: u64 = digits
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid digits {:?}", digits))?;

        // Find the multiplier and enforce ordering.
        let suffix_index = SUFFIXES
            .iter()
            .position(|(suf, _)| *suf == suffix_char)
            .ok_or_else(|| anyhow::anyhow!("unknown suffix {:?}", suffix_char))?;

        if let Some(prev) = last_suffix_index {
            if suffix_index <= prev {
                bail!("suffixes out of order in {:?}", s);
            }
        }
        last_suffix_index = Some(suffix_index);

        let multiplier = SUFFIXES[suffix_index].1;
        total = total
            .checked_add(
                n.checked_mul(multiplier)
                    .ok_or_else(|| anyhow::anyhow!("overflow"))?,
            )
            .ok_or_else(|| anyhow::anyhow!("overflow"))?;
        matched_any = true;
    }

    if !matched_any {
        bail!("empty duration");
    }

    Ok(total)
}

/// Represents either an absolute date or a relative duration.
#[derive(Debug, Clone)]
pub enum SinceSpec {
    /// Absolute date (e.g., "2024-03-15"); midnight UTC is used as the time.
    AbsoluteDate(NaiveDate),
    /// Relative duration from now (e.g., "7d", "2h").
    Relative(Duration),
}

impl SinceSpec {
    /// Convert the spec to a cutoff datetime in UTC.
    pub fn to_cutoff_datetime(&self) -> DateTime<Utc> {
        match self {
            SinceSpec::AbsoluteDate(date) => {
                // Midnight UTC on the given date
                date.and_hms_opt(0, 0, 0)
                    .expect("midnight is always valid")
                    .and_utc()
            }
            SinceSpec::Relative(duration) => Utc::now() - *duration,
        }
    }
}

impl FromStr for SinceSpec {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        // Try to parse as ISO 8601 date first
        if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            return Ok(SinceSpec::AbsoluteDate(date));
        }

        // Try to parse as relative duration
        match parse_duration_secs(s) {
            Ok(secs) => Ok(SinceSpec::Relative(Duration::seconds(secs as i64))),
            Err(_) => bail!(
                "Invalid --since value: {:?}. Expected ISO 8601 date (YYYY-MM-DD) or relative duration (e.g., 7d, 2h)",
                s
            ),
        }
    }
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
    fn test_days_suffix() {
        assert_eq!(parse_duration_secs("1d").unwrap(), 86400);
        assert_eq!(parse_duration_secs("7d").unwrap(), 604800);
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
    fn test_zero_days_is_err() {
        assert!(parse_duration_secs("0d").is_err());
    }

    #[test]
    fn test_multi_char_suffix_is_err() {
        assert!(parse_duration_secs("5min").is_err());
    }

    #[test]
    fn test_space_minutes_is_err() {
        assert!(parse_duration_secs("5 minutes").is_err());
    }

    #[test]
    fn test_compound_duration_1h30m() {
        assert_eq!(parse_duration_secs("1h30m").unwrap(), 5400);
    }

    #[test]
    fn test_compound_duration_1d2h30m() {
        assert_eq!(parse_duration_secs("1d2h30m").unwrap(), 86400 + 7200 + 1800);
    }

    #[test]
    fn test_compound_duration_out_of_order_is_err() {
        assert!(parse_duration_secs("30m1h").is_err());
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

    #[test]
    fn test_since_spec_relative_duration() {
        use std::str::FromStr;
        let spec = SinceSpec::from_str("7d").unwrap();
        match spec {
            SinceSpec::Relative(_) => {} // OK
            _ => panic!("expected Relative variant"),
        }
    }

    #[test]
    fn test_since_spec_absolute_date() {
        use std::str::FromStr;
        let spec = SinceSpec::from_str("2024-03-15").unwrap();
        match spec {
            SinceSpec::AbsoluteDate(date) => {
                assert_eq!(date.to_string(), "2024-03-15");
            }
            _ => panic!("expected AbsoluteDate variant"),
        }
    }

    #[test]
    fn test_since_spec_invalid_date() {
        use std::str::FromStr;
        assert!(SinceSpec::from_str("2024-13-01").is_err());
    }

    #[test]
    fn test_since_spec_to_cutoff_datetime_absolute() {
        use std::str::FromStr;
        let spec = SinceSpec::from_str("2024-03-15").unwrap();
        let cutoff = spec.to_cutoff_datetime();
        // Check that it's midnight UTC on 2024-03-15
        assert_eq!(
            cutoff.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2024-03-15 00:00:00"
        );
    }

    #[test]
    fn test_since_spec_zero_duration_is_err() {
        use std::str::FromStr;
        assert!(SinceSpec::from_str("0d").is_err());
    }
}
