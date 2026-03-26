use crate::config::Config;
use crate::error::BasecampError;
use rand::Rng;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    Retry { delay: Duration },
    DontRetry,
}

#[derive(Debug, Clone, Copy)]
pub struct RetryContext {
    pub attempt: u32,
    pub max_retries: u32,
    pub is_idempotent: bool,
    pub is_post: bool,
}

impl RetryContext {
    pub fn new(attempt: u32, max_retries: u32, is_idempotent: bool, is_post: bool) -> Self {
        Self {
            attempt,
            max_retries,
            is_idempotent,
            is_post,
        }
    }
}

pub fn calculate_backoff(attempt: u32, base_delay: Duration, max_jitter: Duration) -> Duration {
    let base_ms = base_delay.as_millis() as u64;
    let jitter_ms = if max_jitter.as_millis() > 0 {
        rand::thread_rng().gen_range(0..=max_jitter.as_millis() as u64)
    } else {
        0
    };

    let multiplier = 1u64 << (attempt.saturating_sub(1));
    let delay_ms = base_ms.saturating_mul(multiplier).saturating_add(jitter_ms);

    Duration::from_millis(delay_ms)
}

pub fn should_retry(
    error: &BasecampError,
    ctx: &RetryContext,
    retry_after: Option<Duration>,
) -> RetryDecision {
    if ctx.attempt >= ctx.max_retries {
        return RetryDecision::DontRetry;
    }

    if !error.retryable() {
        return RetryDecision::DontRetry;
    }

    if ctx.is_post && !ctx.is_idempotent {
        return RetryDecision::DontRetry;
    }

    let delay = match retry_after {
        Some(ra) => ra,
        None => calculate_backoff(
            ctx.attempt,
            Duration::from_secs(1),
            Duration::from_millis(100),
        ),
    };

    RetryDecision::Retry { delay }
}

pub fn should_retry_with_config(
    error: &BasecampError,
    attempt: u32,
    config: &Config,
    is_idempotent: bool,
    is_post: bool,
    retry_after: Option<Duration>,
) -> RetryDecision {
    if attempt >= config.max_retries {
        return RetryDecision::DontRetry;
    }

    if !error.retryable() {
        return RetryDecision::DontRetry;
    }

    if is_post && !is_idempotent {
        return RetryDecision::DontRetry;
    }

    let delay = match retry_after {
        Some(ra) => ra,
        None => calculate_backoff(attempt, config.base_delay, config.max_jitter),
    };

    RetryDecision::Retry { delay }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_retryable_error() -> BasecampError {
        BasecampError::Api {
            status: 503,
            message: "Service unavailable".to_string(),
            request_id: None,
            retryable: true,
        }
    }

    fn make_non_retryable_error() -> BasecampError {
        BasecampError::NotFound {
            resource_type: None,
            resource_id: None,
            request_id: None,
        }
    }

    fn make_rate_limit_error(retry_after: Option<u64>) -> BasecampError {
        BasecampError::RateLimit {
            retry_after,
            request_id: None,
        }
    }

    fn make_network_error() -> BasecampError {
        BasecampError::Network {
            message: "Connection timeout".to_string(),
        }
    }

    mod backoff_calculation {
        use super::*;

        #[test]
        fn test_backoff_first_attempt() {
            let base = Duration::from_secs(1);
            let jitter = Duration::from_millis(0);

            let delay = calculate_backoff(1, base, jitter);
            assert_eq!(delay, Duration::from_secs(1));
        }

        #[test]
        fn test_backoff_second_attempt() {
            let base = Duration::from_secs(1);
            let jitter = Duration::from_millis(0);

            let delay = calculate_backoff(2, base, jitter);
            assert_eq!(delay, Duration::from_secs(2));
        }

        #[test]
        fn test_backoff_third_attempt() {
            let base = Duration::from_secs(1);
            let jitter = Duration::from_millis(0);

            let delay = calculate_backoff(3, base, jitter);
            assert_eq!(delay, Duration::from_secs(4));
        }

        #[test]
        fn test_backoff_fourth_attempt() {
            let base = Duration::from_secs(1);
            let jitter = Duration::from_millis(0);

            let delay = calculate_backoff(4, base, jitter);
            assert_eq!(delay, Duration::from_secs(8));
        }

        #[test]
        fn test_backoff_with_jitter() {
            let base = Duration::from_secs(1);
            let jitter = Duration::from_millis(100);

            for attempt in 1..=5 {
                let delay = calculate_backoff(attempt, base, jitter);
                let base_part = Duration::from_secs(1 << (attempt - 1));
                assert!(delay >= base_part);
                assert!(delay <= base_part + jitter);
            }
        }

        #[test]
        fn test_backoff_zero_jitter() {
            let base = Duration::from_secs(1);
            let jitter = Duration::from_millis(0);

            let delay = calculate_backoff(1, base, jitter);
            assert_eq!(delay, Duration::from_secs(1));
        }

        #[test]
        fn test_backoff_custom_base_delay() {
            let base = Duration::from_millis(500);
            let jitter = Duration::from_millis(0);

            let delay = calculate_backoff(1, base, jitter);
            assert_eq!(delay, Duration::from_millis(500));

            let delay = calculate_backoff(2, base, jitter);
            assert_eq!(delay, Duration::from_millis(1000));

            let delay = calculate_backoff(3, base, jitter);
            assert_eq!(delay, Duration::from_millis(2000));
        }

        #[test]
        fn test_backoff_formula_base_times_2_power_attempt_minus_1() {
            let base = Duration::from_secs(1);
            let jitter = Duration::from_millis(0);

            for attempt in 1..=10 {
                let delay = calculate_backoff(attempt, base, jitter);
                let expected = Duration::from_secs(1u64 << (attempt - 1));
                assert_eq!(delay, expected, "Attempt {} failed", attempt);
            }
        }
    }

    mod should_retry_logic {
        use super::*;

        #[test]
        fn test_get_retryable_error() {
            let error = make_retryable_error();
            let ctx = RetryContext::new(1, 3, true, false);

            let decision = should_retry(&error, &ctx, None);
            assert!(matches!(decision, RetryDecision::Retry { .. }));
        }

        #[test]
        fn test_get_non_retryable_error() {
            let error = make_non_retryable_error();
            let ctx = RetryContext::new(1, 3, true, false);

            let decision = should_retry(&error, &ctx, None);
            assert_eq!(decision, RetryDecision::DontRetry);
        }

        #[test]
        fn test_post_not_idempotent() {
            let error = make_retryable_error();
            let ctx = RetryContext::new(1, 3, false, true);

            let decision = should_retry(&error, &ctx, None);
            assert_eq!(decision, RetryDecision::DontRetry);
        }

        #[test]
        fn test_post_idempotent() {
            let error = make_retryable_error();
            let ctx = RetryContext::new(1, 3, true, true);

            let decision = should_retry(&error, &ctx, None);
            assert!(matches!(decision, RetryDecision::Retry { .. }));
        }

        #[test]
        fn test_max_retries_exceeded() {
            let error = make_retryable_error();
            let ctx = RetryContext::new(3, 3, true, false);

            let decision = should_retry(&error, &ctx, None);
            assert_eq!(decision, RetryDecision::DontRetry);
        }

        #[test]
        fn test_retry_after_overrides_backoff() {
            let error = make_rate_limit_error(None);
            let ctx = RetryContext::new(1, 3, true, false);
            let retry_after = Some(Duration::from_secs(60));

            let decision = should_retry(&error, &ctx, retry_after);
            match decision {
                RetryDecision::Retry { delay } => assert_eq!(delay, Duration::from_secs(60)),
                RetryDecision::DontRetry => panic!("Expected retry"),
            }
        }

        #[test]
        fn test_rate_limit_error_retryable() {
            let error = make_rate_limit_error(Some(30));
            let ctx = RetryContext::new(1, 3, true, false);

            let decision = should_retry(&error, &ctx, None);
            assert!(matches!(decision, RetryDecision::Retry { .. }));
        }

        #[test]
        fn test_network_error_retryable() {
            let error = make_network_error();
            let ctx = RetryContext::new(1, 3, true, false);

            let decision = should_retry(&error, &ctx, None);
            assert!(matches!(decision, RetryDecision::Retry { .. }));
        }

        #[test]
        fn test_403_not_retryable() {
            let error = BasecampError::Forbidden {
                reason: None,
                request_id: None,
            };
            let ctx = RetryContext::new(1, 3, true, false);

            let decision = should_retry(&error, &ctx, None);
            assert_eq!(decision, RetryDecision::DontRetry);
        }

        #[test]
        fn test_404_not_retryable() {
            let error = BasecampError::NotFound {
                resource_type: None,
                resource_id: None,
                request_id: None,
            };
            let ctx = RetryContext::new(1, 3, true, false);

            let decision = should_retry(&error, &ctx, None);
            assert_eq!(decision, RetryDecision::DontRetry);
        }

        #[test]
        fn test_401_not_retryable_directly() {
            let error = BasecampError::AuthRequired {
                hint: None,
                request_id: None,
            };
            let ctx = RetryContext::new(1, 3, true, false);

            let decision = should_retry(&error, &ctx, None);
            assert_eq!(decision, RetryDecision::DontRetry);
        }

        #[test]
        fn test_validation_error_not_retryable() {
            let error = BasecampError::Validation {
                message: "Invalid".to_string(),
                fields: vec![],
                request_id: None,
            };
            let ctx = RetryContext::new(1, 3, true, false);

            let decision = should_retry(&error, &ctx, None);
            assert_eq!(decision, RetryDecision::DontRetry);
        }
    }

    mod should_retry_with_config {
        use super::*;

        #[test]
        fn test_uses_config_values() {
            let config = Config::builder()
                .max_retries(5)
                .base_delay(Duration::from_millis(200))
                .max_jitter(Duration::from_millis(0))
                .build()
                .unwrap();

            let error = make_retryable_error();
            let decision = should_retry_with_config(&error, 1, &config, true, false, None);

            match decision {
                RetryDecision::Retry { delay } => assert_eq!(delay, Duration::from_millis(200)),
                RetryDecision::DontRetry => panic!("Expected retry"),
            }
        }

        #[test]
        fn test_respects_max_retries_from_config() {
            let config = Config::builder().max_retries(2).build().unwrap();

            let error = make_retryable_error();

            let decision = should_retry_with_config(&error, 1, &config, true, false, None);
            assert!(matches!(decision, RetryDecision::Retry { .. }));

            let decision = should_retry_with_config(&error, 2, &config, true, false, None);
            assert_eq!(decision, RetryDecision::DontRetry);
        }
    }

    mod retry_context {
        use super::*;

        #[test]
        fn test_context_creation() {
            let ctx = RetryContext::new(2, 5, true, false);
            assert_eq!(ctx.attempt, 2);
            assert_eq!(ctx.max_retries, 5);
            assert!(ctx.is_idempotent);
            assert!(!ctx.is_post);
        }
    }

    mod retry_decision {
        use super::*;

        #[test]
        fn test_retry_decision_equality() {
            let decision1 = RetryDecision::Retry {
                delay: Duration::from_secs(1),
            };
            let decision2 = RetryDecision::Retry {
                delay: Duration::from_secs(1),
            };
            let decision3 = RetryDecision::DontRetry;

            assert_eq!(decision1, decision2);
            assert_ne!(decision1, decision3);
        }
    }
}
