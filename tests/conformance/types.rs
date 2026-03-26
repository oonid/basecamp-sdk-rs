use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct TestCase {
    pub name: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub description: Option<String>,
    pub operation: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default, alias = "pathParams")]
    pub path_params: HashMap<String, serde_json::Value>,
    #[serde(default, alias = "queryParams")]
    #[allow(dead_code)]
    pub query_params: HashMap<String, serde_json::Value>,
    #[serde(default, alias = "requestBody")]
    pub request_body: Option<serde_json::Value>,
    #[serde(default, alias = "mockResponses")]
    pub mock_responses: Vec<MockResponse>,
    pub assertions: Vec<Assertion>,
    #[serde(default)]
    #[allow(dead_code)]
    pub tags: Vec<String>,
    #[serde(default, alias = "configOverrides")]
    pub config_overrides: Option<ConfigOverrides>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigOverrides {
    #[serde(default, alias = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(default, alias = "maxPages")]
    pub max_pages: Option<u32>,
    #[serde(default, alias = "maxItems")]
    pub max_items: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MockResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
    #[serde(default)]
    pub delay: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Assertion {
    #[serde(rename = "type")]
    pub assertion_type: String,
    #[serde(default)]
    pub expected: Option<serde_json::Value>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    pub max: Option<f64>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub message: Option<String>,
}

impl TestResult {
    pub fn pass(name: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: true,
            message: None,
        }
    }

    pub fn fail(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: false,
            message: Some(message.to_string()),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RequestTracker {
    pub count: usize,
    pub times: Vec<std::time::Instant>,
    pub paths: Vec<String>,
    pub headers: Vec<Vec<(String, String)>>,
}

impl RequestTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, path: String, headers: Vec<(String, String)>) {
        self.count += 1;
        self.times.push(std::time::Instant::now());
        self.paths.push(path);
        self.headers.push(headers);
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.count = 0;
        self.times.clear();
        self.paths.clear();
        self.headers.clear();
    }

    pub fn delays_between_requests_ms(&self) -> Vec<u64> {
        self.times
            .windows(2)
            .map(|w| w[1].duration_since(w[0]).as_millis() as u64)
            .collect()
    }
}

#[derive(Debug)]
pub struct OperationResult {
    pub error: Option<basecamp_sdk_rs::BasecampError>,
    #[allow(dead_code)]
    pub http_status: Option<u16>,
    pub meta: Option<HashMap<String, serde_json::Value>>,
    pub body: Option<serde_json::Value>,
}
