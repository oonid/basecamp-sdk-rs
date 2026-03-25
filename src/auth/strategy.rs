use reqwest::header::HeaderMap;

pub trait AuthStrategy: Send + Sync {
    fn authenticate(&self, headers: &mut HeaderMap);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAuthStrategy {
        token: String,
    }

    impl MockAuthStrategy {
        fn new(token: &str) -> Self {
            Self {
                token: token.to_string(),
            }
        }
    }

    impl AuthStrategy for MockAuthStrategy {
        fn authenticate(&self, headers: &mut HeaderMap) {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.token).parse().unwrap(),
            );
        }
    }

    #[test]
    fn test_auth_strategy_adds_header() {
        let auth = MockAuthStrategy::new("test-token");
        let mut headers = HeaderMap::new();

        auth.authenticate(&mut headers);

        let auth_header = headers.get(reqwest::header::AUTHORIZATION).unwrap();
        assert_eq!(auth_header.to_str().unwrap(), "Bearer test-token");
    }

    #[test]
    fn test_auth_strategy_is_send_sync() {
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<MockAuthStrategy>();
        assert_send_sync::<dyn AuthStrategy>();
    }
}
