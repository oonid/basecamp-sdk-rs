# Hooks Specification

## Overview

The hooks module provides an observability interface for monitoring SDK operations.
It enables logging, metrics collection, and custom instrumentation.

## Trait: BasecampHooks

```rust
use std::time::Duration;

/// Hook interface for SDK observability.
/// 
/// All methods have default no-op implementations.
/// Implement only the hooks you need.
pub trait BasecampHooks: Send + Sync {
    /// Called when an operation starts.
    fn on_operation_start(&self, info: &OperationInfo) {}
    
    /// Called when an operation completes.
    fn on_operation_end(&self, info: &OperationInfo, result: &OperationResult) {}
    
    /// Called when an HTTP request starts.
    fn on_request_start(&self, info: &RequestInfo) {}
    
    /// Called when an HTTP request completes.
    fn on_request_end(&self, info: &RequestInfo, result: &RequestResult) {}
    
    /// Called when a request is retried.
    fn on_retry(&self, info: &RequestInfo, attempt: u32, error: &BasecampError, delay: Duration) {}
    
    /// Called for each page fetched during pagination.
    fn on_paginate(&self, url: &str, page: u32) {}
}
```

## Data Structures

### OperationInfo

```rust
/// Information about an SDK operation.
#[derive(Debug, Clone)]
pub struct OperationInfo {
    /// Service name (e.g., "projects", "todos").
    pub service: String,
    
    /// Operation name (e.g., "list", "create", "get").
    pub operation: String,
    
    /// Resource type being operated on (if applicable).
    pub resource_type: Option<String>,
    
    /// Whether this operation modifies state.
    pub is_mutation: bool,
    
    /// Project/bucket ID (if applicable).
    pub project_id: Option<i64>,
    
    /// Resource ID (if applicable).
    pub resource_id: Option<i64>,
}
```

### OperationResult

```rust
/// Result of an operation.
#[derive(Debug, Clone)]
pub struct OperationResult {
    /// Whether the operation succeeded.
    pub success: bool,
    
    /// Duration of the operation.
    pub duration: Duration,
    
    /// Error message if failed.
    pub error: Option<String>,
    
    /// Error code if failed.
    pub error_code: Option<ErrorCode>,
}
```

### RequestInfo

```rust
/// Information about an HTTP request.
#[derive(Debug, Clone)]
pub struct RequestInfo {
    /// HTTP method (GET, POST, etc.).
    pub method: String,
    
    /// Request URL.
    pub url: String,
    
    /// Attempt number (1 for initial, 2+ for retries).
    pub attempt: u32,
}
```

### RequestResult

```rust
/// Result of an HTTP request.
#[derive(Debug, Clone)]
pub struct RequestResult {
    /// HTTP status code.
    pub status: Option<u16>,
    
    /// Duration of the request.
    pub duration: Duration,
    
    /// Whether the request succeeded (2xx status).
    pub success: bool,
    
    /// Request ID from response headers.
    pub request_id: Option<String>,
}
```

## Hook Chaining

Multiple hooks can be composed together:

```rust
/// Chain multiple hooks together.
/// Hooks are called in the order they are provided.
pub struct ChainedHooks {
    hooks: Vec<Arc<dyn BasecampHooks>>,
}

impl ChainedHooks {
    /// Create a new chain of hooks.
    pub fn new(hooks: Vec<Arc<dyn BasecampHooks>>) -> Self;
    
    /// Add a hook to the chain.
    pub fn add(&mut self, hook: Arc<dyn BasecampHooks>);
}

impl BasecampHooks for ChainedHooks {
    fn on_operation_start(&self, info: &OperationInfo) {
        for hook in &self.hooks {
            hook.on_operation_start(info);
        }
    }
    
    // ... other methods similarly delegate to all hooks
}

/// Convenience function to chain hooks.
pub fn chain_hooks(hooks: Vec<Arc<dyn BasecampHooks>>) -> Arc<dyn BasecampHooks> {
    Arc::new(ChainedHooks::new(hooks))
}
```

- `[unit]` Chained hooks are called in order
- `[unit]` Empty chain is valid
- `[unit]` Single hook chain delegates correctly

## Console Hooks

A built-in hook for logging to stderr:

```rust
/// Console logging hooks.
/// Logs operations and requests to stderr.
pub struct ConsoleHooks {
    level: ConsoleLogLevel,
}

#[derive(Debug, Clone, Copy)]
pub enum ConsoleLogLevel {
    /// Log only operations.
    Operations,
    /// Log operations and requests.
    Requests,
    /// Log everything including retries.
    Verbose,
}

impl ConsoleHooks {
    /// Create console hooks with default level (Requests).
    pub fn new() -> Self;
    
    /// Set the logging level.
    pub fn with_level(level: ConsoleLogLevel) -> Self;
}

/// Convenience function to create console hooks.
pub fn console_hooks() -> Arc<dyn BasecampHooks> {
    Arc::new(ConsoleHooks::new())
}
```

### Output Format

```
[OPERATION] projects.list started
[REQUEST] GET https://3.basecampapi.com/999/buckets/123/projects.json (attempt 1)
[REQUEST] GET https://... completed 200 (45ms)
[OPERATION] projects.list completed (52ms)
```

- `[unit]` Operations are logged
- `[unit]` Requests are logged
- `[unit]` Retries are logged in verbose mode
- `[manual]` Output format matches specification

## No-Op Hooks

Default implementation provides no-op behavior:

```rust
/// No-op hooks that do nothing.
pub struct NoOpHooks;

impl BasecampHooks for NoOpHooks {}

/// Convenience function for no-op hooks.
pub fn no_hooks() -> Arc<dyn BasecampHooks> {
    Arc::new(NoOpHooks)
}
```

- `[unit]` All methods are no-ops

## Timing Hooks

Hook for collecting timing metrics:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

/// Hooks for collecting timing metrics.
pub struct TimingHooks {
    /// Total operation duration in microseconds.
    pub total_operation_us: AtomicU64,
    
    /// Total request duration in microseconds.
    pub total_request_us: AtomicU64,
    
    /// Number of requests made.
    pub request_count: AtomicU64,
    
    /// Number of retries.
    pub retry_count: AtomicU64,
}

impl TimingHooks {
    pub fn new() -> Self;
    
    /// Get average request duration.
    pub fn avg_request_duration(&self) -> Duration;
}
```

- `[unit]` Accumulates operation durations
- `[unit]` Accumulates request durations
- `[unit]` Counts requests and retries

## Tracing Integration

Optional integration with the `tracing` crate:

```rust
#[cfg(feature = "tracing")]
pub struct TracingHooks;

#[cfg(feature = "tracing")]
impl TracingHooks {
    pub fn new() -> Self;
}

#[cfg(feature = "tracing")]
impl BasecampHooks for TracingHooks {
    fn on_operation_start(&self, info: &OperationInfo) {
        tracing::info_span!(
            "basecamp_operation",
            service = %info.service,
            operation = %info.operation,
            is_mutation = info.is_mutation,
        );
    }
    
    // ... other methods use tracing events
}
```

## Hook Call Sequence

For a typical operation:

```
1. on_operation_start(projects.list)
2. on_request_start(GET /projects.json, attempt=1)
3. [if retry needed]
   a. on_retry(GET /projects.json, attempt=1, error=RateLimit, delay=1s)
   b. on_request_start(GET /projects.json, attempt=2)
4. on_request_end(GET /projects.json, status=200, duration=45ms)
5. [if paginated]
   a. on_paginate(/projects.json?page=2, page=2)
   b. on_request_start(GET ...page=2, attempt=1)
   c. on_request_end(GET ...page=2, ...)
6. on_operation_end(projects.list, success=true, duration=52ms)
```

## Verification

- `[unit]` All hook methods have safe defaults
- `[unit]` Chained hooks call all members
- `[unit]` Console hooks output correctly
- `[unit]` Timing hooks accumulate correctly
- `[manual]` Tracing integration works
- `[conformance]` Hook sequence matches specification
