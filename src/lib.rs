//! # Basecamp Rust SDK (Unofficial)
//!
//! A Rust SDK for the [Basecamp API](https://github.com/basecamp/bc3-api).
//!
//! This is an **unofficial** community-maintained SDK. For official SDKs, see
//! [basecamp/basecamp-sdk](https://github.com/basecamp/basecamp-sdk).
//!
//! ## Features
//!
//! - Full coverage of Basecamp API services
//! - OAuth 2.0 and static token authentication
//! - Automatic retry with exponential backoff
//! - Pagination handling with `get_paginated`
//! - Structured errors with CLI-friendly exit codes
//! - HTTPS enforcement for non-localhost URLs
//! - Observability hooks for logging, metrics, and tracing
//!
//! ## Installation
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! basecamp-sdk-rs = "0.1"
//! tokio = { version = "1", features = ["full"] }
//! ```
//!
//! ## Quick Start
//!
//! ### Using a Static Token
//!
//! ```rust,no_run
//! use basecamp_sdk_rs::{Client, Config, BearerAuth};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure the client
//!     let config = Config::builder()
//!         .base_url("https://3.basecampapi.com/999")
//!         .build()?;
//!
//!     // Use a static token
//!     let auth = BearerAuth::from_token("your-access-token");
//!     let client = Client::new(config, auth)?;
//!
//!     // List all projects
//!     let projects = client.get_paginated::<serde_json::Value>("/projects.json", None).await?;
//!     for project in projects.items {
//!         println!("{:?}", project);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Configuration
//!
//! ### Builder Pattern
//!
//! ```rust
//! use basecamp_sdk_rs::Config;
//!
//! let config = Config::builder()
//!     .base_url("https://3.basecampapi.com/999")
//!     .timeout(std::time::Duration::from_secs(60))
//!     .max_retries(5)
//!     .max_pages(100)
//!     .max_items(1000)
//!     .build()?;
//! # Ok::<(), basecamp_sdk_rs::ConfigError>(())
//! ```
//!
//! ### Environment Variables
//!
//! | Variable | Description | Default |
//! |----------|-------------|---------|
//! | `BASECAMP_TOKEN` | API token | Required |
//! | `BASECAMP_ACCOUNT_ID` | Account ID | Required |
//! | `BASECAMP_BASE_URL` | API base URL | `https://3.basecampapi.com` |
//!
//! ## Authentication
//!
//! ### Static Token
//!
//! ```rust
//! use basecamp_sdk_rs::{BearerAuth, Config, Client};
//!
//! let auth = BearerAuth::from_token("your-token");
//! let client = Client::new(config, auth)?;
//! # Ok::<(), basecamp_sdk_rs::ClientError>(())
//! ```
//!
//! ### OAuth 2.0 with Token Refresh
//!
//! ```rust
//! use basecamp_sdk_rs::{OAuthToken, Config, Client};
//!
//! // Implement TokenProvider for automatic token refresh
//! let token_provider = MyTokenProvider::new();
//! let client = Client::new(config, token_provider)?;
//! # Ok::<(), basecamp_sdk_rs::ClientError>(())
//! ```
//!
//! ## Error Handling
//!
//! The SDK provides structured errors with codes for programmatic handling:
//!
//! ```rust,no_run
//! use basecamp_sdk_rs::{Client, Config, BearerAuth, BasecampError, ErrorCode};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = Config::builder().base_url("https://3.basecampapi.com/999").build()?;
//! let auth = BearerAuth::from_token("token");
//! let client = Client::new(config, auth)?;
//!
//! match client.get("/projects/99999.json", None).await {
//!     Ok(response) => println!("Success: {:?}", response.status()),
//!     Err(e) => {
//!         match e {
//!             BasecampError::NotFound { message, .. } => {
//!                 eprintln!("Not found: {}", message);
//!             }
//!             BasecampError::AuthRequired { message, .. } => {
//!                 eprintln!("Auth required: {}", message);
//!             }
//!             BasecampError::RateLimit { retry_after, .. } => {
//!                 eprintln!("Rate limited. Retry after: {:?}", retry_after);
//!             }
//!             _ => eprintln!("Error: {}", e),
//!         }
//!         
//!         // Use exit codes for CLI applications
//!         std::process::exit(e.exit_code());
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Error Codes
//!
//! | Code | HTTP Status | Exit Code | Description |
//! |------|-------------|-----------|-------------|
//! | `auth_required` | 401 | 3 | Authentication required |
//! | `forbidden` | 403 | 4 | Access denied |
//! | `not_found` | 404 | 2 | Resource not found |
//! | `rate_limit` | 429 | 5 | Rate limit exceeded (retryable) |
//! | `network` | - | 6 | Network error (retryable) |
//! | `api_error` | 5xx | 7 | Server error |
//! | `validation` | 400, 422 | 9 | Invalid request data |
//! | `usage` | - | 1 | Configuration or argument error |
//!
//! ## Pagination
//!
//! Use `get_paginated` for automatic pagination:
//!
//! ```rust,no_run
//! use basecamp_sdk_rs::{Client, Config, BearerAuth};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = Config::builder()
//!     .base_url("https://3.basecampapi.com/999")
//!     .max_pages(10)      // Limit pages
//!     .max_items(500)     // Limit total items
//!     .build()?;
//! let auth = BearerAuth::from_token("token");
//! let client = Client::new(config, auth)?;
//!
//! // Fetch all pages automatically
//! let result = client.get_paginated::<serde_json::Value>("/projects.json", None).await?;
//!
//! println!("Fetched {} items", result.items.len());
//! println!("Truncated: {}", result.meta.truncated);
//! println!("Total count: {:?}", result.meta.total_count);
//! # Ok(())
//! # }
//! ```
//!
//! ## Retry Behavior
//!
//! The SDK automatically retries requests on transient failures:
//!
//! - **Retryable errors**: 429 (rate limit) and 503 (service unavailable)
//! - **Backoff**: Exponential with jitter
//! - **Rate limits**: Respects `Retry-After` header
//! - **Max retries**: 3 attempts by default
//!
//! Non-idempotent operations (POST) are not retried by default.
//!
//! ## Observability
//!
//! ### Console Logging
//!
//! ```rust
//! use basecamp_sdk_rs::{Client, Config, BearerAuth, console_hooks};
//!
//! let config = Config::builder().base_url("https://3.basecampapi.com/999").build()?;
//! let auth = BearerAuth::from_token("token");
//! let hooks = console_hooks();
//! let client = Client::with_hooks(config, auth, hooks)?;
//! # Ok::<(), basecamp_sdk_rs::ClientError>(())
//! ```
//!
//! Output:
//! ```text
//! [Basecamp] -> GET https://3.basecampapi.com/999/projects.json
//! [Basecamp] <- GET https://3.basecampapi.com/999/projects.json 200 (145ms)
//! ```
//!
//! ### Custom Hooks
//!
//! Implement `BasecampHooks` for custom observability:
//!
//! ```rust
//! use basecamp_sdk_rs::{BasecampHooks, OperationInfo, OperationResult, RequestInfo, RequestResult};
//!
//! struct MetricsHooks;
//!
//! impl BasecampHooks for MetricsHooks {
//!     fn on_operation_start(&self, info: &OperationInfo) {
//!         println!("Starting: {}.{}", info.service, info.operation);
//!     }
//!
//!     fn on_operation_end(&self, info: &OperationInfo, result: &OperationResult) {
//!         println!("Completed in {}ms", result.duration_ms);
//!     }
//!
//!     fn on_request_end(&self, info: &RequestInfo, result: &RequestResult) {
//!         println!("{} {} -> {} ({}ms)", 
//!             info.method, info.url, result.status, result.duration_ms);
//!     }
//! }
//! ```
//!
//! ## HTTPS Enforcement
//!
//! The SDK enforces HTTPS for non-localhost URLs:
//!
//! ```rust
//! use basecamp_sdk_rs::Config;
//!
//! // This will fail
//! let result = Config::builder()
//!     .base_url("http://api.example.com/999")
//!     .build();
//! assert!(result.is_err());
//!
//! // Localhost is allowed
//! let config = Config::builder()
//!     .base_url("http://localhost:8080/999")
//!     .build()?;
//! # Ok::<(), basecamp_sdk_rs::ConfigError>(())
//! ```

pub mod auth;
pub mod client;
pub mod config;
pub mod error;
pub mod hooks;
pub mod http;
pub mod pagination;
pub mod security;
pub mod services;

pub use auth::{
    AuthStrategy, BearerAuth, OAuthToken, OAuthTokenProvider, OnRefreshCallback,
    StaticTokenProvider, TokenProvider,
};
pub use client::{AccountClient, Client, ClientBuilder, ClientError};
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
pub use services::{Authorization, AuthorizationService};
