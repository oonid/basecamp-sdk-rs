use std::future::Future;
use std::pin::Pin;

pub trait TokenProvider: Send + Sync {
    fn access_token(&self) -> String;

    fn refresh(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;

    fn refreshable(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTokenProvider {
        token: String,
    }

    impl MockTokenProvider {
        fn new(token: &str) -> Self {
            Self {
                token: token.to_string(),
            }
        }
    }

    impl TokenProvider for MockTokenProvider {
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

    #[test]
    fn test_access_token_returns_token() {
        let provider = MockTokenProvider::new("test-token-123");
        assert_eq!(provider.access_token(), "test-token-123");
    }

    #[test]
    fn test_refreshable_returns_false() {
        let provider = MockTokenProvider::new("token");
        assert!(!provider.refreshable());
    }

    #[tokio::test]
    async fn test_refresh_returns_false() {
        let provider = MockTokenProvider::new("token");
        let result = provider.refresh().await;
        assert!(!result);
    }

    #[test]
    fn test_token_provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<MockTokenProvider>();
        assert_send_sync::<dyn TokenProvider>();
    }
}
