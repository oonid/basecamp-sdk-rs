use std::io::{self, Write};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU64, Ordering};
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

impl OperationInfo {
    pub fn new(service: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            operation: operation.into(),
            resource_type: None,
            is_mutation: false,
            project_id: None,
            resource_id: None,
        }
    }

    pub fn with_resource_type(mut self, resource_type: impl Into<String>) -> Self {
        self.resource_type = Some(resource_type.into());
        self
    }

    pub fn with_mutation(mut self, is_mutation: bool) -> Self {
        self.is_mutation = is_mutation;
        self
    }

    pub fn with_project_id(mut self, project_id: i64) -> Self {
        self.project_id = Some(project_id);
        self
    }

    pub fn with_resource_id(mut self, resource_id: i64) -> Self {
        self.resource_id = Some(resource_id);
        self
    }
}

#[derive(Debug, Clone)]
pub struct OperationResult {
    pub success: bool,
    pub duration: Duration,
    pub error: Option<String>,
    pub error_code: Option<crate::error::ErrorCode>,
}

impl OperationResult {
    pub fn success(duration: Duration) -> Self {
        Self {
            success: true,
            duration,
            error: None,
            error_code: None,
        }
    }

    pub fn failure(
        duration: Duration,
        error: impl Into<String>,
        error_code: crate::error::ErrorCode,
    ) -> Self {
        Self {
            success: false,
            duration,
            error: Some(error.into()),
            error_code: Some(error_code),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RequestInfo {
    pub method: String,
    pub url: String,
    pub attempt: u32,
}

impl RequestInfo {
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            url: url.into(),
            attempt: 1,
        }
    }

    pub fn with_attempt(mut self, attempt: u32) -> Self {
        self.attempt = attempt;
        self
    }
}

#[derive(Debug, Clone)]
pub struct RequestResult {
    pub status: Option<u16>,
    pub duration: Duration,
    pub success: bool,
    pub request_id: Option<String>,
}

impl RequestResult {
    pub fn success(status: u16, duration: Duration) -> Self {
        Self {
            status: Some(status),
            duration,
            success: true,
            request_id: None,
        }
    }

    pub fn failure(duration: Duration) -> Self {
        Self {
            status: None,
            duration,
            success: false,
            request_id: None,
        }
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

pub fn safe_hook<F>(f: F)
where
    F: FnOnce(),
{
    let result = std::panic::catch_unwind(AssertUnwindSafe(f));
    if let Err(e) = result {
        if let Some(s) = e.downcast_ref::<&str>() {
            eprintln!("[basecamp] Hook error: {}", s);
        } else if let Some(s) = e.downcast_ref::<String>() {
            eprintln!("[basecamp] Hook error: {}", s);
        } else {
            eprintln!("[basecamp] Hook error: unknown panic");
        }
    }
}

pub struct ChainedHooks {
    hooks: Vec<Arc<dyn BasecampHooks>>,
}

impl ChainedHooks {
    pub fn new(hooks: Vec<Arc<dyn BasecampHooks>>) -> Self {
        Self { hooks }
    }

    pub fn add(&mut self, hook: Arc<dyn BasecampHooks>) {
        self.hooks.push(hook);
    }
}

impl BasecampHooks for ChainedHooks {
    fn on_operation_start(&self, info: &OperationInfo) {
        for hook in &self.hooks {
            safe_hook(|| hook.on_operation_start(info));
        }
    }

    fn on_operation_end(&self, info: &OperationInfo, result: &OperationResult) {
        for hook in self.hooks.iter().rev() {
            safe_hook(|| hook.on_operation_end(info, result));
        }
    }

    fn on_request_start(&self, info: &RequestInfo) {
        for hook in &self.hooks {
            safe_hook(|| hook.on_request_start(info));
        }
    }

    fn on_request_end(&self, info: &RequestInfo, result: &RequestResult) {
        for hook in self.hooks.iter().rev() {
            safe_hook(|| hook.on_request_end(info, result));
        }
    }

    fn on_retry(
        &self,
        info: &RequestInfo,
        attempt: u32,
        error: &crate::error::BasecampError,
        delay: Duration,
    ) {
        for hook in &self.hooks {
            safe_hook(|| hook.on_retry(info, attempt, error, delay));
        }
    }

    fn on_paginate(&self, url: &str, page: u32) {
        for hook in &self.hooks {
            safe_hook(|| hook.on_paginate(url, page));
        }
    }
}

pub fn chain_hooks(hooks: Vec<Arc<dyn BasecampHooks>>) -> Arc<dyn BasecampHooks> {
    if hooks.len() == 1 {
        return hooks.into_iter().next().unwrap();
    }
    Arc::new(ChainedHooks::new(hooks))
}

#[derive(Debug, Clone, Copy)]
pub enum ConsoleLogLevel {
    Operations,
    Requests,
    Verbose,
}

pub struct ConsoleHooks {
    level: ConsoleLogLevel,
}

impl ConsoleHooks {
    pub fn new() -> Self {
        Self {
            level: ConsoleLogLevel::Requests,
        }
    }

    pub fn with_level(level: ConsoleLogLevel) -> Self {
        Self { level }
    }

    fn log(&self, msg: &str) {
        let _ = writeln!(io::stderr(), "[basecamp] {}", msg);
    }
}

impl Default for ConsoleHooks {
    fn default() -> Self {
        Self::new()
    }
}

impl BasecampHooks for ConsoleHooks {
    fn on_operation_start(&self, info: &OperationInfo) {
        self.log(&format!("{}.{} started", info.service, info.operation));
    }

    fn on_operation_end(&self, info: &OperationInfo, result: &OperationResult) {
        let status = if result.success { "ok" } else { "error" };
        let duration_ms = result.duration.as_millis();
        self.log(&format!(
            "{}.{} {} ({}ms)",
            info.service, info.operation, status, duration_ms
        ));
    }

    fn on_request_start(&self, info: &RequestInfo) {
        if matches!(
            self.level,
            ConsoleLogLevel::Requests | ConsoleLogLevel::Verbose
        ) {
            self.log(&format!(
                "{} {} (attempt {})",
                info.method, info.url, info.attempt
            ));
        }
    }

    fn on_request_end(&self, info: &RequestInfo, result: &RequestResult) {
        if matches!(
            self.level,
            ConsoleLogLevel::Requests | ConsoleLogLevel::Verbose
        ) {
            let status = result
                .status
                .map(|s| s.to_string())
                .unwrap_or_else(|| "error".to_string());
            let duration_ms = result.duration.as_millis();
            self.log(&format!(
                "{} {} -> {} ({}ms)",
                info.method, info.url, status, duration_ms
            ));
        }
    }

    fn on_retry(
        &self,
        info: &RequestInfo,
        attempt: u32,
        error: &crate::error::BasecampError,
        delay: Duration,
    ) {
        if matches!(self.level, ConsoleLogLevel::Verbose) {
            let delay_ms = delay.as_millis();
            self.log(&format!(
                "retrying {} {} (attempt {}, delay {}ms): {}",
                info.method, info.url, attempt, delay_ms, error
            ));
        }
    }

    fn on_paginate(&self, url: &str, page: u32) {
        if matches!(
            self.level,
            ConsoleLogLevel::Requests | ConsoleLogLevel::Verbose
        ) {
            self.log(&format!("paginate page {}: {}", page, url));
        }
    }
}

pub fn console_hooks() -> Arc<dyn BasecampHooks> {
    Arc::new(ConsoleHooks::new())
}

pub struct TimingHooks {
    pub total_operation_us: AtomicU64,
    pub total_request_us: AtomicU64,
    pub request_count: AtomicU64,
    pub retry_count: AtomicU64,
}

impl TimingHooks {
    pub fn new() -> Self {
        Self {
            total_operation_us: AtomicU64::new(0),
            total_request_us: AtomicU64::new(0),
            request_count: AtomicU64::new(0),
            retry_count: AtomicU64::new(0),
        }
    }

    pub fn avg_request_duration(&self) -> Duration {
        let count = self.request_count.load(Ordering::SeqCst);
        if count == 0 {
            return Duration::ZERO;
        }
        let total_us = self.total_request_us.load(Ordering::SeqCst);
        Duration::from_micros(total_us / count)
    }

    pub fn total_operation_duration(&self) -> Duration {
        Duration::from_micros(self.total_operation_us.load(Ordering::SeqCst))
    }

    pub fn total_request_duration(&self) -> Duration {
        Duration::from_micros(self.total_request_us.load(Ordering::SeqCst))
    }
}

impl Default for TimingHooks {
    fn default() -> Self {
        Self::new()
    }
}

impl BasecampHooks for TimingHooks {
    fn on_operation_end(&self, _info: &OperationInfo, result: &OperationResult) {
        let us = result.duration.as_micros() as u64;
        self.total_operation_us.fetch_add(us, Ordering::SeqCst);
    }

    fn on_request_end(&self, _info: &RequestInfo, result: &RequestResult) {
        let us = result.duration.as_micros() as u64;
        self.total_request_us.fetch_add(us, Ordering::SeqCst);
        self.request_count.fetch_add(1, Ordering::SeqCst);
    }

    fn on_retry(
        &self,
        _info: &RequestInfo,
        _attempt: u32,
        _error: &crate::error::BasecampError,
        _delay: Duration,
    ) {
        self.retry_count.fetch_add(1, Ordering::SeqCst);
    }
}

pub fn timing_hooks() -> Arc<TimingHooks> {
    Arc::new(TimingHooks::new())
}

pub struct NoOpHooks;

impl BasecampHooks for NoOpHooks {}

pub fn no_hooks() -> Arc<dyn BasecampHooks> {
    Arc::new(NoOpHooks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::BasecampError;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Mutex;

    fn make_operation_info() -> OperationInfo {
        OperationInfo::new("projects", "list")
    }

    fn make_request_info() -> RequestInfo {
        RequestInfo::new("GET", "https://example.com/test")
    }

    fn make_operation_result() -> OperationResult {
        OperationResult::success(Duration::from_millis(50))
    }

    fn make_request_result() -> RequestResult {
        RequestResult::success(200, Duration::from_millis(45))
    }

    mod operation_info {
        use super::*;

        #[test]
        fn test_new() {
            let info = OperationInfo::new("projects", "list");
            assert_eq!(info.service, "projects");
            assert_eq!(info.operation, "list");
            assert!(info.resource_type.is_none());
            assert!(!info.is_mutation);
            assert!(info.project_id.is_none());
            assert!(info.resource_id.is_none());
        }

        #[test]
        fn test_builder_methods() {
            let info = OperationInfo::new("todos", "create")
                .with_resource_type("Todo")
                .with_mutation(true)
                .with_project_id(123)
                .with_resource_id(456);

            assert_eq!(info.service, "todos");
            assert_eq!(info.operation, "create");
            assert_eq!(info.resource_type, Some("Todo".to_string()));
            assert!(info.is_mutation);
            assert_eq!(info.project_id, Some(123));
            assert_eq!(info.resource_id, Some(456));
        }

        #[test]
        fn test_clone() {
            let info = OperationInfo::new("test", "op");
            let cloned = info.clone();
            assert_eq!(info.service, cloned.service);
        }
    }

    mod operation_result {
        use super::*;
        use crate::error::ErrorCode;

        #[test]
        fn test_success() {
            let result = OperationResult::success(Duration::from_millis(100));
            assert!(result.success);
            assert_eq!(result.duration, Duration::from_millis(100));
            assert!(result.error.is_none());
            assert!(result.error_code.is_none());
        }

        #[test]
        fn test_failure() {
            let result = OperationResult::failure(
                Duration::from_millis(50),
                "Not found",
                ErrorCode::NotFound,
            );
            assert!(!result.success);
            assert_eq!(result.error, Some("Not found".to_string()));
            assert_eq!(result.error_code, Some(ErrorCode::NotFound));
        }
    }

    mod request_info {
        use super::*;

        #[test]
        fn test_new() {
            let info = RequestInfo::new("POST", "https://example.com/create");
            assert_eq!(info.method, "POST");
            assert_eq!(info.url, "https://example.com/create");
            assert_eq!(info.attempt, 1);
        }

        #[test]
        fn test_with_attempt() {
            let info = RequestInfo::new("GET", "https://example.com/test").with_attempt(3);
            assert_eq!(info.attempt, 3);
        }
    }

    mod request_result {
        use super::*;

        #[test]
        fn test_success() {
            let result = RequestResult::success(200, Duration::from_millis(50));
            assert_eq!(result.status, Some(200));
            assert!(result.success);
            assert_eq!(result.duration, Duration::from_millis(50));
        }

        #[test]
        fn test_failure() {
            let result = RequestResult::failure(Duration::from_millis(10));
            assert!(result.status.is_none());
            assert!(!result.success);
        }

        #[test]
        fn test_with_request_id() {
            let result =
                RequestResult::success(200, Duration::from_millis(10)).with_request_id("req-123");
            assert_eq!(result.request_id, Some("req-123".to_string()));
        }
    }

    mod safe_hook {
        use super::*;

        #[test]
        fn test_calls_function_normally() {
            let called = Arc::new(AtomicUsize::new(0));
            let called_clone = called.clone();
            safe_hook(move || {
                called_clone.fetch_add(1, Ordering::SeqCst);
            });
            assert_eq!(called.load(Ordering::SeqCst), 1);
        }

        #[test]
        fn test_swallows_panic_string() {
            safe_hook(|| {
                panic!("test panic");
            });
        }

        #[test]
        fn test_swallows_panic_static_str() {
            std::panic::set_hook(Box::new(|_| {}));
            safe_hook(|| {
                std::panic::panic_any("static str panic");
            });
            let _ = std::panic::take_hook();
        }

        #[test]
        fn test_swallows_panic_other() {
            std::panic::set_hook(Box::new(|_| {}));
            safe_hook(|| {
                std::panic::panic_any(42usize);
            });
            let _ = std::panic::take_hook();
        }
    }

    mod no_op_hooks {
        use super::*;

        #[test]
        fn test_all_methods_are_no_ops() {
            let hooks = NoOpHooks;
            let info = make_operation_info();
            let result = make_operation_result();
            let req_info = make_request_info();
            let req_result = make_request_result();
            let error = BasecampError::Network {
                message: "test".to_string(),
            };

            hooks.on_operation_start(&info);
            hooks.on_operation_end(&info, &result);
            hooks.on_request_start(&req_info);
            hooks.on_request_end(&req_info, &req_result);
            hooks.on_retry(&req_info, 2, &error, Duration::from_secs(1));
            hooks.on_paginate("https://example.com", 1);
        }
    }

    mod chained_hooks {
        use super::*;

        struct RecordingHook {
            calls: Arc<Mutex<Vec<String>>>,
            name: String,
        }

        impl RecordingHook {
            fn new(name: &str, calls: Arc<Mutex<Vec<String>>>) -> Self {
                Self {
                    calls,
                    name: name.to_string(),
                }
            }
        }

        impl BasecampHooks for RecordingHook {
            fn on_request_start(&self, _info: &RequestInfo) {
                self.calls
                    .lock()
                    .unwrap()
                    .push(format!("{}:on_request_start", self.name));
            }

            fn on_request_end(&self, _info: &RequestInfo, _result: &RequestResult) {
                self.calls
                    .lock()
                    .unwrap()
                    .push(format!("{}:on_request_end", self.name));
            }
        }

        #[test]
        fn test_calls_all_hooks_in_order() {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let hooks: Vec<Arc<dyn BasecampHooks>> = vec![
                Arc::new(RecordingHook::new("a", calls.clone())),
                Arc::new(RecordingHook::new("b", calls.clone())),
                Arc::new(RecordingHook::new("c", calls.clone())),
            ];
            let chained = ChainedHooks::new(hooks);

            chained.on_request_start(&make_request_info());

            let calls = calls.lock().unwrap();
            assert_eq!(
                *calls,
                vec![
                    "a:on_request_start",
                    "b:on_request_start",
                    "c:on_request_start"
                ]
            );
        }

        #[test]
        fn test_on_request_end_calls_in_reverse() {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let hooks: Vec<Arc<dyn BasecampHooks>> = vec![
                Arc::new(RecordingHook::new("a", calls.clone())),
                Arc::new(RecordingHook::new("b", calls.clone())),
            ];
            let chained = ChainedHooks::new(hooks);

            chained.on_request_end(&make_request_info(), &make_request_result());

            let calls = calls.lock().unwrap();
            assert_eq!(*calls, vec!["b:on_request_end", "a:on_request_end"]);
        }

        #[test]
        fn test_single_hook_returned_directly() {
            let hook: Arc<dyn BasecampHooks> = Arc::new(NoOpHooks);
            let result = chain_hooks(vec![hook.clone()]);
            assert!(Arc::ptr_eq(&hook, &result));
        }

        #[test]
        fn test_empty_chain_valid() {
            let chained = ChainedHooks::new(vec![]);
            chained.on_request_start(&make_request_info());
        }

        #[test]
        fn test_add_hook() {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let mut chained = ChainedHooks::new(vec![]);
            chained.add(Arc::new(RecordingHook::new("added", calls.clone())));

            chained.on_request_start(&make_request_info());

            let calls = calls.lock().unwrap();
            assert_eq!(*calls, vec!["added:on_request_start"]);
        }

        #[test]
        fn test_exception_in_one_hook_does_not_block_others() {
            struct BadHook;
            impl BasecampHooks for BadHook {
                fn on_request_start(&self, _info: &RequestInfo) {
                    panic!("boom");
                }
            }

            let calls = Arc::new(Mutex::new(Vec::new()));
            let hooks: Vec<Arc<dyn BasecampHooks>> = vec![
                Arc::new(BadHook),
                Arc::new(RecordingHook::new("good", calls.clone())),
            ];
            let chained = ChainedHooks::new(hooks);

            chained.on_request_start(&make_request_info());

            let calls = calls.lock().unwrap();
            assert_eq!(*calls, vec!["good:on_request_start"]);
        }
    }

    mod chain_hooks_fn {
        use super::*;

        #[test]
        fn test_returns_chained_hooks() {
            let hooks: Vec<Arc<dyn BasecampHooks>> = vec![Arc::new(NoOpHooks), Arc::new(NoOpHooks)];
            let chained = chain_hooks(hooks);
            chained.on_request_start(&make_request_info());
        }
    }

    mod console_hooks {
        use super::*;

        #[test]
        fn test_on_operation_start() {
            let hooks = ConsoleHooks::new();
            let info = OperationInfo::new("Projects", "list");
            hooks.on_operation_start(&info);
        }

        #[test]
        fn test_on_operation_end_success() {
            let hooks = ConsoleHooks::new();
            let info = OperationInfo::new("Projects", "list");
            let result = OperationResult::success(Duration::from_millis(52));
            hooks.on_operation_end(&info, &result);
        }

        #[test]
        fn test_on_operation_end_failure() {
            let hooks = ConsoleHooks::new();
            let info = OperationInfo::new("Projects", "list");
            let result = OperationResult::failure(
                Duration::from_millis(10),
                "Not found",
                crate::error::ErrorCode::NotFound,
            );
            hooks.on_operation_end(&info, &result);
        }

        #[test]
        fn test_on_request_start() {
            let hooks = ConsoleHooks::with_level(ConsoleLogLevel::Requests);
            let info = RequestInfo::new("GET", "https://example.com/test").with_attempt(1);
            hooks.on_request_start(&info);
        }

        #[test]
        fn test_on_request_end() {
            let hooks = ConsoleHooks::with_level(ConsoleLogLevel::Requests);
            let info = RequestInfo::new("GET", "https://example.com/test");
            let result = RequestResult::success(200, Duration::from_millis(45));
            hooks.on_request_end(&info, &result);
        }

        #[test]
        fn test_on_retry_verbose_only() {
            let hooks = ConsoleHooks::with_level(ConsoleLogLevel::Verbose);
            let info = RequestInfo::new("GET", "https://example.com/test");
            let error = BasecampError::Network {
                message: "timeout".to_string(),
            };
            hooks.on_retry(&info, 2, &error, Duration::from_secs(1));
        }

        #[test]
        fn test_on_retry_not_logged_in_requests_mode() {
            let hooks = ConsoleHooks::with_level(ConsoleLogLevel::Requests);
            let info = RequestInfo::new("GET", "https://example.com/test");
            let error = BasecampError::Network {
                message: "timeout".to_string(),
            };
            hooks.on_retry(&info, 2, &error, Duration::from_secs(1));
        }

        #[test]
        fn test_on_paginate() {
            let hooks = ConsoleHooks::with_level(ConsoleLogLevel::Requests);
            hooks.on_paginate("https://example.com/page2", 2);
        }

        #[test]
        fn test_console_hooks_fn() {
            let hooks = console_hooks();
            hooks.on_operation_start(&make_operation_info());
        }
    }

    mod timing_hooks {
        use super::*;

        #[test]
        fn test_new() {
            let hooks = TimingHooks::new();
            assert_eq!(hooks.total_operation_us.load(Ordering::SeqCst), 0);
            assert_eq!(hooks.total_request_us.load(Ordering::SeqCst), 0);
            assert_eq!(hooks.request_count.load(Ordering::SeqCst), 0);
            assert_eq!(hooks.retry_count.load(Ordering::SeqCst), 0);
        }

        #[test]
        fn test_accumulates_operation_duration() {
            let hooks = TimingHooks::new();
            let info = make_operation_info();
            let result1 = OperationResult::success(Duration::from_micros(100));
            let result2 = OperationResult::success(Duration::from_micros(200));

            hooks.on_operation_end(&info, &result1);
            hooks.on_operation_end(&info, &result2);

            assert_eq!(hooks.total_operation_us.load(Ordering::SeqCst), 300);
        }

        #[test]
        fn test_accumulates_request_duration() {
            let hooks = TimingHooks::new();
            let info = make_request_info();
            let result1 = RequestResult::success(200, Duration::from_micros(50));
            let result2 = RequestResult::success(200, Duration::from_micros(150));

            hooks.on_request_end(&info, &result1);
            hooks.on_request_end(&info, &result2);

            assert_eq!(hooks.total_request_us.load(Ordering::SeqCst), 200);
            assert_eq!(hooks.request_count.load(Ordering::SeqCst), 2);
        }

        #[test]
        fn test_counts_retries() {
            let hooks = TimingHooks::new();
            let info = make_request_info();
            let error = BasecampError::Network {
                message: "timeout".to_string(),
            };

            hooks.on_retry(&info, 2, &error, Duration::from_secs(1));
            hooks.on_retry(&info, 3, &error, Duration::from_secs(2));

            assert_eq!(hooks.retry_count.load(Ordering::SeqCst), 2);
        }

        #[test]
        fn test_avg_request_duration() {
            let hooks = TimingHooks::new();
            let info = make_request_info();

            hooks.on_request_end(
                &info,
                &RequestResult::success(200, Duration::from_micros(100)),
            );
            hooks.on_request_end(
                &info,
                &RequestResult::success(200, Duration::from_micros(200)),
            );
            hooks.on_request_end(
                &info,
                &RequestResult::success(200, Duration::from_micros(300)),
            );

            assert_eq!(hooks.avg_request_duration(), Duration::from_micros(200));
        }

        #[test]
        fn test_avg_request_duration_zero_requests() {
            let hooks = TimingHooks::new();
            assert_eq!(hooks.avg_request_duration(), Duration::ZERO);
        }

        #[test]
        fn test_timing_hooks_fn() {
            let hooks = timing_hooks();
            hooks.on_request_end(&make_request_info(), &make_request_result());
        }

        #[test]
        fn test_is_send_sync() {
            fn assert_send_sync<T: Send + Sync>() {}
            assert_send_sync::<TimingHooks>();
        }
    }

    mod no_hooks_fn {
        use super::*;

        #[test]
        fn test_no_hooks() {
            let hooks = no_hooks();
            hooks.on_operation_start(&make_operation_info());
        }
    }
}
