use std::time::Duration;
use thiserror::Error;

pub const DEFAULT_BASE_URL: &str = "https://3.basecampapi.com";
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_MAX_RETRIES: u32 = 3;
pub const DEFAULT_BASE_DELAY_SECS: u64 = 1;
pub const DEFAULT_MAX_JITTER_MS: u64 = 100;
pub const DEFAULT_MAX_PAGES: u32 = 10_000;

#[derive(Debug, Clone, Error)]
pub enum ConfigError {
    #[error("Base URL is not valid: {reason}")]
    InvalidBaseUrl { reason: String },

    #[error("Timeout must be positive")]
    InvalidTimeout,

    #[error("Max retries must be between 0 and 10")]
    InvalidMaxRetries,

    #[error("Base delay must be positive")]
    InvalidBaseDelay,

    #[error("Max jitter must be non-negative and <= base delay")]
    InvalidMaxJitter,

    #[error("Max pages must be positive")]
    InvalidMaxPages,

    #[error("Max items must be positive if set")]
    InvalidMaxItems,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub base_url: String,
    pub timeout: Duration,
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_jitter: Duration,
    pub max_pages: u32,
    pub max_items: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay: Duration::from_secs(DEFAULT_BASE_DELAY_SECS),
            max_jitter: Duration::from_millis(DEFAULT_MAX_JITTER_MS),
            max_pages: DEFAULT_MAX_PAGES,
            max_items: None,
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    pub fn from_env() -> Result<Self, ConfigError> {
        let mut builder = Self::builder();

        if let Ok(val) = std::env::var("BASECAMP_BASE_URL") {
            builder = builder.base_url(&val);
        }

        if let Ok(val) = std::env::var("BASECAMP_TIMEOUT") {
            if let Ok(secs) = val.parse::<u64>() {
                builder = builder.timeout(Duration::from_secs(secs));
            }
        }

        if let Ok(val) = std::env::var("BASECAMP_MAX_RETRIES") {
            if let Ok(retries) = val.parse::<u32>() {
                builder = builder.max_retries(retries);
            }
        }

        builder.build()
    }
}

pub struct ConfigBuilder {
    base_url: Option<String>,
    timeout: Option<Duration>,
    max_retries: Option<u32>,
    base_delay: Option<Duration>,
    max_jitter: Option<Duration>,
    max_pages: Option<u32>,
    max_items: Option<u32>,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            base_url: None,
            timeout: None,
            max_retries: None,
            base_delay: None,
            max_jitter: None,
            max_pages: None,
            max_items: None,
        }
    }

    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = Some(retries);
        self
    }

    pub fn base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = Some(delay);
        self
    }

    pub fn max_jitter(mut self, jitter: Duration) -> Self {
        self.max_jitter = Some(jitter);
        self
    }

    pub fn max_pages(mut self, pages: u32) -> Self {
        self.max_pages = Some(pages);
        self
    }

    pub fn max_items(mut self, items: u32) -> Self {
        self.max_items = Some(items);
        self
    }

    pub fn build(self) -> Result<Config, ConfigError> {
        let base_url = self
            .base_url
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let base_url = base_url.trim_end_matches('/').to_string();

        let timeout = self
            .timeout
            .unwrap_or_else(|| Duration::from_secs(DEFAULT_TIMEOUT_SECS));

        let max_retries = self.max_retries.unwrap_or(DEFAULT_MAX_RETRIES);

        let base_delay = self
            .base_delay
            .unwrap_or_else(|| Duration::from_secs(DEFAULT_BASE_DELAY_SECS));

        let max_jitter = self
            .max_jitter
            .unwrap_or_else(|| Duration::from_millis(DEFAULT_MAX_JITTER_MS));

        let max_pages = self.max_pages.unwrap_or(DEFAULT_MAX_PAGES);

        let max_items = self.max_items;

        if timeout.is_zero() {
            return Err(ConfigError::InvalidTimeout);
        }

        if max_retries > 10 {
            return Err(ConfigError::InvalidMaxRetries);
        }

        if base_delay.is_zero() {
            return Err(ConfigError::InvalidBaseDelay);
        }

        if max_jitter > base_delay {
            return Err(ConfigError::InvalidMaxJitter);
        }

        if max_pages == 0 {
            return Err(ConfigError::InvalidMaxPages);
        }

        if let Some(items) = max_items {
            if items == 0 {
                return Err(ConfigError::InvalidMaxItems);
            }
        }

        Ok(Config {
            base_url,
            timeout,
            max_retries,
            base_delay,
            max_jitter,
            max_pages,
            max_items,
        })
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod defaults {
        use super::*;

        #[test]
        fn test_default_config() {
            let config = Config::default();
            assert_eq!(config.base_url, "https://3.basecampapi.com");
            assert_eq!(config.timeout, Duration::from_secs(30));
            assert_eq!(config.max_retries, 3);
            assert_eq!(config.base_delay, Duration::from_secs(1));
            assert_eq!(config.max_jitter, Duration::from_millis(100));
            assert_eq!(config.max_pages, 10_000);
            assert_eq!(config.max_items, None);
        }

        #[test]
        fn test_config_new() {
            let config = Config::new();
            assert_eq!(config.base_url, DEFAULT_BASE_URL);
        }
    }

    mod builder {
        use super::*;

        #[test]
        fn test_builder_defaults() {
            let config = Config::builder().build().unwrap();
            let default = Config::default();
            assert_eq!(config, default);
        }

        #[test]
        fn test_builder_base_url() {
            let config = Config::builder()
                .base_url("https://custom.api.com")
                .build()
                .unwrap();
            assert_eq!(config.base_url, "https://custom.api.com");
        }

        #[test]
        fn test_builder_base_url_trailing_slash_removed() {
            let config = Config::builder()
                .base_url("https://api.example.com/")
                .build()
                .unwrap();
            assert_eq!(config.base_url, "https://api.example.com");
        }

        #[test]
        fn test_builder_timeout() {
            let config = Config::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap();
            assert_eq!(config.timeout, Duration::from_secs(60));
        }

        #[test]
        fn test_builder_max_retries() {
            let config = Config::builder().max_retries(5).build().unwrap();
            assert_eq!(config.max_retries, 5);
        }

        #[test]
        fn test_builder_max_items() {
            let config = Config::builder().max_items(100).build().unwrap();
            assert_eq!(config.max_items, Some(100));
        }

        #[test]
        fn test_builder_zero_retries() {
            let config = Config::builder().max_retries(0).build().unwrap();
            assert_eq!(config.max_retries, 0);
        }
    }

    mod validation {
        use super::*;

        #[test]
        fn test_zero_timeout_rejected() {
            let result = Config::builder().timeout(Duration::from_secs(0)).build();
            assert!(matches!(result, Err(ConfigError::InvalidTimeout)));
        }

        #[test]
        fn test_max_retries_too_high_rejected() {
            let result = Config::builder().max_retries(11).build();
            assert!(matches!(result, Err(ConfigError::InvalidMaxRetries)));
        }

        #[test]
        fn test_max_retries_10_accepted() {
            let config = Config::builder().max_retries(10).build().unwrap();
            assert_eq!(config.max_retries, 10);
        }

        #[test]
        fn test_zero_base_delay_rejected() {
            let result = Config::builder().base_delay(Duration::from_secs(0)).build();
            assert!(matches!(result, Err(ConfigError::InvalidBaseDelay)));
        }

        #[test]
        fn test_jitter_exceeds_base_delay_rejected() {
            let result = Config::builder()
                .base_delay(Duration::from_millis(50))
                .max_jitter(Duration::from_millis(100))
                .build();
            assert!(matches!(result, Err(ConfigError::InvalidMaxJitter)));
        }

        #[test]
        fn test_jitter_equals_base_delay_accepted() {
            let config = Config::builder()
                .base_delay(Duration::from_millis(100))
                .max_jitter(Duration::from_millis(100))
                .build()
                .unwrap();
            assert_eq!(config.max_jitter, Duration::from_millis(100));
        }

        #[test]
        fn test_zero_max_pages_rejected() {
            let result = Config::builder().max_pages(0).build();
            assert!(matches!(result, Err(ConfigError::InvalidMaxPages)));
        }

        #[test]
        fn test_zero_max_items_rejected() {
            let result = Config::builder().max_items(0).build();
            assert!(matches!(result, Err(ConfigError::InvalidMaxItems)));
        }
    }

    mod from_env {
        use super::*;
        use std::env;

        #[test]
        fn test_from_env_no_vars() {
            env::remove_var("BASECAMP_BASE_URL");
            env::remove_var("BASECAMP_TIMEOUT");
            env::remove_var("BASECAMP_MAX_RETRIES");

            let config = Config::from_env().unwrap();
            let default = Config::default();
            assert_eq!(config, default);
        }

        #[test]
        fn test_from_env_with_base_url() {
            env::set_var("BASECAMP_BASE_URL", "https://custom.api.com");
            env::remove_var("BASECAMP_TIMEOUT");
            env::remove_var("BASECAMP_MAX_RETRIES");

            let config = Config::from_env().unwrap();
            assert_eq!(config.base_url, "https://custom.api.com");

            env::remove_var("BASECAMP_BASE_URL");
        }

        #[test]
        fn test_from_env_with_timeout() {
            env::remove_var("BASECAMP_BASE_URL");
            env::set_var("BASECAMP_TIMEOUT", "60");
            env::remove_var("BASECAMP_MAX_RETRIES");

            let config = Config::from_env().unwrap();
            assert_eq!(config.timeout, Duration::from_secs(60));

            env::remove_var("BASECAMP_TIMEOUT");
        }

        #[test]
        fn test_from_env_with_max_retries() {
            env::remove_var("BASECAMP_BASE_URL");
            env::remove_var("BASECAMP_TIMEOUT");
            env::set_var("BASECAMP_MAX_RETRIES", "5");

            let config = Config::from_env().unwrap();
            assert_eq!(config.max_retries, 5);

            env::remove_var("BASECAMP_MAX_RETRIES");
        }

        #[test]
        fn test_from_env_invalid_timeout_ignored() {
            env::set_var("BASECAMP_TIMEOUT", "invalid");
            env::remove_var("BASECAMP_BASE_URL");
            env::remove_var("BASECAMP_MAX_RETRIES");

            let config = Config::from_env().unwrap();
            assert_eq!(config.timeout, Duration::from_secs(DEFAULT_TIMEOUT_SECS));

            env::remove_var("BASECAMP_TIMEOUT");
        }
    }
}
