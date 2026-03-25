# Configuration Specification

## Overview

The configuration module defines SDK behavior including HTTP settings, retry behavior,
pagination limits, and environment variable support.

## Interface: Config

```rust
use std::time::Duration;

/// SDK configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Base URL for the Basecamp API.
    /// Default: "https://3.basecampapi.com"
    pub base_url: String,
    
    /// HTTP request timeout.
    /// Default: 30 seconds
    pub timeout: Duration,
    
    /// Maximum number of retries for transient failures.
    /// Default: 3
    pub max_retries: u32,
    
    /// Base delay for exponential backoff.
    /// Default: 1 second
    pub base_delay: Duration,
    
    /// Maximum jitter for backoff randomization.
    /// Default: 100 milliseconds
    pub max_jitter: Duration,
    
    /// Maximum number of pages to fetch during pagination.
    /// Default: 10,000
    pub max_pages: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_url: "https://3.basecampapi.com".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
            base_delay: Duration::from_secs(1),
            max_jitter: Duration::from_millis(100),
            max_pages: 10_000,
        }
    }
}

impl Config {
    /// Create a new config with defaults.
    pub fn new() -> Self;
    
    /// Load configuration from environment variables.
    /// Environment variables:
    /// - BASECAMP_BASE_URL
    /// - BASECAMP_TIMEOUT (seconds)
    /// - BASECAMP_MAX_RETRIES
    pub fn from_env() -> Result<Self, ConfigError>;
    
    /// Create a builder for custom configuration.
    pub fn builder() -> ConfigBuilder;
}
```

## Interface: ConfigBuilder

```rust
pub struct ConfigBuilder {
    // private fields
}

impl ConfigBuilder {
    pub fn new() -> Self;
    
    /// Set the base URL.
    pub fn base_url(self, url: impl Into<String>) -> Self;
    
    /// Set the request timeout.
    pub fn timeout(self, timeout: Duration) -> Self;
    
    /// Set the maximum number of retries.
    pub fn max_retries(self, retries: u32) -> Self;
    
    /// Set the base delay for backoff.
    pub fn base_delay(self, delay: Duration) -> Self;
    
    /// Set the maximum jitter.
    pub fn max_jitter(self, jitter: Duration) -> Self;
    
    /// Set the maximum number of pages.
    pub fn max_pages(self, pages: u32) -> Self;
    
    /// Build the configuration.
    pub fn build(self) -> Result<Config, ConfigError>;
}
```

## Validation Rules

```rust
pub enum ConfigError {
    /// Base URL is not a valid URL.
    InvalidBaseUrl { value: String, reason: String },
    
    /// Timeout is zero or negative.
    InvalidTimeout { value: Duration },
    
    /// Max retries exceeds limit.
    InvalidMaxRetries { value: u32, max: u32 },
    
    /// Base delay is zero.
    InvalidBaseDelay { value: Duration },
    
    /// Max jitter exceeds base delay.
    InvalidMaxJitter { value: Duration, base_delay: Duration },
}
```

### Constraints

- `[static]` `timeout` must be > 0
- `[static]` `max_retries` must be in range 0..=10
- `[static]` `base_delay` must be > 0
- `[static]` `max_jitter` must be <= `base_delay`
- `[static]` `max_pages` must be > 0
- `[unit]` Invalid configuration returns appropriate `ConfigError`

## Environment Variables

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `BASECAMP_BASE_URL` | String | `https://3.basecampapi.com` | API base URL |
| `BASECAMP_TIMEOUT` | u64 | `30` | Request timeout in seconds |
| `BASECAMP_MAX_RETRIES` | u32 | `3` | Maximum retry attempts |

### Environment Parsing

```rust
impl Config {
    /// Parse environment variable with fallback to default.
    fn parse_env_or<T: FromStr>(key: &str, default: T) -> T;
}
```

- `[unit]` `from_env()` with no variables returns defaults
- `[unit]` `from_env()` parses valid environment variables
- `[unit]` `from_env()` ignores invalid values with warning log
- `[conformance]` Environment variables override defaults

## Immutability

`Config` is immutable after construction. To modify, create a new instance:

```rust
let config = Config::default();
let modified = Config::builder()
    .base_url("https://custom.api.com".to_string())
    .timeout(config.timeout)
    .max_retries(config.max_retries)
    .base_delay(config.base_delay)
    .max_jitter(config.max_jitter)
    .max_pages(config.max_pages)
    .build()?;
```

## Verification

- `[unit]` Default values match specification
- `[unit]` Builder produces equivalent config to direct construction
- `[unit]` `from_env()` handles missing variables gracefully
- `[unit]` `from_env()` parses valid values correctly
- `[unit]` Validation catches all constraint violations
- `[conformance]` Configuration affects HTTP client behavior
