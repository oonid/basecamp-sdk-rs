use crate::auth::{AuthStrategy, TokenProvider};
use crate::config::Config;
use crate::error::{self, BasecampError, ErrorCode};
use crate::hooks::BasecampHooks;
use crate::http::retry::{should_retry_with_config, RetryDecision};
use crate::pagination::{parse_next_link, parse_total_count, resolve_url, ListMeta, ListResult};
use crate::security::{self, MAX_RESPONSE_BODY_BYTES};
use reqwest::{header, Client, Method, Response};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use url::Url;

const API_VERSION: &str = "3";
const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

struct RequestInfo {
    method: Method,
    url: String,
    body: Option<String>,
}

pub struct HttpClient {
    inner: Client,
    config: Config,
    auth: Arc<dyn AuthStrategy>,
    token_provider: Option<Arc<dyn TokenProvider>>,
    user_agent: String,
    hooks: Option<Arc<dyn BasecampHooks>>,
}

impl HttpClient {
    pub fn new(config: Config, auth: impl AuthStrategy + 'static) -> Result<Self, BasecampError> {
        let user_agent = format!("basecamp-sdk-rust/{} (api:{})", SDK_VERSION, API_VERSION);

        let inner = Client::builder()
            .timeout(config.timeout)
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| BasecampError::Network {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        let token_provider = auth.token_provider();
        let auth: Arc<dyn AuthStrategy> = Arc::new(auth);

        Ok(Self {
            inner,
            config,
            auth,
            token_provider,
            user_agent,
            hooks: None,
        })
    }

    pub fn with_hooks(mut self, hooks: Arc<dyn BasecampHooks>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    fn build_request(&self, info: &RequestInfo) -> reqwest::RequestBuilder {
        let mut headers = self.base_headers();
        self.auth.authenticate(&mut headers);

        let mut req_builder = self
            .inner
            .request(info.method.clone(), &info.url)
            .headers(headers);

        if let Some(ref body) = info.body {
            req_builder = req_builder
                .body(body.clone())
                .header(header::CONTENT_TYPE, "application/json");
        }

        req_builder
    }

    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    pub async fn get(
        &self,
        path: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<Response, BasecampError> {
        let url = self.build_url(path)?;
        self.request(Method::GET, &url, params, None, None).await
    }

    pub async fn get_absolute(
        &self,
        url: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<Response, BasecampError> {
        if !security::is_localhost(url) {
            security::require_https(url).map_err(|e| BasecampError::Usage {
                message: e,
                hint: None,
            })?;
        }
        self.request(Method::GET, url, params, None, None).await
    }

    pub async fn post(
        &self,
        path: &str,
        json_body: Option<&serde_json::Value>,
        operation: Option<&str>,
    ) -> Result<Response, BasecampError> {
        let url = self.build_url(path)?;
        let body = json_body.map(|v| v.to_string());
        self.request(Method::POST, &url, None, body.as_deref(), operation)
            .await
    }

    pub async fn put(
        &self,
        path: &str,
        json_body: Option<&serde_json::Value>,
        operation: Option<&str>,
    ) -> Result<Response, BasecampError> {
        let url = self.build_url(path)?;
        let body = json_body.map(|v| v.to_string());
        self.request(Method::PUT, &url, None, body.as_deref(), operation)
            .await
    }

    pub async fn delete(
        &self,
        path: &str,
        operation: Option<&str>,
    ) -> Result<Response, BasecampError> {
        let url = self.build_url(path)?;
        self.request(Method::DELETE, &url, None, None, operation)
            .await
    }

    pub async fn post_raw(
        &self,
        path: &str,
        content: &[u8],
        content_type: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<Response, BasecampError> {
        let url = self.build_url(path)?;
        self.request_raw(Method::POST, &url, content, content_type, params)
            .await
    }

    pub async fn request_multipart(
        &self,
        method: Method,
        path: &str,
        field: &str,
        content: &[u8],
        filename: &str,
        content_type: &str,
    ) -> Result<Response, BasecampError> {
        let url = self.build_url(path)?;

        if security::contains_crlf(filename) {
            return Err(BasecampError::Usage {
                message: "Filename contains CRLF sequence".to_string(),
                hint: None,
            });
        }
        if security::contains_crlf(content_type) {
            return Err(BasecampError::Usage {
                message: "Content type contains CRLF sequence".to_string(),
                hint: None,
            });
        }

        let form = reqwest::multipart::Form::new().part(
            field.to_string(),
            reqwest::multipart::Part::bytes(content.to_vec())
                .file_name(filename.to_string())
                .mime_str(content_type)
                .map_err(|e| BasecampError::Usage {
                    message: format!("Invalid content type: {}", e),
                    hint: None,
                })?,
        );

        let mut headers = self.base_headers();
        self.auth.authenticate(&mut headers);

        let req = self
            .inner
            .request(method, &url)
            .headers(headers)
            .multipart(form)
            .build()
            .map_err(|e| BasecampError::Network {
                message: format!("Failed to build request: {}", e),
            })?;

        self.execute_single(req).await
    }

    pub async fn get_no_retry(&self, path: &str) -> Result<Response, BasecampError> {
        let url = self.build_url(path)?;
        self.request_without_retry(Method::GET, &url, None, None)
            .await
    }

    pub async fn get_paginated<T: DeserializeOwned>(
        &self,
        path: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<ListResult<T>, BasecampError> {
        let base_url = self.build_url(path)?;
        let mut all_items = Vec::new();
        let mut current_url = Some(base_url.clone());
        let mut total_count: Option<u64> = Some(0);
        let mut page_count = 0u32;
        let max_pages = self.config.max_pages;
        let max_items = self.config.max_items;

        while let Some(url) = current_url.take() {
            page_count += 1;

            if let Some(ref hooks) = self.hooks {
                hooks.on_paginate(&url, page_count);
            }

            if page_count > max_pages {
                return Ok(ListResult {
                    items: all_items,
                    meta: ListMeta {
                        total_count,
                        truncated: true,
                        next_url: Some(url),
                    },
                });
            }

            let response = self.get_absolute(&url, params).await?;

            if let Some(content_length) = response.content_length() {
                if content_length as usize > MAX_RESPONSE_BODY_BYTES {
                    return Err(BasecampError::Usage {
                        message: format!(
                            "Response body too large ({} bytes, max {})",
                            content_length, MAX_RESPONSE_BODY_BYTES
                        ),
                        hint: Some("Response exceeds maximum allowed size".to_string()),
                    });
                }
            }

            if page_count == 1 {
                if let Some(count) = parse_total_count(response.headers()) {
                    total_count = Some(count);
                }
            }

            let link_header = response
                .headers()
                .get("Link")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let items: Vec<T> = response.json().await.map_err(|e| BasecampError::Network {
                message: format!("Failed to parse response: {}", e),
            })?;
            all_items.extend(items);

            if let Some(max) = max_items {
                if all_items.len() >= max as usize {
                    all_items.truncate(max as usize);
                    return Ok(ListResult {
                        items: all_items,
                        meta: ListMeta {
                            total_count,
                            truncated: true,
                            next_url: None,
                        },
                    });
                }
            }

            if let Some(next_url) = parse_next_link(link_header.as_deref()) {
                let resolved_next_url = resolve_url(&url, &next_url);

                if !security::same_origin(&base_url, &resolved_next_url) {
                    return Err(BasecampError::Usage {
                        message: format!(
                            "Pagination URL has different origin: {}",
                            resolved_next_url
                        ),
                        hint: Some("Possible SSRF attack via Link header".to_string()),
                    });
                }

                let is_localhost_url = Url::parse(&resolved_next_url)
                    .ok()
                    .and_then(|u| u.host_str().map(|h| h.to_string()))
                    .map(|h| security::is_localhost(&h))
                    .unwrap_or(false);

                if !resolved_next_url.starts_with("https://") && !is_localhost_url {
                    return Err(BasecampError::Usage {
                        message: format!(
                            "Pagination URL uses insecure protocol: {}",
                            resolved_next_url
                        ),
                        hint: Some("Protocol downgrade not allowed".to_string()),
                    });
                }

                current_url = Some(resolved_next_url);
            }
        }

        Ok(ListResult {
            items: all_items,
            meta: ListMeta {
                total_count,
                truncated: false,
                next_url: None,
            },
        })
    }

    fn build_url(&self, path: &str) -> Result<String, BasecampError> {
        if path.starts_with("https://") {
            return Ok(path.to_string());
        }
        if path.starts_with("http://") {
            if let Ok(parsed) = Url::parse(path) {
                if security::is_localhost(parsed.host_str().unwrap_or("")) {
                    return Ok(path.to_string());
                }
            }
            return Err(BasecampError::Usage {
                message: format!("URL must use HTTPS: {}", path),
                hint: None,
            });
        }

        let normalized_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };

        Ok(format!("{}{}", self.config.base_url, normalized_path))
    }

    fn base_headers(&self) -> header::HeaderMap {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_str(&self.user_agent).unwrap(),
        );
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );
        headers
    }

    fn add_query_params(url: &str, params: &[(&str, &str)]) -> String {
        let mut parsed = Url::parse(url).unwrap();
        {
            let mut query_pairs = parsed.query_pairs_mut();
            for (key, value) in params {
                query_pairs.append_pair(key, value);
            }
        }
        parsed.to_string()
    }

    async fn request(
        &self,
        method: Method,
        url: &str,
        params: Option<&[(&str, &str)]>,
        body: Option<&str>,
        operation: Option<&str>,
    ) -> Result<Response, BasecampError> {
        let url = match params {
            Some(p) if !p.is_empty() => Self::add_query_params(url, p),
            _ => url.to_string(),
        };

        let request_info = RequestInfo {
            method: method.clone(),
            url,
            body: body.map(|s| s.to_string()),
        };

        let is_idempotent = operation == Some("idempotent");
        self.execute_request(&request_info, is_idempotent).await
    }

    async fn request_without_retry(
        &self,
        method: Method,
        url: &str,
        params: Option<&[(&str, &str)]>,
        body: Option<&str>,
    ) -> Result<Response, BasecampError> {
        let url = match params {
            Some(p) if !p.is_empty() => Self::add_query_params(url, p),
            _ => url.to_string(),
        };

        let mut headers = self.base_headers();
        self.auth.authenticate(&mut headers);

        let mut req_builder = self.inner.request(method, &url).headers(headers);

        if let Some(b) = body {
            req_builder = req_builder
                .body(b.to_string())
                .header(header::CONTENT_TYPE, "application/json");
        }

        let req = req_builder.build().map_err(|e| BasecampError::Network {
            message: format!("Failed to build request: {}", e),
        })?;

        self.execute_without_retry(req).await
    }

    async fn request_raw(
        &self,
        method: Method,
        url: &str,
        content: &[u8],
        content_type: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<Response, BasecampError> {
        let url = match params {
            Some(p) if !p.is_empty() => Self::add_query_params(url, p),
            _ => url.to_string(),
        };

        let mut headers = self.base_headers();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_str(content_type).map_err(|e| BasecampError::Usage {
                message: format!("Invalid content type: {}", e),
                hint: None,
            })?,
        );
        self.auth.authenticate(&mut headers);

        let req = self
            .inner
            .request(method, &url)
            .headers(headers)
            .body(content.to_vec())
            .build()
            .map_err(|e| BasecampError::Network {
                message: format!("Failed to build request: {}", e),
            })?;

        self.execute_single(req).await
    }

    async fn execute_request(
        &self,
        request_info: &RequestInfo,
        is_idempotent: bool,
    ) -> Result<Response, BasecampError> {
        let is_post = request_info.method == Method::POST;
        let mut attempt = 1u32;
        let mut refresh_attempted = false;

        loop {
            let req_builder = self.build_request(request_info);
            let built_req = req_builder.build().map_err(|e| BasecampError::Network {
                message: format!("Failed to build request: {}", e),
            })?;

            match self.execute_single(built_req).await {
                Ok(response) => return Ok(response),
                Err(error) => {
                    if error.code() == ErrorCode::AuthRequired
                        && !refresh_attempted
                        && self.can_refresh_token()
                        && self.attempt_token_refresh().await
                    {
                        refresh_attempted = true;
                        continue;
                    }

                    let retry_after = error.retry_after().map(Duration::from_secs);

                    let decision = should_retry_with_config(
                        &error,
                        attempt,
                        &self.config,
                        is_idempotent,
                        is_post,
                        retry_after,
                    );

                    match decision {
                        RetryDecision::Retry { delay } => {
                            sleep(delay).await;
                            attempt += 1;
                        }
                        RetryDecision::DontRetry => return Err(error),
                    }
                }
            }
        }
    }

    fn can_refresh_token(&self) -> bool {
        match &self.token_provider {
            Some(provider) => provider.refreshable(),
            None => false,
        }
    }

    async fn attempt_token_refresh(&self) -> bool {
        match &self.token_provider {
            Some(provider) if provider.refreshable() => provider.refresh().await,
            _ => false,
        }
    }

    async fn execute_single(&self, req: reqwest::Request) -> Result<Response, BasecampError> {
        let response = self.inner.execute(req).await.map_err(|e| {
            if e.is_timeout() {
                BasecampError::Network {
                    message: format!("Request timed out: {}", e),
                }
            } else if e.is_connect() {
                BasecampError::Network {
                    message: format!("Connection failed: {}", e),
                }
            } else {
                BasecampError::Network {
                    message: format!("Request failed: {}", e),
                }
            }
        })?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            Err(self.handle_error_response(response).await)
        }
    }

    async fn execute_without_retry(
        &self,
        req: reqwest::Request,
    ) -> Result<Response, BasecampError> {
        let response = self.inner.execute(req).await.map_err(|e| {
            if e.is_timeout() {
                BasecampError::Network {
                    message: format!("Request timed out: {}", e),
                }
            } else if e.is_connect() {
                BasecampError::Network {
                    message: format!("Connection failed: {}", e),
                }
            } else {
                BasecampError::Network {
                    message: format!("Request failed: {}", e),
                }
            }
        })?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            Err(self.handle_error_response(response).await)
        }
    }

    async fn handle_error_response(&self, response: Response) -> BasecampError {
        let status = response.status().as_u16();
        let headers = response.headers().clone();

        let body_bytes = response.bytes().await.unwrap_or_default();
        let body = if body_bytes.len() > security::MAX_ERROR_BODY_BYTES {
            &body_bytes[..security::MAX_ERROR_BODY_BYTES]
        } else {
            &body_bytes[..]
        };

        let body_str = String::from_utf8_lossy(body);
        error::error_from_response(status, &body_str, &headers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::BearerAuth;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    fn create_test_client() -> HttpClient {
        let config = Config::new();
        let auth = BearerAuth::from_token("test-token");
        HttpClient::new(config, auth).unwrap()
    }

    async fn create_test_client_with_server() -> (HttpClient, MockServer) {
        let client = create_test_client();
        let mock_server = MockServer::start().await;
        (client, mock_server)
    }

    mod get {
        use super::*;

        #[tokio::test]
        async fn test_get_success() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .and(matchers::path("/test.json"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": "ok"})),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/test.json", mock_server.uri());
            let response = client.get(&url, None).await.unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_get_with_params() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .and(matchers::path("/search.json"))
                .and(matchers::query_param("q", "test"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
                .mount(&mock_server)
                .await;

            let url = format!("{}/search.json", mock_server.uri());
            let response = client.get(&url, Some(&[("q", "test")])).await.unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_get_adds_auth_header() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .and(matchers::header("Authorization", "Bearer test-token"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&mock_server)
                .await;

            let url = format!("{}/test.json", mock_server.uri());
            let response = client.get(&url, None).await.unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_get_adds_user_agent() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .and(matchers::header(
                    "User-Agent",
                    "basecamp-sdk-rust/0.1.0 (api:3)",
                ))
                .respond_with(ResponseTemplate::new(200))
                .mount(&mock_server)
                .await;

            let url = format!("{}/test.json", mock_server.uri());
            let response = client.get(&url, None).await.unwrap();

            assert_eq!(response.status(), 200);
        }
    }

    mod post_put_delete {
        use super::*;

        #[tokio::test]
        async fn test_post_success() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("POST"))
                .and(matchers::path("/create.json"))
                .and(matchers::body_json(serde_json::json!({"name": "test"})))
                .respond_with(
                    ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 1})),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/create.json", mock_server.uri());
            let response = client
                .post(&url, Some(&serde_json::json!({"name": "test"})), None)
                .await
                .unwrap();

            assert_eq!(response.status(), 201);
        }

        #[tokio::test]
        async fn test_put_success() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("PUT"))
                .and(matchers::path("/update.json"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&mock_server)
                .await;

            let url = format!("{}/update.json", mock_server.uri());
            let response = client
                .put(&url, Some(&serde_json::json!({"name": "updated"})), None)
                .await
                .unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_delete_success() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("DELETE"))
                .and(matchers::path("/delete.json"))
                .respond_with(ResponseTemplate::new(204))
                .mount(&mock_server)
                .await;

            let url = format!("{}/delete.json", mock_server.uri());
            let response = client.delete(&url, None).await.unwrap();

            assert_eq!(response.status(), 204);
        }
    }

    mod get_absolute {
        use super::*;

        #[tokio::test]
        async fn test_get_absolute_uses_full_url() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .and(matchers::path("/absolute/path.json"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"absolute": true})),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/absolute/path.json", mock_server.uri());
            let response = client.get_absolute(&url, None).await.unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_get_absolute_with_params() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .and(matchers::query_param("page", "2"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&mock_server)
                .await;

            let url = format!("{}/test.json", mock_server.uri());
            let response = client
                .get_absolute(&url, Some(&[("page", "2")]))
                .await
                .unwrap();

            assert_eq!(response.status(), 200);
        }
    }

    mod timeout {
        use super::*;
        use std::time::Duration;

        #[tokio::test]
        async fn test_timeout_enforced() {
            let config = Config::builder()
                .timeout(Duration::from_millis(100))
                .build()
                .unwrap();
            let auth = BearerAuth::from_token("test-token");
            let client = HttpClient::new(config, auth).unwrap();
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(500)))
                .mount(&mock_server)
                .await;

            let url = format!("{}/slow.json", mock_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), crate::error::ErrorCode::Network);
            assert!(err.to_string().contains("timed out"));
        }
    }

    mod error_handling {
        use super::*;

        #[tokio::test]
        async fn test_404_returns_not_found_error() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(404))
                .mount(&mock_server)
                .await;

            let url = format!("{}/missing.json", mock_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), crate::error::ErrorCode::NotFound);
        }

        #[tokio::test]
        async fn test_401_returns_auth_error() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(401))
                .mount(&mock_server)
                .await;

            let url = format!("{}/unauthorized.json", mock_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), crate::error::ErrorCode::AuthRequired);
        }

        #[tokio::test]
        async fn test_500_returns_api_error_retryable() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(500))
                .mount(&mock_server)
                .await;

            let url = format!("{}/error.json", mock_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), crate::error::ErrorCode::Api);
            assert!(err.retryable());
        }

        #[tokio::test]
        async fn test_429_returns_rate_limit_error() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "60"))
                .mount(&mock_server)
                .await;

            let url = format!("{}/rate-limited.json", mock_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), crate::error::ErrorCode::RateLimit);
            assert_eq!(err.retry_after(), Some(60));
        }

        #[tokio::test]
        async fn test_403_returns_forbidden_error() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(403))
                .mount(&mock_server)
                .await;

            let url = format!("{}/forbidden.json", mock_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), crate::error::ErrorCode::Forbidden);
        }

        #[tokio::test]
        async fn test_422_returns_validation_error() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("POST"))
                .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
                    "errors": {"name": ["cannot be empty"]}
                })))
                .mount(&mock_server)
                .await;

            let url = format!("{}/validate.json", mock_server.uri());
            let result = client.post(&url, Some(&serde_json::json!({})), None).await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), crate::error::ErrorCode::Validation);
        }
    }

    mod url_building {
        use super::*;

        #[test]
        fn test_build_url_relative_path() {
            let client = create_test_client();

            let url = client.build_url("projects.json").unwrap();
            assert_eq!(url, "https://3.basecampapi.com/projects.json");
        }

        #[test]
        fn test_build_url_with_leading_slash() {
            let client = create_test_client();

            let url = client.build_url("/projects.json").unwrap();
            assert_eq!(url, "https://3.basecampapi.com/projects.json");
        }

        #[test]
        fn test_build_url_absolute_https() {
            let client = create_test_client();

            let url = client.build_url("https://custom.api.com/path").unwrap();
            assert_eq!(url, "https://custom.api.com/path");
        }

        #[test]
        fn test_build_url_absolute_http_localhost() {
            let client = create_test_client();

            let url = client.build_url("http://localhost/test.json").unwrap();
            assert_eq!(url, "http://localhost/test.json");
        }

        #[test]
        fn test_build_url_absolute_http_127_0_0_1() {
            let client = create_test_client();

            let url = client.build_url("http://127.0.0.1:8080/test.json").unwrap();
            assert_eq!(url, "http://127.0.0.1:8080/test.json");
        }

        #[test]
        fn test_build_url_rejects_http_non_localhost() {
            let client = create_test_client();

            let result = client.build_url("http://evil.com/test.json");
            assert!(result.is_err());
        }
    }

    mod get_no_retry {
        use super::*;

        #[tokio::test]
        async fn test_get_no_retry_success() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .and(matchers::path("/test.json"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": "ok"})),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/test.json", mock_server.uri());
            let response = client.get_no_retry(&url).await.unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_get_no_retry_returns_error() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(500))
                .mount(&mock_server)
                .await;

            let url = format!("{}/error.json", mock_server.uri());
            let result = client.get_no_retry(&url).await;

            assert!(result.is_err());
        }
    }

    mod multipart {
        use super::*;

        #[tokio::test]
        async fn test_multipart_upload() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("POST"))
                .and(matchers::path("/upload.json"))
                .respond_with(ResponseTemplate::new(201))
                .mount(&mock_server)
                .await;

            let url = format!("{}/upload.json", mock_server.uri());
            let content = b"test file content";
            let response = client
                .request_multipart(
                    Method::POST,
                    &url,
                    "attach",
                    content,
                    "test.txt",
                    "text/plain",
                )
                .await
                .unwrap();

            assert_eq!(response.status(), 201);
        }

        #[tokio::test]
        async fn test_multipart_rejects_crlf_in_filename() {
            let client = create_test_client();

            let result = client
                .request_multipart(
                    Method::POST,
                    "/upload.json",
                    "attach",
                    b"content",
                    "test\r\n.txt",
                    "text/plain",
                )
                .await;

            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_multipart_rejects_crlf_in_content_type() {
            let client = create_test_client();

            let result = client
                .request_multipart(
                    Method::POST,
                    "/upload.json",
                    "attach",
                    b"content",
                    "test.txt",
                    "text/plain\r\nX-Injected: evil",
                )
                .await;

            assert!(result.is_err());
        }
    }

    mod post_raw {
        use super::*;

        #[tokio::test]
        async fn test_post_raw_success() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("POST"))
                .and(matchers::path("/raw.json"))
                .and(matchers::header("Content-Type", "application/octet-stream"))
                .respond_with(ResponseTemplate::new(201))
                .mount(&mock_server)
                .await;

            let url = format!("{}/raw.json", mock_server.uri());
            let content = b"raw binary data";
            let response = client
                .post_raw(&url, content, "application/octet-stream", None)
                .await
                .unwrap();

            assert_eq!(response.status(), 201);
        }
    }

    mod retry_integration {
        use super::*;

        fn create_test_client_with_retries(max_retries: u32) -> HttpClient {
            let config = Config::builder()
                .max_retries(max_retries)
                .base_delay(Duration::from_millis(10))
                .max_jitter(Duration::from_millis(0))
                .build()
                .unwrap();
            let auth = BearerAuth::from_token("test-token");
            HttpClient::new(config, auth).unwrap()
        }

        #[tokio::test]
        async fn test_get_retries_on_503() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(503))
                .up_to_n_times(2)
                .mount(&mock_server)
                .await;

            Mock::given(matchers::method("GET"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/retry.json", mock_server.uri());
            let response = client.get(&url, None).await.unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_get_retries_on_502() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(502))
                .up_to_n_times(1)
                .mount(&mock_server)
                .await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&mock_server)
                .await;

            let url = format!("{}/retry.json", mock_server.uri());
            let response = client.get(&url, None).await.unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_get_retries_on_500() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(500))
                .up_to_n_times(1)
                .mount(&mock_server)
                .await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&mock_server)
                .await;

            let url = format!("{}/retry.json", mock_server.uri());
            let response = client.get(&url, None).await.unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_429_respects_retry_after() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
                .up_to_n_times(1)
                .mount(&mock_server)
                .await;

            Mock::given(matchers::method("GET"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/rate-limited.json", mock_server.uri());
            let start = std::time::Instant::now();
            let response = client.get(&url, None).await.unwrap();
            let elapsed = start.elapsed();

            assert_eq!(response.status(), 200);
            assert!(
                elapsed >= Duration::from_secs(1),
                "Should have waited for Retry-After"
            );
        }

        #[tokio::test]
        async fn test_post_does_not_retry_by_default() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .respond_with(ResponseTemplate::new(503))
                .expect(1)
                .mount(&mock_server)
                .await;

            let url = format!("{}/no-retry.json", mock_server.uri());
            let result = client
                .post(&url, Some(&serde_json::json!({"test": true})), None)
                .await;

            assert!(result.is_err());
            mock_server.verify().await;
        }

        #[tokio::test]
        async fn test_post_retries_when_idempotent() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .respond_with(ResponseTemplate::new(503))
                .up_to_n_times(1)
                .mount(&mock_server)
                .await;

            Mock::given(matchers::method("POST"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/idempotent.json", mock_server.uri());
            let response = client
                .post(
                    &url,
                    Some(&serde_json::json!({"test": true})),
                    Some("idempotent"),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_404_not_retryable() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(404))
                .expect(1)
                .mount(&mock_server)
                .await;

            let url = format!("{}/not-found.json", mock_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().code(),
                crate::error::ErrorCode::NotFound
            );
            mock_server.verify().await;
        }

        #[tokio::test]
        async fn test_403_not_retryable() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(403))
                .expect(1)
                .mount(&mock_server)
                .await;

            let url = format!("{}/forbidden.json", mock_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().code(),
                crate::error::ErrorCode::Forbidden
            );
            mock_server.verify().await;
        }

        #[tokio::test]
        async fn test_401_not_retryable_directly() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(401))
                .expect(1)
                .mount(&mock_server)
                .await;

            let url = format!("{}/unauthorized.json", mock_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().code(),
                crate::error::ErrorCode::AuthRequired
            );
            mock_server.verify().await;
        }

        #[tokio::test]
        async fn test_validation_error_not_retryable() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .respond_with(
                    ResponseTemplate::new(422).set_body_json(serde_json::json!({"errors": {}})),
                )
                .expect(1)
                .mount(&mock_server)
                .await;

            let url = format!("{}/validation.json", mock_server.uri());
            let result = client.post(&url, Some(&serde_json::json!({})), None).await;

            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().code(),
                crate::error::ErrorCode::Validation
            );
            mock_server.verify().await;
        }

        #[tokio::test]
        async fn test_exhausts_max_retries() {
            let client = create_test_client_with_retries(2);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(503))
                .expect(2)
                .mount(&mock_server)
                .await;

            let url = format!("{}/exhaust.json", mock_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            mock_server.verify().await;
        }

        #[tokio::test]
        async fn test_put_retries_on_503() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("PUT"))
                .respond_with(ResponseTemplate::new(503))
                .up_to_n_times(1)
                .mount(&mock_server)
                .await;

            Mock::given(matchers::method("PUT"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&mock_server)
                .await;

            let url = format!("{}/put-retry.json", mock_server.uri());
            let response = client
                .put(&url, Some(&serde_json::json!({"test": true})), None)
                .await
                .unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_delete_retries_on_503() {
            let client = create_test_client_with_retries(3);
            let mock_server = MockServer::start().await;

            Mock::given(matchers::method("DELETE"))
                .respond_with(ResponseTemplate::new(503))
                .up_to_n_times(1)
                .mount(&mock_server)
                .await;

            Mock::given(matchers::method("DELETE"))
                .respond_with(ResponseTemplate::new(204))
                .mount(&mock_server)
                .await;

            let url = format!("{}/delete-retry.json", mock_server.uri());
            let response = client.delete(&url, None).await.unwrap();

            assert_eq!(response.status(), 204);
        }
    }

    mod token_refresh_integration {
        use super::*;
        use crate::auth::{BearerAuth, OAuthTokenProvider};

        #[tokio::test]
        async fn test_401_triggers_refresh_and_retry() {
            let api_server = MockServer::start().await;
            let token_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .and(matchers::path("/token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "new-access-token",
                    "refresh_token": "new-refresh-token",
                    "expires_in": 3600
                })))
                .mount(&token_server)
                .await;

            Mock::given(matchers::method("GET"))
                .and(matchers::header("Authorization", "Bearer old-access-token"))
                .respond_with(ResponseTemplate::new(401))
                .up_to_n_times(1)
                .mount(&api_server)
                .await;

            Mock::given(matchers::method("GET"))
                .and(matchers::header("Authorization", "Bearer new-access-token"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})),
                )
                .mount(&api_server)
                .await;

            let oauth_provider =
                OAuthTokenProvider::new("old-access-token", "client-id", "client-secret")
                    .with_refresh_token("old-refresh-token")
                    .with_token_url(format!("{}/token", token_server.uri()));

            let auth = BearerAuth::new(oauth_provider);
            let config = Config::builder().max_retries(0).build().unwrap();
            let client = HttpClient::new(config, auth).unwrap();

            let url = format!("{}/protected.json", api_server.uri());
            let response = client.get(&url, None).await.unwrap();

            assert_eq!(response.status(), 200);
        }

        #[tokio::test]
        async fn test_single_refresh_attempt_no_loop() {
            let api_server = MockServer::start().await;
            let token_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .and(matchers::path("/token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "refreshed-token",
                    "refresh_token": "refreshed-refresh"
                })))
                .expect(1)
                .mount(&token_server)
                .await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(401))
                .expect(2)
                .mount(&api_server)
                .await;

            let oauth_provider =
                OAuthTokenProvider::new("initial-token", "client-id", "client-secret")
                    .with_refresh_token("initial-refresh")
                    .with_token_url(format!("{}/token", token_server.uri()));

            let auth = BearerAuth::new(oauth_provider);
            let config = Config::builder().max_retries(0).build().unwrap();
            let client = HttpClient::new(config, auth).unwrap();

            let url = format!("{}/always-401.json", api_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            assert_eq!(result.unwrap_err().code(), ErrorCode::AuthRequired);
            token_server.verify().await;
        }

        #[tokio::test]
        async fn test_failed_refresh_returns_original_error() {
            let api_server = MockServer::start().await;
            let token_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .and(matchers::path("/token"))
                .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                    "error": "invalid_grant"
                })))
                .mount(&token_server)
                .await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(401))
                .expect(1)
                .mount(&api_server)
                .await;

            let oauth_provider =
                OAuthTokenProvider::new("expired-token", "client-id", "client-secret")
                    .with_refresh_token("expired-refresh")
                    .with_token_url(format!("{}/token", token_server.uri()));

            let auth = BearerAuth::new(oauth_provider);
            let config = Config::builder().max_retries(0).build().unwrap();
            let client = HttpClient::new(config, auth).unwrap();

            let url = format!("{}/protected.json", api_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            assert_eq!(result.unwrap_err().code(), ErrorCode::AuthRequired);
            api_server.verify().await;
        }

        #[tokio::test]
        async fn test_no_refresh_without_refresh_token() {
            let api_server = MockServer::start().await;
            let token_server = MockServer::start().await;

            Mock::given(matchers::method("POST"))
                .and(matchers::path("/token"))
                .respond_with(ResponseTemplate::new(200))
                .expect(0)
                .mount(&token_server)
                .await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(401))
                .expect(1)
                .mount(&api_server)
                .await;

            let oauth_provider =
                OAuthTokenProvider::new("access-token", "client-id", "client-secret")
                    .with_token_url(format!("{}/token", token_server.uri()));

            let auth = BearerAuth::new(oauth_provider);
            let config = Config::builder().max_retries(0).build().unwrap();
            let client = HttpClient::new(config, auth).unwrap();

            let url = format!("{}/protected.json", api_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            assert_eq!(result.unwrap_err().code(), ErrorCode::AuthRequired);
            token_server.verify().await;
        }

        #[tokio::test]
        async fn test_static_token_no_refresh_attempt() {
            let api_server = MockServer::start().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(401))
                .expect(1)
                .mount(&api_server)
                .await;

            let auth = BearerAuth::from_token("static-token");
            let config = Config::builder().max_retries(0).build().unwrap();
            let client = HttpClient::new(config, auth).unwrap();

            let url = format!("{}/protected.json", api_server.uri());
            let result = client.get(&url, None).await;

            assert!(result.is_err());
            assert_eq!(result.unwrap_err().code(), ErrorCode::AuthRequired);
            api_server.verify().await;
        }
    }

    mod pagination {
        use super::*;

        #[tokio::test]
        async fn test_single_page_no_next_link() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .and(matchers::path("/items.json"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!([{"id": 1}, {"id": 2}]))
                        .insert_header("X-Total-Count", "2"),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/items.json", mock_server.uri());
            let result: ListResult<serde_json::Value> =
                client.get_paginated(&url, None).await.unwrap();

            assert_eq!(result.len(), 2);
            assert_eq!(result.meta.total_count, Some(2));
            assert!(!result.meta.truncated);
            assert!(result.meta.next_url.is_none());
        }

        #[tokio::test]
        async fn test_multi_page_fetch() {
            let (client, mock_server) = create_test_client_with_server().await;
            let base_uri = mock_server.uri();

            Mock::given(matchers::method("GET"))
                .and(matchers::path("/items.json"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!([{"id": 1}]))
                        .insert_header(
                            "Link",
                            format!(r#"<{}/items.json?page=2>; rel="next""#, base_uri),
                        )
                        .insert_header("X-Total-Count", "3"),
                )
                .up_to_n_times(1)
                .mount(&mock_server)
                .await;

            Mock::given(matchers::method("GET"))
                .and(matchers::path("/items.json"))
                .and(matchers::query_param("page", "2"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!([{"id": 2}, {"id": 3}])),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/items.json", mock_server.uri());
            let result: ListResult<serde_json::Value> =
                client.get_paginated(&url, None).await.unwrap();

            assert_eq!(result.len(), 3);
            assert_eq!(result.meta.total_count, Some(3));
            assert!(!result.meta.truncated);
        }

        #[tokio::test]
        async fn test_max_pages_limit_sets_truncated() {
            let config = Config::builder().max_pages(2).build().unwrap();
            let auth = BearerAuth::from_token("test-token");
            let client = HttpClient::new(config, auth).unwrap();
            let mock_server = MockServer::start().await;
            let base_uri = mock_server.uri();

            Mock::given(matchers::method("GET"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!([{"id": 1}]))
                        .insert_header(
                            "Link",
                            format!(r#"<{}/items.json?page=999>; rel="next""#, base_uri),
                        ),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/items.json", mock_server.uri());
            let result: ListResult<serde_json::Value> =
                client.get_paginated(&url, None).await.unwrap();

            assert_eq!(result.len(), 2);
            assert!(result.meta.truncated);
            assert!(result.meta.next_url.is_some());
        }

        #[tokio::test]
        async fn test_max_items_cap() {
            let config = Config::builder().max_items(3).build().unwrap();
            let auth = BearerAuth::from_token("test-token");
            let client = HttpClient::new(config, auth).unwrap();
            let mock_server = MockServer::start().await;
            let base_uri = mock_server.uri();

            Mock::given(matchers::method("GET"))
                .and(matchers::path("/items.json"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!([{"id": 1}, {"id": 2}]))
                        .insert_header(
                            "Link",
                            format!(r#"<{}/items.json?page=2>; rel="next""#, base_uri),
                        ),
                )
                .up_to_n_times(1)
                .mount(&mock_server)
                .await;

            Mock::given(matchers::method("GET"))
                .and(matchers::query_param("page", "2"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!([{"id": 3}, {"id": 4}, {"id": 5}])),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/items.json", mock_server.uri());
            let result: ListResult<serde_json::Value> =
                client.get_paginated(&url, None).await.unwrap();

            assert_eq!(result.len(), 3);
            assert!(result.meta.truncated);
            assert!(result.meta.next_url.is_none());
        }

        #[tokio::test]
        async fn test_same_origin_validation_rejects_different_origin() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!([{"id": 1}]))
                        .insert_header("Link", r#"<https://evil.com/items>; rel="next""#),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/items.json", mock_server.uri());
            let result: Result<ListResult<serde_json::Value>, _> =
                client.get_paginated(&url, None).await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), ErrorCode::Usage);
            assert!(err.to_string().contains("different origin"));
        }

        #[tokio::test]
        async fn test_protocol_downgrade_rejected() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!([{"id": 1}]))
                        .insert_header("Link", r#"<http://evil.com/items>; rel="next""#),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/items.json", mock_server.uri());
            let result: Result<ListResult<serde_json::Value>, _> =
                client.get_paginated(&url, None).await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), ErrorCode::Usage);
            assert!(
                err.to_string().contains("different origin")
                    || err.to_string().contains("insecure protocol")
            );
        }

        #[tokio::test]
        async fn test_http_allowed_for_localhost() {
            let (client, mock_server) = create_test_client_with_server().await;
            let base_uri = mock_server.uri();
            let http_uri = base_uri.replace("https://", "http://");

            Mock::given(matchers::method("GET"))
                .and(matchers::path("/items.json"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!([{"id": 1}]))
                        .insert_header(
                            "Link",
                            format!(r#"<{}/items?page=2>; rel="next""#, http_uri),
                        )
                        .insert_header("X-Total-Count", "2"),
                )
                .up_to_n_times(1)
                .mount(&mock_server)
                .await;

            Mock::given(matchers::method("GET"))
                .and(matchers::query_param("page", "2"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!([{"id": 2}])),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/items.json", mock_server.uri());
            let result: ListResult<serde_json::Value> =
                client.get_paginated(&url, None).await.unwrap();

            assert_eq!(result.len(), 2);
        }

        #[tokio::test]
        async fn test_empty_result() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
                .mount(&mock_server)
                .await;

            let url = format!("{}/empty.json", mock_server.uri());
            let result: ListResult<serde_json::Value> =
                client.get_paginated(&url, None).await.unwrap();

            assert!(result.is_empty());
            assert!(!result.meta.truncated);
        }

        #[tokio::test]
        async fn test_with_query_params() {
            let (client, mock_server) = create_test_client_with_server().await;

            Mock::given(matchers::method("GET"))
                .and(matchers::path("/search.json"))
                .and(matchers::query_param("q", "test"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!([{"id": 1}])),
                )
                .mount(&mock_server)
                .await;

            let url = format!("{}/search.json", mock_server.uri());
            let result: ListResult<serde_json::Value> = client
                .get_paginated(&url, Some(&[("q", "test")]))
                .await
                .unwrap();

            assert_eq!(result.len(), 1);
        }
    }
}
