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
}
