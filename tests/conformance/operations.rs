use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use basecamp_sdk_rs::{BasecampError, HttpClient};

use super::types::{OperationResult, RequestTracker, TestCase};

pub struct OperationDispatcher {
    http_client: HttpClient,
    tracker: Arc<Mutex<RequestTracker>>,
}

impl OperationDispatcher {
    pub fn new(http_client: HttpClient, tracker: Arc<Mutex<RequestTracker>>) -> Self {
        Self {
            http_client,
            tracker,
        }
    }

    pub async fn execute(&self, tc: &TestCase) -> OperationResult {
        let path = self.build_path(tc);
        let method = tc.method.as_deref().unwrap_or("GET").to_uppercase();
        
        match method.as_str() {
            "GET" => self.execute_get(tc, &path).await,
            "POST" => self.execute_post(tc, &path).await,
            "PUT" => self.execute_put(tc, &path).await,
            "DELETE" => self.execute_delete(tc, &path).await,
            _ => OperationResult {
                error: Some(BasecampError::Usage {
                    message: format!("Unknown method: {}", method),
                    hint: None,
                }),
                http_status: None,
                meta: None,
                body: None,
            },
        }
    }

    fn build_path(&self, tc: &TestCase) -> String {
        let mut path = tc.path.clone().unwrap_or_default();
        
        for (key, value) in &tc.path_params {
            path = path.replace(&format!("{{{}}}", key), &value_to_string(value));
        }
        
        path
    }

    async fn execute_get(&self, tc: &TestCase, path: &str) -> OperationResult {
        match tc.operation.as_str() {
            "ListProjects" | "ListTodos" | "ListWebhooks" => {
                let result = self.http_client.get_paginated::<serde_json::Value>(path, None).await;
                self.record_request(path);
                
                match result {
                    Ok(list_result) => {
                        let mut meta = HashMap::new();
                        meta.insert(
                            "totalCount".to_string(),
                            serde_json::json!(list_result.meta.total_count.unwrap_or(0)),
                        );
                        meta.insert(
                            "truncated".to_string(),
                            serde_json::json!(list_result.meta.truncated),
                        );
                        
                        OperationResult {
                            error: None,
                            http_status: Some(200),
                            meta: Some(meta),
                            body: Some(serde_json::json!(list_result.items)),
                        }
                    }
                    Err(e) => OperationResult {
                        error: Some(e),
                        http_status: None,
                        meta: None,
                        body: None,
                    },
                }
            }
            _ => {
                let result = self.http_client.get(path, None).await;
                self.record_request(path);
                
                match result {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        let body: serde_json::Value = response.json().await.unwrap_or(serde_json::json!(null));
                        OperationResult {
                            error: None,
                            http_status: Some(status),
                            meta: None,
                            body: Some(body),
                        }
                    }
                    Err(e) => {
                        let status = e.http_status();
                        OperationResult {
                            error: Some(e),
                            http_status: status,
                            meta: None,
                            body: None,
                        }
                    }
                }
            }
        }
    }

    async fn execute_post(&self, tc: &TestCase, path: &str) -> OperationResult {
        let body = tc.request_body.as_ref();
        
        let result = self.http_client.post(path, body, Some(&tc.operation)).await;
        self.record_request(path);
        
        match result {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response.json().await.unwrap_or(serde_json::json!(null));
                OperationResult {
                    error: None,
                    http_status: Some(status),
                    meta: None,
                    body: Some(body),
                }
            }
            Err(e) => {
                let status = e.http_status();
                OperationResult {
                    error: Some(e),
                    http_status: status,
                    meta: None,
                    body: None,
                }
            }
        }
    }

    async fn execute_put(&self, tc: &TestCase, path: &str) -> OperationResult {
        let body = tc.request_body.as_ref();
        
        let result = self.http_client.put(path, body, Some(&tc.operation)).await;
        self.record_request(path);
        
        match result {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response.json().await.unwrap_or(serde_json::json!(null));
                OperationResult {
                    error: None,
                    http_status: Some(status),
                    meta: None,
                    body: Some(body),
                }
            }
            Err(e) => {
                let status = e.http_status();
                OperationResult {
                    error: Some(e),
                    http_status: status,
                    meta: None,
                    body: None,
                }
            }
        }
    }

    async fn execute_delete(&self, _tc: &TestCase, path: &str) -> OperationResult {
        let result = self.http_client.delete(path, None).await;
        self.record_request(path);
        
        match result {
            Ok(response) => {
                let status = response.status().as_u16();
                OperationResult {
                    error: None,
                    http_status: Some(status),
                    meta: None,
                    body: None,
                }
            }
            Err(e) => {
                let status = e.http_status();
                OperationResult {
                    error: Some(e),
                    http_status: status,
                    meta: None,
                    body: None,
                }
            }
        }
    }

    fn record_request(&self, path: &str) {
        let mut tracker = self.tracker.lock().unwrap();
        tracker.record(path.to_string(), vec![]);
    }
}

fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => value.to_string(),
    }
}
