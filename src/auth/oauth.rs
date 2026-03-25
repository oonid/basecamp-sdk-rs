use crate::auth::TokenProvider;
use chrono::{DateTime, Utc};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

pub const DEFAULT_TOKEN_URL: &str = "https://launchpad.37signals.com/authorization/token";
pub const DEFAULT_EXPIRY_BUFFER_SECS: i64 = 60;

#[derive(Debug, Clone)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub scope: Option<String>,
}

impl OAuthToken {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            refresh_token: None,
            expires_at: None,
            scope: None,
        }
    }

    pub fn is_expired(&self, buffer_seconds: i64) -> bool {
        match self.expires_at {
            Some(expires_at) => {
                let buffer = chrono::Duration::seconds(buffer_seconds);
                Utc::now() + buffer >= expires_at
            }
            None => false,
        }
    }
}

pub type OnRefreshCallback = Arc<dyn Fn(&OAuthToken) + Send + Sync>;

pub struct OAuthTokenProvider {
    client: reqwest::Client,
    token_url: String,
    client_id: String,
    client_secret: String,
    token: Arc<RwLock<OAuthToken>>,
    on_refresh: Option<OnRefreshCallback>,
}

impl OAuthTokenProvider {
    pub fn new(
        access_token: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            token_url: DEFAULT_TOKEN_URL.to_string(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            token: Arc::new(RwLock::new(OAuthToken::new(access_token))),
            on_refresh: None,
        }
    }

    pub fn with_refresh_token(self, token: impl Into<String>) -> Self {
        let mut oauth_token = self.token.try_write().unwrap();
        oauth_token.refresh_token = Some(token.into());
        drop(oauth_token);
        self
    }

    pub fn with_expires_at(self, expires_at: DateTime<Utc>) -> Self {
        let mut oauth_token = self.token.try_write().unwrap();
        oauth_token.expires_at = Some(expires_at);
        drop(oauth_token);
        self
    }

    pub fn with_on_refresh(self, callback: OnRefreshCallback) -> Self {
        Self {
            on_refresh: Some(callback),
            ..self
        }
    }

    pub fn with_token_url(self, token_url: impl Into<String>) -> Self {
        Self {
            token_url: token_url.into(),
            ..self
        }
    }

    pub fn with_client(self, client: reqwest::Client) -> Self {
        Self { client, ..self }
    }

    pub async fn current_token(&self) -> OAuthToken {
        self.token.read().await.clone()
    }
}

impl TokenProvider for OAuthTokenProvider {
    fn access_token(&self) -> String {
        let token = self.token.try_read();
        match token {
            Ok(t) => t.access_token.clone(),
            Err(_) => {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async { self.token.read().await.access_token.clone() })
            }
        }
    }

    fn refresh(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async {
            let refresh_token = {
                let token = self.token.read().await;
                token.refresh_token.clone()
            };

            let Some(refresh_token) = refresh_token else {
                return false;
            };

            let response = self
                .client
                .post(&self.token_url)
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", &refresh_token),
                    ("client_id", &self.client_id),
                    ("client_secret", &self.client_secret),
                ])
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let json: serde_json::Value = match resp.json().await {
                        Ok(j) => j,
                        Err(_) => return false,
                    };

                    let new_access_token = json.get("access_token").and_then(|v| v.as_str());
                    let new_refresh_token = json.get("refresh_token").and_then(|v| v.as_str());
                    let expires_in = json.get("expires_in").and_then(|v| v.as_i64());

                    let Some(access_token) = new_access_token else {
                        return false;
                    };

                    let mut token = self.token.write().await;
                    token.access_token = access_token.to_string();

                    if let Some(rt) = new_refresh_token {
                        token.refresh_token = Some(rt.to_string());
                    }

                    if let Some(secs) = expires_in {
                        token.expires_at = Some(Utc::now() + chrono::Duration::seconds(secs));
                    }

                    let updated_token = token.clone();

                    if let Some(callback) = &self.on_refresh {
                        callback(&updated_token);
                    }

                    true
                }
                _ => false,
            }
        })
    }

    fn refreshable(&self) -> bool {
        let token = self.token.try_read();
        match token {
            Ok(t) => t.refresh_token.is_some(),
            Err(_) => {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async { self.token.read().await.refresh_token.is_some() })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod oauth_token {
        use super::*;

        #[test]
        fn test_new_creates_token() {
            let token = OAuthToken::new("test-access-token");
            assert_eq!(token.access_token, "test-access-token");
            assert!(token.refresh_token.is_none());
            assert!(token.expires_at.is_none());
            assert!(token.scope.is_none());
        }

        #[test]
        fn test_is_expired_no_expiry() {
            let token = OAuthToken::new("token");
            assert!(!token.is_expired(60));
        }

        #[test]
        fn test_is_expired_future() {
            let mut token = OAuthToken::new("token");
            token.expires_at = Some(Utc::now() + chrono::Duration::hours(1));
            assert!(!token.is_expired(60));
        }

        #[test]
        fn test_is_expired_past() {
            let mut token = OAuthToken::new("token");
            token.expires_at = Some(Utc::now() - chrono::Duration::hours(1));
            assert!(token.is_expired(60));
        }

        #[test]
        fn test_is_expired_within_buffer() {
            let mut token = OAuthToken::new("token");
            token.expires_at = Some(Utc::now() + chrono::Duration::seconds(30));
            assert!(token.is_expired(60));
        }

        #[test]
        fn test_is_expired_outside_buffer() {
            let mut token = OAuthToken::new("token");
            token.expires_at = Some(Utc::now() + chrono::Duration::seconds(120));
            assert!(!token.is_expired(60));
        }

        #[test]
        fn test_is_expired_zero_buffer() {
            let mut token = OAuthToken::new("token");
            token.expires_at = Some(Utc::now() + chrono::Duration::seconds(30));
            assert!(!token.is_expired(0));
        }
    }

    mod oauth_token_provider {
        use super::*;
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[test]
        fn test_new_creates_provider() {
            let provider = OAuthTokenProvider::new("access", "client_id", "secret");
            assert_eq!(provider.access_token(), "access");
        }

        #[test]
        fn test_with_refresh_token() {
            let provider =
                OAuthTokenProvider::new("access", "client_id", "secret").with_refresh_token("refresh");

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let token = provider.current_token().await;
                assert_eq!(token.refresh_token, Some("refresh".to_string()));
            });
        }

        #[test]
        fn test_with_expires_at() {
            let expires = Utc::now() + chrono::Duration::hours(1);
            let provider =
                OAuthTokenProvider::new("access", "client_id", "secret").with_expires_at(expires);

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let token = provider.current_token().await;
                assert_eq!(token.expires_at, Some(expires));
            });
        }

        #[test]
        fn test_refreshable_without_refresh_token() {
            let provider = OAuthTokenProvider::new("access", "client_id", "secret");
            assert!(!provider.refreshable());
        }

        #[test]
        fn test_refreshable_with_refresh_token() {
            let provider =
                OAuthTokenProvider::new("access", "client_id", "secret").with_refresh_token("refresh");
            assert!(provider.refreshable());
        }

        #[test]
        fn test_access_token_returns_current() {
            let provider = OAuthTokenProvider::new("my-token", "client_id", "secret");
            assert_eq!(provider.access_token(), "my-token");
        }

        #[tokio::test]
        async fn test_refresh_without_refresh_token_returns_false() {
            let provider = OAuthTokenProvider::new("access", "client_id", "secret");
            let result = provider.refresh().await;
            assert!(!result);
        }

        #[test]
        fn test_is_send_sync() {
            fn assert_send_sync<T: Send + Sync>() {}
            assert_send_sync::<OAuthTokenProvider>();
            assert_send_sync::<OAuthToken>();
        }

        #[tokio::test]
        async fn test_current_token() {
            let provider = OAuthTokenProvider::new("access", "client_id", "secret");
            let token = provider.current_token().await;
            assert_eq!(token.access_token, "access");
        }

        #[test]
        fn test_on_refresh_callback_invoked() {
            let call_count = Arc::new(AtomicUsize::new(0));
            let call_count_clone = call_count.clone();

            let callback: OnRefreshCallback = Arc::new(move |_token: &OAuthToken| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
            });

            let provider = OAuthTokenProvider::new("access", "client_id", "secret")
                .with_refresh_token("refresh")
                .with_on_refresh(callback);

            assert!(provider.on_refresh.is_some());
        }

        #[tokio::test]
        async fn test_concurrent_access() {
            let provider = Arc::new(
                OAuthTokenProvider::new("access", "client_id", "secret")
                    .with_refresh_token("refresh"),
            );

            let mut handles = vec![];

            for _ in 0..10 {
                let p = provider.clone();
                handles.push(tokio::spawn(async move {
                    p.access_token()
                }));
            }

            for handle in handles {
                let token = handle.await.unwrap();
                assert_eq!(token, "access");
            }
        }
    }

    mod constants {
        use super::*;

        #[test]
        fn test_default_token_url() {
            assert_eq!(
                DEFAULT_TOKEN_URL,
                "https://launchpad.37signals.com/authorization/token"
            );
        }

        #[test]
        fn test_default_expiry_buffer() {
            assert_eq!(DEFAULT_EXPIRY_BUFFER_SECS, 60);
        }
    }

    mod http_refresh {
        use super::*;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

        #[tokio::test]
        async fn test_refresh_success_updates_token() {
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .and(matchers::path("/token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "new-access-token",
                    "refresh_token": "new-refresh-token",
                    "expires_in": 3600,
                    "token_type": "Bearer"
                })))
                .mount(&mock_server)
                .await;

            let provider = OAuthTokenProvider::new("old-access", "client_id", "secret")
                .with_refresh_token("old-refresh")
                .with_token_url(format!("{}/token", mock_server.uri()));

            let result = provider.refresh().await;
            assert!(result);

            let token = provider.current_token().await;
            assert_eq!(token.access_token, "new-access-token");
            assert_eq!(token.refresh_token, Some("new-refresh-token".to_string()));
            assert!(token.expires_at.is_some());
        }

        #[tokio::test]
        async fn test_refresh_failure_returns_false() {
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                    "error": "invalid_grant"
                })))
                .mount(&mock_server)
                .await;

            let provider = OAuthTokenProvider::new("access", "client_id", "secret")
                .with_refresh_token("refresh")
                .with_token_url(format!("{}/token", mock_server.uri()));

            let result = provider.refresh().await;
            assert!(!result);
        }

        #[tokio::test]
        async fn test_refresh_uses_correct_form_data() {
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .and(matchers::body_string_contains("grant_type=refresh_token"))
                .and(matchers::body_string_contains("refresh_token=my-refresh-token"))
                .and(matchers::body_string_contains("client_id=my-client-id"))
                .and(matchers::body_string_contains("client_secret=my-client-secret"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "new-token"
                })))
                .mount(&mock_server)
                .await;

            let provider = OAuthTokenProvider::new("access", "my-client-id", "my-client-secret")
                .with_refresh_token("my-refresh-token")
                .with_token_url(format!("{}/token", mock_server.uri()));

            let result = provider.refresh().await;
            assert!(result);
        }

        #[tokio::test]
        async fn test_refresh_callback_invoked_on_success() {
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "new-token"
                })))
                .mount(&mock_server)
                .await;

            let call_count = Arc::new(AtomicUsize::new(0));
            let call_count_clone = call_count.clone();

            let callback: OnRefreshCallback = Arc::new(move |token: &OAuthToken| {
                assert_eq!(token.access_token, "new-token");
                call_count_clone.fetch_add(1, Ordering::SeqCst);
            });

            let provider = OAuthTokenProvider::new("access", "client_id", "secret")
                .with_refresh_token("refresh")
                .with_token_url(format!("{}/token", mock_server.uri()))
                .with_on_refresh(callback);

            let result = provider.refresh().await;
            assert!(result);
            assert_eq!(call_count.load(Ordering::SeqCst), 1);
        }

        #[tokio::test]
        async fn test_refresh_no_callback_on_failure() {
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .respond_with(ResponseTemplate::new(401))
                .mount(&mock_server)
                .await;

            let call_count = Arc::new(AtomicUsize::new(0));
            let call_count_clone = call_count.clone();

            let callback: OnRefreshCallback = Arc::new(move |_token: &OAuthToken| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
            });

            let provider = OAuthTokenProvider::new("access", "client_id", "secret")
                .with_refresh_token("refresh")
                .with_token_url(format!("{}/token", mock_server.uri()))
                .with_on_refresh(callback);

            let result = provider.refresh().await;
            assert!(!result);
            assert_eq!(call_count.load(Ordering::SeqCst), 0);
        }

        #[tokio::test]
        async fn test_refresh_handles_missing_access_token() {
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "refresh_token": "new-refresh"
                })))
                .mount(&mock_server)
                .await;

            let provider = OAuthTokenProvider::new("access", "client_id", "secret")
                .with_refresh_token("refresh")
                .with_token_url(format!("{}/token", mock_server.uri()));

            let result = provider.refresh().await;
            assert!(!result);
        }

        #[tokio::test]
        async fn test_refresh_updates_only_access_token_minimal_response() {
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "minimal-token"
                })))
                .mount(&mock_server)
                .await;

            let provider = OAuthTokenProvider::new("access", "client_id", "secret")
                .with_refresh_token("original-refresh")
                .with_token_url(format!("{}/token", mock_server.uri()));

            let result = provider.refresh().await;
            assert!(result);

            let token = provider.current_token().await;
            assert_eq!(token.access_token, "minimal-token");
            assert_eq!(token.refresh_token, Some("original-refresh".to_string()));
            assert!(token.expires_at.is_none());
        }
    }
}
