use crate::state::FailureReason;
use crate::workflow::CompiledErrorProfile;

/// Classify an executor error based on output patterns.
/// Returns the first matching error class: Quota, Auth, or Unknown.
pub fn classify(output: &str, profile: &CompiledErrorProfile) -> FailureReason {
    // Check quota patterns first (first-match-wins)
    for regex in &profile.quota_regexes {
        if regex.is_match(output) {
            return FailureReason::Quota;
        }
    }

    // Check auth patterns
    for regex in &profile.auth_regexes {
        if regex.is_match(output) {
            return FailureReason::Auth;
        }
    }

    // No match
    FailureReason::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    fn make_profile(quota: Vec<&str>, auth: Vec<&str>) -> CompiledErrorProfile {
        let quota_regexes = quota
            .iter()
            .map(|p| Regex::new(&format!("(?i){}", regex::escape(p))).unwrap())
            .collect();
        let auth_regexes = auth
            .iter()
            .map(|p| Regex::new(&format!("(?i){}", regex::escape(p))).unwrap())
            .collect();
        CompiledErrorProfile {
            quota_regexes,
            auth_regexes,
        }
    }

    #[test]
    fn test_classify_quota_patterns() {
        let profile = make_profile(
            vec![
                "usage limit reached",
                "rate limit",
                "quota exceeded",
                "too many requests",
                "429",
                "claude.ai/settings",
            ],
            vec![],
        );

        // Test each quota pattern individually
        assert_eq!(
            classify("Error: usage limit reached", &profile),
            FailureReason::Quota
        );
        assert_eq!(
            classify("Rate limit exceeded", &profile),
            FailureReason::Quota
        );
        assert_eq!(classify("quota exceeded", &profile), FailureReason::Quota);
        assert_eq!(
            classify("too many requests", &profile),
            FailureReason::Quota
        );
        assert_eq!(
            classify("HTTP 429 Too Many Requests", &profile),
            FailureReason::Quota
        );
        assert_eq!(
            classify("Visit claude.ai/settings for more info", &profile),
            FailureReason::Quota
        );
    }

    #[test]
    fn test_classify_auth_patterns() {
        let profile = make_profile(
            vec![],
            vec![
                "authentication",
                "invalid api key",
                "unauthorized",
                "401",
                "please log in",
                "not logged in",
            ],
        );

        // Test each auth pattern individually
        assert_eq!(
            classify("authentication failed", &profile),
            FailureReason::Auth
        );
        assert_eq!(classify("Invalid API Key", &profile), FailureReason::Auth);
        assert_eq!(
            classify("Unauthorized request", &profile),
            FailureReason::Auth
        );
        assert_eq!(
            classify("HTTP 401 Unauthorized", &profile),
            FailureReason::Auth
        );
        assert_eq!(classify("Please log in", &profile), FailureReason::Auth);
        assert_eq!(
            classify("You are not logged in", &profile),
            FailureReason::Auth
        );
    }

    #[test]
    fn test_classify_case_insensitive() {
        let profile = make_profile(vec!["QUOTA EXCEEDED"], vec!["UNAUTHORIZED"]);

        assert_eq!(classify("quota exceeded", &profile), FailureReason::Quota);
        assert_eq!(classify("QUOTA EXCEEDED", &profile), FailureReason::Quota);
        assert_eq!(classify("Quota Exceeded", &profile), FailureReason::Quota);
        assert_eq!(classify("unauthorized", &profile), FailureReason::Auth);
        assert_eq!(classify("UNAUTHORIZED", &profile), FailureReason::Auth);
    }

    #[test]
    fn test_classify_first_match_wins() {
        let profile = make_profile(vec!["quota"], vec!["auth"]);

        // Both patterns present: quota wins (checked first)
        assert_eq!(
            classify("quota exceeded and auth failed", &profile),
            FailureReason::Quota
        );
    }

    #[test]
    fn test_classify_unknown() {
        let profile = make_profile(vec!["quota"], vec!["auth"]);

        assert_eq!(classify("no match here", &profile), FailureReason::Unknown);
        assert_eq!(classify("", &profile), FailureReason::Unknown);
    }

    #[test]
    fn test_classify_none_profile() {
        let profile = make_profile(vec![], vec![]);

        // None profile: no patterns, always Unknown
        assert_eq!(classify("quota exceeded", &profile), FailureReason::Unknown);
        assert_eq!(classify("unauthorized", &profile), FailureReason::Unknown);
    }

    #[test]
    fn test_classify_pattern_in_stderr_half() {
        let profile = make_profile(vec!["quota exceeded"], vec![]);

        // Pattern in "stderr half" (after newline separator)
        let combined_output = "stdout line 1\nstdout line 2\nquota exceeded";
        assert_eq!(classify(combined_output, &profile), FailureReason::Quota);
    }

    #[test]
    fn test_classify_pattern_in_non_error_context() {
        let profile = make_profile(vec!["429"], vec![]);

        // Pattern appears in non-error context, but we only classify on non-zero exit
        // (which is enforced by the engine, not this function).
        // This test verifies that the function itself matches the pattern.
        assert_eq!(
            classify("Processing 429 records", &profile),
            FailureReason::Quota
        );
    }
}
