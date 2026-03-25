mod client;

pub use client::HttpClient;

use crate::error::BasecampError;

pub type HttpResult<T> = Result<T, BasecampError>;
