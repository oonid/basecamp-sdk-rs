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
//! use basecamp_sdk_rs::Client;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client with access token
//!     let client = Client::new("your-access-token");
//!
//!     // Get account-scoped client
//!     let account = client.for_account(999);
//!
//!     // List all projects
//!     let projects = account.http().get_paginated::<serde_json::Value>("/projects.json", None).await?;
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
//! use basecamp_sdk_rs::{Client, Config};
//!
//! let config = Config::builder()
//!     .base_url("https://3.basecampapi.com/999")
//!     .timeout(std::time::Duration::from_secs(60))
//!     .max_retries(5)
//!     .max_pages(100)
//!     .max_items(1000)
//!     .build()?;
//!
//! let client = Client::builder()
//!     .access_token("your-token")
//!     .config(config)
//!     .build()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
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
//! use basecamp_sdk_rs::Client;
//!
//! // Simple case
//! let client = Client::new("your-token");
//!
//! // Or with builder
//! let client = Client::builder()
//!     .access_token("your-token")
//!     .build()?;
//! # Ok::<(), basecamp_sdk_rs::ClientError>(())
//! ```
//!
//! ### OAuth 2.0 with Token Refresh
//!
//! ```rust,no_run
//! use basecamp_sdk_rs::{Client, OAuthTokenProvider};
//! use std::sync::Arc;
//!
//! // Use OAuthTokenProvider for automatic token refresh
//! let provider = OAuthTokenProvider::new(
//!     "your-access-token",
//!     "your-client-id",
//!     "your-client-secret",
//! );
//!
//! let client = Client::builder()
//!     .token_provider(provider)
//!     .build()?;
//! # Ok::<(), basecamp_sdk_rs::ClientError>(())
//! ```
//!
//! ## Error Handling
//!
//! The SDK provides structured errors with codes for programmatic handling:
//!
//! ```rust,no_run
//! use basecamp_sdk_rs::{Client, BasecampError};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let client = Client::new("token");
//! let account = client.for_account(999);
//!
//! match account.http().get("/projects/99999.json", None).await {
//!     Ok(response) => println!("Success: {:?}", response.status()),
//!     Err(e) => {
//!         match &e {
//!             BasecampError::NotFound { resource_id, .. } => {
//!                 eprintln!("Not found: {:?}", resource_id);
//!             }
//!             BasecampError::AuthRequired { hint, .. } => {
//!                 eprintln!("Auth required. Hint: {:?}", hint);
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
//! use basecamp_sdk_rs::{Client, Config};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = Config::builder()
//!     .max_pages(10)      // Limit pages
//!     .max_items(500)     // Limit total items
//!     .build()?;
//!
//! let client = Client::builder()
//!     .access_token("token")
//!     .config(config)
//!     .build()?;
//!
//! let account = client.for_account(999);
//!
//! // Fetch all pages automatically
//! let result = account.http().get_paginated::<serde_json::Value>("/projects.json", None).await?;
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
//! use basecamp_sdk_rs::{Client, ConsoleHooks};
//!
//! let client = Client::builder()
//!     .access_token("token")
//!     .hooks(ConsoleHooks::new())
//!     .build()?;
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
//!         println!("Completed in {:?}", result.duration);
//!     }
//!
//!     fn on_request_end(&self, info: &RequestInfo, result: &RequestResult) {
//!         println!("{} {} -> {} ({:?})",
//!             info.method, info.url, result.status.unwrap_or(0), result.duration);
//!     }
//! }
//! ```
//!
//! ## HTTPS Enforcement
//!
//! The SDK enforces HTTPS for non-localhost URLs at request time:
//!
//! ```rust,no_run
//! use basecamp_sdk_rs::Client;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // HTTP URLs are rejected for non-localhost when making requests
//! let client = Client::builder()
//!     .access_token("token")
//!     .build()?;
//!
//! // This would fail at request time:
//! // let account = client.for_account(999);
//! // account.http().get_absolute("http://api.example.com/data").await?;
//! // // Error: Usage error: HTTPS required for non-localhost URLs
//!
//! // Localhost URLs are allowed:
//! let account = client.for_account(999);
//! // account.http().get_absolute("http://localhost:8080/data").await?;
//! # Ok(())
//! # }
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
