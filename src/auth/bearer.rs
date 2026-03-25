use crate::auth::{AuthStrategy, StaticTokenProvider, TokenProvider};
use reqwest::header::HeaderMap;
use std::sync::Arc;

pub struct BearerAuth {
    provider: Arc<dyn TokenProvider>,
}

impl BearerAuth {
    pub fn new(provider: impl TokenProvider + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }

    pub fn from_token(token: impl Into<String>) -> Self {
        Self {
            provider: Arc::new(StaticTokenProvider::new(token)),
        }
    }

    pub fn provider(&self) -> &dyn TokenProvider {
        self.provider.as_ref()
    }
}

impl AuthStrategy for BearerAuth {
    fn authenticate(&self, headers: &mut HeaderMap) {
        let token = self.provider.access_token();
        let header_value = format!("Bearer {}", token);
        if let Ok(value) = header_value.parse() {
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
    }

    fn token_provider(&self) -> Option<Arc<dyn TokenProvider>> {
        Some(self.provider.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header;

    mod bearer_auth {
        use super::*;

        #[test]
        fn test_from_token_creates_bearer_auth() {
            let auth = BearerAuth::from_token("my-secret-token");
            let mut headers = HeaderMap::new();
            auth.authenticate(&mut headers);

            let auth_header = headers.get(header::AUTHORIZATION).unwrap();
            assert_eq!(auth_header.to_str().unwrap(), "Bearer my-secret-token");
        }

        #[test]
        fn test_new_with_static_provider() {
            let provider = StaticTokenProvider::new("test-token");
            let auth = BearerAuth::new(provider);
            let mut headers = HeaderMap::new();
            auth.authenticate(&mut headers);

            let auth_header = headers.get(header::AUTHORIZATION).unwrap();
            assert_eq!(auth_header.to_str().unwrap(), "Bearer test-token");
        }

        #[test]
        fn test_authorization_header_format() {
            let auth = BearerAuth::from_token("abc123");
            let mut headers = HeaderMap::new();
            auth.authenticate(&mut headers);

            let auth_header = headers.get(header::AUTHORIZATION).unwrap();
            let header_str = auth_header.to_str().unwrap();

            assert!(header_str.starts_with("Bearer "));
            assert_eq!(header_str, "Bearer abc123");
        }

        #[test]
        fn test_empty_headers_receives_auth() {
            let auth = BearerAuth::from_token("token");
            let mut headers = HeaderMap::new();
            assert!(headers.is_empty());

            auth.authenticate(&mut headers);
            assert!(!headers.is_empty());
            assert_eq!(headers.len(), 1);
        }

        #[test]
        fn test_overwrites_existing_auth_header() {
            let auth = BearerAuth::from_token("new-token");
            let mut headers = HeaderMap::new();
            headers.insert(header::AUTHORIZATION, "Bearer old-token".parse().unwrap());

            auth.authenticate(&mut headers);

            let auth_header = headers.get(header::AUTHORIZATION).unwrap();
            assert_eq!(auth_header.to_str().unwrap(), "Bearer new-token");
        }

        #[test]
        fn test_preserves_other_headers() {
            let auth = BearerAuth::from_token("token");
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
            headers.insert(header::ACCEPT, "application/json".parse().unwrap());

            auth.authenticate(&mut headers);

            assert_eq!(headers.len(), 3);
            assert_eq!(
                headers.get(header::CONTENT_TYPE).unwrap().to_str().unwrap(),
                "application/json"
            );
            assert_eq!(
                headers.get(header::ACCEPT).unwrap().to_str().unwrap(),
                "application/json"
            );
        }

        #[test]
        fn test_header_injection_prevention() {
            let malicious_token = "valid-token\r\nX-Injected: malicious";
            let auth = BearerAuth::from_token(malicious_token);
            let mut headers = HeaderMap::new();
            auth.authenticate(&mut headers);

            assert!(!headers.contains_key("X-Injected"));
            assert!(!headers.contains_key("x-injected"));

            if let Some(auth_header) = headers.get(header::AUTHORIZATION) {
                if let Ok(header_str) = auth_header.to_str() {
                    assert!(!header_str.contains("X-Injected"));
                }
            }
        }

        #[test]
        fn test_token_with_special_characters() {
            let special_token = "token_with-special.chars:123";
            let auth = BearerAuth::from_token(special_token);
            let mut headers = HeaderMap::new();
            auth.authenticate(&mut headers);

            let auth_header = headers.get(header::AUTHORIZATION).unwrap();
            assert!(auth_header.to_str().unwrap().contains(special_token));
        }

        #[test]
        fn test_provider_access() {
            let auth = BearerAuth::from_token("test-token");
            assert_eq!(auth.provider().access_token(), "test-token");
            assert!(!auth.provider().refreshable());
        }

        #[test]
        fn test_token_provider_returns_some() {
            let auth = BearerAuth::from_token("test-token");
            let provider = auth.token_provider();
            assert!(provider.is_some());
            let provider = provider.unwrap();
            assert_eq!(provider.access_token(), "test-token");
        }

        #[test]
        fn test_is_send_sync() {
            fn assert_send_sync<T: Send + Sync>() {}
            assert_send_sync::<BearerAuth>();
        }

        #[test]
        fn test_multiple_authenticate_calls() {
            let auth = BearerAuth::from_token("consistent-token");

            let mut headers1 = HeaderMap::new();
            auth.authenticate(&mut headers1);

            let mut headers2 = HeaderMap::new();
            auth.authenticate(&mut headers2);

            assert_eq!(
                headers1.get(header::AUTHORIZATION),
                headers2.get(header::AUTHORIZATION)
            );
        }
    }
}
