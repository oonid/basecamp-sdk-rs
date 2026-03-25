use crate::error::BasecampError;
use crate::http::HttpClient;

pub struct AuthorizationService {
    http: std::sync::Arc<HttpClient>,
}

impl AuthorizationService {
    pub fn new(http: std::sync::Arc<HttpClient>) -> Self {
        Self { http }
    }

    pub async fn get(&self) -> Result<Authorization, BasecampError> {
        let response = self.http.get("/authorization.json", None).await?;
        let body = response.text().await.map_err(|e| BasecampError::Network {
            message: format!("Failed to read response: {}", e),
        })?;
        serde_json::from_str(&body).map_err(|e| BasecampError::Usage {
            message: format!("Failed to parse authorization response: {}", e),
            hint: None,
        })
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Authorization {
    pub id: i64,
    pub name: String,
    pub email_address: String,
    pub identity_id: i64,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::BearerAuth;
    use crate::config::Config;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_get_authorization() {
        let mock_server = MockServer::start().await;

        Mock::given(matchers::method("GET"))
            .and(matchers::path("/authorization.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 12345,
                "name": "Test User",
                "email_address": "test@example.com",
                "identity_id": 67890
            })))
            .mount(&mock_server)
            .await;

        let config = Config::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap();
        let auth = BearerAuth::from_token("test-token");
        let http = std::sync::Arc::new(HttpClient::new(config, auth).unwrap());
        let service = AuthorizationService::new(http);

        let result = service.get().await.unwrap();
        assert_eq!(result.id, 12345);
        assert_eq!(result.name, "Test User");
        assert_eq!(result.email_address, "test@example.com");
    }

    #[tokio::test]
    async fn test_get_authorization_with_expires_at() {
        let mock_server = MockServer::start().await;

        Mock::given(matchers::method("GET"))
            .and(matchers::path("/authorization.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 12345,
                "name": "Test User",
                "email_address": "test@example.com",
                "identity_id": 67890,
                "expires_at": "2024-12-31T23:59:59Z"
            })))
            .mount(&mock_server)
            .await;

        let config = Config::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap();
        let auth = BearerAuth::from_token("test-token");
        let http = std::sync::Arc::new(HttpClient::new(config, auth).unwrap());
        let service = AuthorizationService::new(http);

        let result = service.get().await.unwrap();
        assert_eq!(result.expires_at, Some("2024-12-31T23:59:59Z".to_string()));
    }

    #[tokio::test]
    async fn test_get_authorization_401_error() {
        let mock_server = MockServer::start().await;

        Mock::given(matchers::method("GET"))
            .and(matchers::path("/authorization.json"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let config = Config::builder()
            .base_url(mock_server.uri())
            .max_retries(0)
            .build()
            .unwrap();
        let auth = BearerAuth::from_token("test-token");
        let http = std::sync::Arc::new(HttpClient::new(config, auth).unwrap());
        let service = AuthorizationService::new(http);

        let result = service.get().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), crate::error::ErrorCode::AuthRequired);
    }

    #[tokio::test]
    async fn test_get_authorization_invalid_json() {
        let mock_server = MockServer::start().await;

        Mock::given(matchers::method("GET"))
            .and(matchers::path("/authorization.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
            .mount(&mock_server)
            .await;

        let config = Config::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap();
        let auth = BearerAuth::from_token("test-token");
        let http = std::sync::Arc::new(HttpClient::new(config, auth).unwrap());
        let service = AuthorizationService::new(http);

        let result = service.get().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), crate::error::ErrorCode::Usage);
    }

    #[test]
    fn test_authorization_struct() {
        let auth = Authorization {
            id: 123,
            name: "Test".to_string(),
            email_address: "test@example.com".to_string(),
            identity_id: 456,
            expires_at: Some("2024-01-01".to_string()),
        };
        assert_eq!(auth.id, 123);
        assert_eq!(auth.name, "Test");
        assert_eq!(auth.email_address, "test@example.com");
        assert_eq!(auth.identity_id, 456);
        assert_eq!(auth.expires_at, Some("2024-01-01".to_string()));
    }

    #[test]
    fn test_authorization_clone() {
        let auth = Authorization {
            id: 123,
            name: "Test".to_string(),
            email_address: "test@example.com".to_string(),
            identity_id: 456,
            expires_at: None,
        };
        let cloned = auth.clone();
        assert_eq!(auth.id, cloned.id);
        assert_eq!(auth.name, cloned.name);
    }
}
