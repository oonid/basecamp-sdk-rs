mod strategy;
mod token;

pub use strategy::AuthStrategy;
pub use token::TokenProvider;

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;

    struct MockTokenProvider;

    impl TokenProvider for MockTokenProvider {
        fn access_token(&self) -> String {
            "mock-token".to_string()
        }

        fn refresh(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
            Box::pin(async { false })
        }

        fn refreshable(&self) -> bool {
            false
        }
    }

    struct MockAuthStrategy;

    impl AuthStrategy for MockAuthStrategy {
        fn authenticate(&self, headers: &mut reqwest::header::HeaderMap) {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                "Bearer mock".parse().unwrap(),
            );
        }
    }

    #[test]
    fn test_traits_are_send_sync() {
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}

        assert_send_sync::<dyn AuthStrategy>();
        assert_send_sync::<dyn TokenProvider>();
        assert_send_sync::<MockTokenProvider>();
        assert_send_sync::<MockAuthStrategy>();
    }
}
