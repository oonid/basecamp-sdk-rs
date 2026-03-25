use crate::auth::{AuthStrategy, BearerAuth, TokenProvider};
use crate::config::Config;
use crate::hooks::BasecampHooks;
use crate::http::HttpClient;
use crate::services::AuthorizationService;
use once_cell::sync::OnceCell;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");
const API_VERSION: &str = "3";

struct TokenProviderWrapper(Arc<dyn TokenProvider>);

impl TokenProvider for TokenProviderWrapper {
    fn access_token(&self) -> String {
        self.0.access_token()
    }

    fn refresh(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        self.0.refresh()
    }

    fn refreshable(&self) -> bool {
        self.0.refreshable()
    }
}

#[derive(Debug, Clone)]
pub enum ClientError {
    AmbiguousAuth { message: String },
    NoAuthProvider { message: String },
    Config { message: String },
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::AmbiguousAuth { message } => write!(f, "{}", message),
            ClientError::NoAuthProvider { message } => write!(f, "{}", message),
            ClientError::Config { message } => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for ClientError {}

enum AuthSource {
    None,
    AccessToken(String),
    TokenProvider(Arc<dyn TokenProvider>),
    Auth(Arc<dyn AuthStrategy>),
}

pub struct ClientBuilder {
    auth_source: AuthSource,
    config: Option<Config>,
    hooks: Option<Arc<dyn BasecampHooks>>,
    user_agent: Option<String>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self {
            auth_source: AuthSource::None,
            config: None,
            hooks: None,
            user_agent: None,
        }
    }

    pub fn access_token(mut self, token: impl Into<String>) -> Self {
        match self.auth_source {
            AuthSource::None => {
                self.auth_source = AuthSource::AccessToken(token.into());
            }
            _ => {
                self.auth_source = AuthSource::Auth(Arc::new(AmbiguousAuthMarker));
            }
        }
        self
    }

    pub fn token_provider(mut self, provider: impl TokenProvider + 'static) -> Self {
        match self.auth_source {
            AuthSource::None => {
                self.auth_source = AuthSource::TokenProvider(Arc::new(provider));
            }
            _ => {
                self.auth_source = AuthSource::Auth(Arc::new(AmbiguousAuthMarker));
            }
        }
        self
    }

    pub fn auth(mut self, auth: impl AuthStrategy + 'static) -> Self {
        match self.auth_source {
            AuthSource::None => {
                self.auth_source = AuthSource::Auth(Arc::new(auth));
            }
            _ => {
                self.auth_source = AuthSource::Auth(Arc::new(AmbiguousAuthMarker));
            }
        }
        self
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn hooks(mut self, hooks: impl BasecampHooks + 'static) -> Self {
        self.hooks = Some(Arc::new(hooks));
        self
    }

    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    pub fn build(self) -> Result<Client, ClientError> {
        let auth: Arc<dyn AuthStrategy> = match self.auth_source {
            AuthSource::None => {
                return Err(ClientError::NoAuthProvider {
                    message: "No authentication provider specified. Use access_token(), token_provider(), or auth()".to_string(),
                });
            }
            AuthSource::AccessToken(token) => Arc::new(BearerAuth::from_token(token)),
            AuthSource::TokenProvider(provider) => {
                Arc::new(BearerAuth::new(TokenProviderWrapper(provider)))
            }
            AuthSource::Auth(auth) => auth,
        };

        if auth.as_any().is::<AmbiguousAuthMarker>() {
            return Err(ClientError::AmbiguousAuth {
                message: "Multiple authentication sources specified. Use exactly one of: access_token(), token_provider(), or auth()".to_string(),
            });
        }

        let config = self.config.unwrap_or_default();
        let user_agent = self
            .user_agent
            .unwrap_or_else(|| format!("basecamp-sdk-rust/{} (api:{})", SDK_VERSION, API_VERSION));

        let mut http = HttpClient::new(config, BearerAuthShim(auth.clone())).map_err(|e| {
            ClientError::Config {
                message: e.to_string(),
            }
        })?;

        http = http.with_user_agent(user_agent);

        let http = if let Some(hooks) = self.hooks {
            Arc::new(http.with_hooks(hooks))
        } else {
            Arc::new(http)
        };

        let authorization = AuthorizationService::new(http.clone());

        Ok(Client {
            http,
            auth,
            authorization,
            closed: Arc::new(AtomicBool::new(false)),
        })
    }
}

struct AmbiguousAuthMarker;

impl AuthStrategy for AmbiguousAuthMarker {
    fn authenticate(&self, _headers: &mut reqwest::header::HeaderMap) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct BearerAuthShim(Arc<dyn AuthStrategy>);

impl AuthStrategy for BearerAuthShim {
    fn authenticate(&self, headers: &mut reqwest::header::HeaderMap) {
        self.0.authenticate(headers);
    }

    fn token_provider(&self) -> Option<Arc<dyn TokenProvider>> {
        self.0.token_provider()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self.0.as_any()
    }
}

pub struct Client {
    http: Arc<HttpClient>,
    auth: Arc<dyn AuthStrategy>,
    authorization: AuthorizationService,
    closed: Arc<AtomicBool>,
}

impl Client {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self::builder()
            .access_token(access_token)
            .build()
            .expect("Default config should always succeed")
    }

    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub fn for_account(&self, account_id: impl Into<i64>) -> AccountClient {
        AccountClient {
            account_id: account_id.into(),
            http: self.http.clone(),
            projects: OnceCell::new(),
        }
    }

    pub fn authorization(&self) -> &AuthorizationService {
        &self.authorization
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub async fn close(self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    pub fn http(&self) -> &HttpClient {
        &self.http
    }
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Self {
            http: self.http.clone(),
            auth: self.auth.clone(),
            authorization: AuthorizationService::new(self.http.clone()),
            closed: self.closed.clone(),
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
    }
}

pub struct AccountClient {
    account_id: i64,
    http: Arc<HttpClient>,
    projects: OnceCell<crate::services::ProjectsService>,
}

impl AccountClient {
    pub fn account_id(&self) -> i64 {
        self.account_id
    }

    /// Build an account-scoped API path.
    ///
    /// The Basecamp 3 API uses account-scoped URLs of the form:
    /// `/{account_id}{path}` - e.g., `/12345/projects.json`
    ///
    /// This prepends the account ID to the given path. The path should start with `/`.
    ///
    /// # Example
    /// ```
    /// // For account ID 12345 and path "/projects.json"
    /// // Returns: "/12345/projects.json"
    /// ```
    ///
    /// Note: For project-scoped resources (under `/buckets/{project_id}/...`),
    /// use `bucket_path()` instead.
    pub fn account_path(&self, path: &str) -> String {
        format!("/{}{}", self.account_id, path)
    }

    /// Build a project-scoped (bucket) API path.
    ///
    /// Many Basecamp 3 resources are scoped to a project (called a "bucket" in the API).
    /// These URLs have the form: `/{account_id}/buckets/{project_id}{path}`
    ///
    /// # Example
    /// ```
    /// // For account ID 12345, project ID 67890, and path "/todos.json"
    /// // Returns: "/12345/buckets/67890/todos.json"
    /// ```
    ///
    /// This is the Rust equivalent of Python SDK's `BaseService._bucket_path()`.
    pub fn bucket_path(&self, project_id: i64, path: &str) -> String {
        format!("/{}/buckets/{}{}", self.account_id, project_id, path)
    }

    pub fn http(&self) -> &HttpClient {
        &self.http
    }

    pub fn projects(&self) -> &crate::services::ProjectsService {
        self.projects
            .get_or_init(|| crate::services::ProjectsService::new(self.http.clone()))
    }
}

impl Clone for AccountClient {
    fn clone(&self) -> Self {
        Self {
            account_id: self.account_id,
            http: self.http.clone(),
            projects: OnceCell::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::StaticTokenProvider;
    use crate::config::Config;
    use crate::hooks::NoOpHooks;

    mod client_new {
        use super::*;

        #[test]
        fn test_new_creates_client_with_default_config() {
            let client = Client::new("test-token");
            assert!(!client.is_closed());
            assert_eq!(client.http().config().base_url, "https://3.basecampapi.com");
        }

        #[test]
        fn test_new_creates_authorization_service() {
            let client = Client::new("test-token");
            let _auth = client.authorization();
        }
    }

    mod client_builder {
        use super::*;

        #[test]
        fn test_builder_access_token_succeeds() {
            let client = Client::builder().access_token("test-token").build();
            assert!(client.is_ok());
        }

        #[test]
        fn test_builder_token_provider_succeeds() {
            let provider = StaticTokenProvider::new("test-token");
            let client = Client::builder().token_provider(provider).build();
            assert!(client.is_ok());
        }

        #[test]
        fn test_builder_auth_succeeds() {
            let auth = BearerAuth::from_token("test-token");
            let client = Client::builder().auth(auth).build();
            assert!(client.is_ok());
        }

        #[test]
        fn test_builder_no_auth_returns_error() {
            let result = Client::builder().build();
            assert!(matches!(result, Err(ClientError::NoAuthProvider { .. })));
        }

        #[test]
        fn test_builder_access_token_and_token_provider_returns_ambiguous() {
            let provider = StaticTokenProvider::new("test-token");
            let result = Client::builder()
                .access_token("another-token")
                .token_provider(provider)
                .build();
            assert!(matches!(result, Err(ClientError::AmbiguousAuth { .. })));
        }

        #[test]
        fn test_builder_access_token_and_auth_returns_ambiguous() {
            let auth = BearerAuth::from_token("test-token");
            let result = Client::builder()
                .access_token("another-token")
                .auth(auth)
                .build();
            assert!(matches!(result, Err(ClientError::AmbiguousAuth { .. })));
        }

        #[test]
        fn test_builder_token_provider_and_auth_returns_ambiguous() {
            let provider = StaticTokenProvider::new("test-token");
            let auth = BearerAuth::from_token("another-token");
            let result = Client::builder()
                .token_provider(provider)
                .auth(auth)
                .build();
            assert!(matches!(result, Err(ClientError::AmbiguousAuth { .. })));
        }

        #[test]
        fn test_builder_all_three_auth_sources_returns_ambiguous() {
            let provider = StaticTokenProvider::new("test-token");
            let auth = BearerAuth::from_token("yet-another-token");
            let result = Client::builder()
                .access_token("another-token")
                .token_provider(provider)
                .auth(auth)
                .build();
            assert!(matches!(result, Err(ClientError::AmbiguousAuth { .. })));
        }

        #[test]
        fn test_builder_with_custom_config() {
            let config = Config::builder()
                .base_url("https://custom.api.com")
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap();

            let client = Client::builder()
                .access_token("test-token")
                .config(config)
                .build()
                .unwrap();

            assert_eq!(client.http().config().base_url, "https://custom.api.com");
        }

        #[test]
        fn test_builder_with_hooks() {
            let client = Client::builder()
                .access_token("test-token")
                .hooks(NoOpHooks)
                .build();
            assert!(client.is_ok());
        }

        #[test]
        fn test_builder_with_custom_user_agent() {
            let client = Client::builder()
                .access_token("test-token")
                .user_agent("my-app/1.0")
                .build()
                .unwrap();

            assert_eq!(client.http().user_agent(), "my-app/1.0");
        }
    }

    mod client_methods {
        use super::*;

        #[test]
        fn test_for_account_returns_account_client() {
            let client = Client::new("test-token");
            let account = client.for_account(12345);
            assert_eq!(account.account_id(), 12345);
        }

        #[test]
        fn test_authorization_returns_service() {
            let client = Client::new("test-token");
            let _service = client.authorization();
        }

        #[test]
        fn test_is_closed_initially_false() {
            let client = Client::new("test-token");
            assert!(!client.is_closed());
        }

        #[test]
        fn test_close_sets_closed_flag() {
            let client = Client::new("test-token");
            let closed = client.closed.clone();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(client.close());
            assert!(closed.load(Ordering::SeqCst));
        }

        #[test]
        fn test_drop_sets_closed_flag() {
            let closed;
            {
                let client = Client::new("test-token");
                closed = client.closed.clone();
                assert!(!closed.load(Ordering::SeqCst));
            }
            assert!(closed.load(Ordering::SeqCst));
        }

        #[test]
        fn test_clone_shares_http_pool() {
            let client1 = Client::new("test-token");
            let client2 = client1.clone();

            assert!(Arc::ptr_eq(&client1.http, &client2.http));
        }

        #[test]
        fn test_clone_shares_auth_state() {
            let client1 = Client::new("test-token");
            let client2 = client1.clone();

            assert!(Arc::ptr_eq(&client1.auth, &client2.auth));
        }

        #[test]
        fn test_clone_independent_closed_flag() {
            let client1 = Client::new("test-token");
            let client2 = client1.clone();

            client1.closed.store(true, Ordering::SeqCst);
            assert!(client1.is_closed());
            assert!(client2.is_closed());
        }
    }

    mod account_client {
        use super::*;

        #[test]
        fn test_account_path_prepends_account_id() {
            let client = Client::new("test-token");
            let account = client.for_account(12345);

            assert_eq!(
                account.account_path("/projects.json"),
                "/12345/projects.json"
            );
            assert_eq!(account.account_path("/todos.json"), "/12345/todos.json");
        }

        #[test]
        fn test_bucket_path_for_project_scoped_resources() {
            let client = Client::new("test-token");
            let account = client.for_account(12345);

            assert_eq!(
                account.bucket_path(67890, "/todos.json"),
                "/12345/buckets/67890/todos.json"
            );
            assert_eq!(
                account.bucket_path(67890, "/messages/1.json"),
                "/12345/buckets/67890/messages/1.json"
            );
        }

        #[test]
        fn test_account_id_returns_correct_id() {
            let client = Client::new("test-token");
            let account = client.for_account(99999);

            assert_eq!(account.account_id(), 99999);
        }

        #[test]
        fn test_clone_shares_http_pool() {
            let client = Client::new("test-token");
            let account1 = client.for_account(12345);
            let account2 = account1.clone();

            assert!(Arc::ptr_eq(&account1.http, &account2.http));
        }

        #[test]
        fn test_projects_lazy_loading() {
            let client = Client::new("test-token");
            let account = client.for_account(12345);

            let projects1 = account.projects() as *const _;
            let projects2 = account.projects() as *const _;

            assert_eq!(projects1, projects2, "Same instance should be returned");
        }

        #[test]
        fn test_projects_creates_service_on_first_access() {
            let client = Client::new("test-token");
            let account = client.for_account(12345);

            let projects = account.projects();
            let _service = projects;
        }

        #[test]
        fn test_clone_creates_new_projects_cell() {
            let client = Client::new("test-token");
            let account1 = client.for_account(12345);

            let _projects1 = account1.projects();

            let account2 = account1.clone();

            let projects2 = account2.projects() as *const _;
            let projects1 = account1.projects() as *const _;

            assert_ne!(
                projects1, projects2,
                "Clone should have its own service instance"
            );
        }
    }

    mod thread_safety {
        use super::*;

        #[test]
        fn test_client_is_send_sync() {
            fn assert_send_sync<T: Send + Sync>() {}
            assert_send_sync::<Client>();
            assert_send_sync::<AccountClient>();
            assert_send_sync::<ClientBuilder>();
        }
    }

    mod client_error {
        use super::*;

        #[test]
        fn test_ambiguous_auth_display() {
            let err = ClientError::AmbiguousAuth {
                message: "test message".to_string(),
            };
            assert_eq!(format!("{}", err), "test message");
        }

        #[test]
        fn test_no_auth_provider_display() {
            let err = ClientError::NoAuthProvider {
                message: "no auth".to_string(),
            };
            assert_eq!(format!("{}", err), "no auth");
        }

        #[test]
        fn test_config_error_display() {
            let err = ClientError::Config {
                message: "config failed".to_string(),
            };
            assert_eq!(format!("{}", err), "config failed");
        }

        #[test]
        fn test_client_error_is_std_error() {
            let err = ClientError::NoAuthProvider {
                message: "test".to_string(),
            };
            let _: &dyn std::error::Error = &err;
        }
    }

    mod token_provider_wrapper {
        use super::*;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingTokenProvider {
            token: String,
            refresh_count: AtomicUsize,
        }

        impl TokenProvider for CountingTokenProvider {
            fn access_token(&self) -> String {
                self.token.clone()
            }

            fn refresh(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
                self.refresh_count.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { true })
            }

            fn refreshable(&self) -> bool {
                true
            }
        }

        #[test]
        fn test_wrapper_delegates_access_token() {
            let provider = CountingTokenProvider {
                token: "test-token".to_string(),
                refresh_count: AtomicUsize::new(0),
            };
            let wrapper = TokenProviderWrapper(Arc::new(provider));
            assert_eq!(wrapper.access_token(), "test-token");
        }

        #[test]
        fn test_wrapper_delegates_refreshable() {
            let provider = CountingTokenProvider {
                token: "test-token".to_string(),
                refresh_count: AtomicUsize::new(0),
            };
            let wrapper = TokenProviderWrapper(Arc::new(provider));
            assert!(wrapper.refreshable());
        }

        #[tokio::test]
        async fn test_wrapper_delegates_refresh() {
            let provider = CountingTokenProvider {
                token: "test-token".to_string(),
                refresh_count: AtomicUsize::new(0),
            };
            let wrapper = TokenProviderWrapper(Arc::new(provider));
            let result = wrapper.refresh().await;
            assert!(result);
        }
    }

    mod default_user_agent {
        use super::*;

        #[test]
        fn test_default_user_agent_format() {
            let client = Client::new("test-token");
            let ua = client.http().user_agent();
            assert!(ua.starts_with("basecamp-sdk-rust/"));
            assert!(ua.contains("(api:3)"));
        }
    }

    mod client_builder_default {
        use super::*;

        #[test]
        fn test_builder_default_trait() {
            let builder1 = ClientBuilder::default();
            let builder2 = ClientBuilder::new();
            let client1 = builder1.access_token("token1").build().unwrap();
            let client2 = builder2.access_token("token2").build().unwrap();
            assert_eq!(
                client1.http().config().base_url,
                client2.http().config().base_url
            );
        }
    }
}
