pub mod auth;
pub mod config;
pub mod error;
pub mod hooks;
pub mod http;
pub mod pagination;
pub mod security;

pub use auth::{
    AuthStrategy, BearerAuth, OAuthToken, OAuthTokenProvider, OnRefreshCallback,
    StaticTokenProvider, TokenProvider,
};
pub use config::{Config, ConfigBuilder, ConfigError};
pub use error::{BasecampError, ErrorCode, FieldError};
pub use hooks::{
    chain_hooks, console_hooks, no_hooks, safe_hook, timing_hooks, BasecampHooks, ChainedHooks,
    ConsoleHooks, ConsoleLogLevel, NoOpHooks, OperationInfo, OperationResult, RequestInfo,
    RequestResult, TimingHooks,
};
pub use http::HttpClient;
pub use pagination::{parse_next_link, parse_total_count, resolve_url, ListMeta, ListResult};
pub use security::{
    check_body_size, contains_crlf, is_localhost, redact_headers, require_https, same_origin,
    truncate, validate_header_value, validate_url_for_redirect, MAX_ERROR_BODY_BYTES,
    MAX_ERROR_MESSAGE_BYTES, MAX_RESPONSE_BODY_BYTES,
};
