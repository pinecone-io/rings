use std::time::Duration;

#[derive(Debug, Clone)]
pub struct QuotaBackoff {
    pub enabled: bool,
    pub delay_secs: u64,
    pub max_retries: u32,
    pub current_retries: u32,
}

impl QuotaBackoff {
    pub fn new(enabled: bool, delay_secs: u64, max_retries: u32) -> Self {
        Self {
            enabled,
            delay_secs,
            max_retries,
            current_retries: 0,
        }
    }

    pub fn should_retry(&self) -> bool {
        if !self.enabled {
            return false;
        }
        self.current_retries < self.max_retries
    }

    pub fn record_retry(&mut self) {
        self.current_retries += 1;
    }

    pub fn delay_duration(&self) -> Duration {
        Duration::from_secs(self.delay_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_retry_disabled() {
        let backoff = QuotaBackoff::new(false, 1, 3);
        assert!(!backoff.should_retry());
    }

    #[test]
    fn test_should_retry_state_machine() {
        let mut backoff = QuotaBackoff::new(true, 1, 2);

        // First attempt should allow retry
        assert!(backoff.should_retry());

        // Record first retry
        backoff.record_retry();
        assert_eq!(backoff.current_retries, 1);
        assert!(backoff.should_retry());

        // Record second retry (exhausts max)
        backoff.record_retry();
        assert_eq!(backoff.current_retries, 2);
        assert!(!backoff.should_retry());
    }

    #[test]
    fn test_delay_duration() {
        let backoff = QuotaBackoff::new(true, 5, 3);
        assert_eq!(backoff.delay_duration(), Duration::from_secs(5));
    }
}
