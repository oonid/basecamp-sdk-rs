use std::sync::Arc;
use std::time::Duration;

pub trait BasecampHooks: Send + Sync {
    fn on_operation_start(&self, _info: &OperationInfo) {}
    fn on_operation_end(&self, _info: &OperationInfo, _result: &OperationResult) {}
    fn on_request_start(&self, _info: &RequestInfo) {}
    fn on_request_end(&self, _info: &RequestInfo, _result: &RequestResult) {}
    fn on_retry(
        &self,
        _info: &RequestInfo,
        _attempt: u32,
        _error: &crate::error::BasecampError,
        _delay: Duration,
    ) {
    }
    fn on_paginate(&self, _url: &str, _page: u32) {}
}

#[derive(Debug, Clone)]
pub struct OperationInfo {
    pub service: String,
    pub operation: String,
    pub resource_type: Option<String>,
    pub is_mutation: bool,
    pub project_id: Option<i64>,
    pub resource_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct OperationResult {
    pub success: bool,
    pub duration: Duration,
    pub error: Option<String>,
    pub error_code: Option<crate::error::ErrorCode>,
}

#[derive(Debug, Clone)]
pub struct RequestInfo {
    pub method: String,
    pub url: String,
    pub attempt: u32,
}

#[derive(Debug, Clone)]
pub struct RequestResult {
    pub status: Option<u16>,
    pub duration: Duration,
    pub success: bool,
    pub request_id: Option<String>,
}

pub struct NoOpHooks;

impl BasecampHooks for NoOpHooks {}

pub fn no_hooks() -> Arc<dyn BasecampHooks> {
    Arc::new(NoOpHooks)
}
