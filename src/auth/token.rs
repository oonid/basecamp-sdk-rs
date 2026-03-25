use std::future::Future;
use std::pin::Pin;

pub trait TokenProvider: Send + Sync {
    fn access_token(&self) -> String;

    fn refresh(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;

    fn refreshable(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct StaticTokenProvider {
    token: String,
}

impl StaticTokenProvider {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }
}

impl TokenProvider for StaticTokenProvider {
    fn access_token(&self) -> String {
        self.token.clone()
    }

    fn refresh(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { false })
    }

    fn refreshable(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod static_token_provider {
        use super::*;

        #[test]
        fn test_new_creates_provider() {
            let provider = StaticTokenProvider::new("my-secret-token");
            assert_eq!(provider.access_token(), "my-secret-token");
        }

        #[test]
        fn test_access_token_returns_configured_token() {
            let provider = StaticTokenProvider::new("test-token-123");
            assert_eq!(provider.access_token(), "test-token-123");
        }

        #[test]
        fn test_access_token_returns_same_token_on_multiple_calls() {
            let provider = StaticTokenProvider::new("consistent-token");
            assert_eq!(provider.access_token(), "consistent-token");
            assert_eq!(provider.access_token(), "consistent-token");
            assert_eq!(provider.access_token(), "consistent-token");
        }

        #[test]
        fn test_refreshable_returns_false() {
            let provider = StaticTokenProvider::new("token");
            assert!(!provider.refreshable());
        }

        #[tokio::test]
        async fn test_refresh_returns_false() {
            let provider = StaticTokenProvider::new("token");
            let result = provider.refresh().await;
            assert!(!result);
        }

        #[tokio::test]
        async fn test_refresh_always_returns_false() {
            let provider = StaticTokenProvider::new("token");
            assert!(!provider.refresh().await);
            assert!(!provider.refresh().await);
            assert!(!provider.refresh().await);
        }

        #[test]
        fn test_clone() {
            let provider = StaticTokenProvider::new("clonable-token");
            let cloned = provider.clone();
            assert_eq!(cloned.access_token(), "clonable-token");
        }

        #[test]
        fn test_is_send_sync() {
            fn assert_send_sync<T: Send + Sync>() {}
            assert_send_sync::<StaticTokenProvider>();
        }
    }
}
