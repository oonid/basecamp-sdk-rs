mod client;
mod retry;

pub use client::HttpClient;
pub use retry::{
    calculate_backoff, should_retry, should_retry_with_config, RetryContext, RetryDecision,
};

use crate::error::BasecampError;

pub type HttpResult<T> = Result<T, BasecampError>;
