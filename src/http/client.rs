use crate::auth::AuthStrategy;
use crate::config::Config;
use crate::error::{self, BasecampError};
use crate::security;
use reqwest::{header, Client, Method, Response};
use std::sync::Arc;
use std::time::Duration;
use url::Url;

const API_VERSION: &str = "3";
const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct HttpClient {
    inner: Client,
    config: Config,
    auth: Arc<dyn AuthStrategy>,
    user_agent: String,
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

        Ok(Self {
            inner,
            config,
            auth: Arc::from(Box::new(auth) as Box<dyn AuthStrategy>),
            user_agent,
        })
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
            .multipart(form);

        self.execute_request(req).await
    }

    pub async fn get_no_retry(&self, path: &str) -> Result<Response, BasecampError> {
        let url = self.build_url(path)?;
        self.request_without_retry(Method::GET, &url, None, None)
            .await
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
        _operation: Option<&str>,
    ) -> Result<Response, BasecampError> {
        let url = match params {
            Some(p) if !p.is_empty() => Self::add_query_params(url, p),
            _ => url.to_string(),
        };

        let mut headers = self.base_headers();
        self.auth.authenticate(&mut headers);

        let mut req_builder = self.inner.request(method.clone(), &url).headers(headers);

        if let Some(b) = body {
            req_builder = req_builder
                .body(b.to_string())
                .header(header::CONTENT_TYPE, "application/json");
        }

        self.execute_request(req_builder).await
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

        let req_builder = self
            .inner
            .request(method, &url)
            .headers(headers)
            .body(content.to_vec());

        self.execute_request(req_builder).await
    }

    async fn execute_request(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<Response, BasecampError> {
        let response = req.send().await.map_err(|e| {
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
}
