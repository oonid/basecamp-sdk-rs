# HTTP Client Specification

## Overview

The HTTP client module provides a robust HTTP transport layer with automatic retries,
token refresh on 401, pagination support, and security hardening.

## Interface: HttpClient

```rust
use reqwest::{Response, StatusCode};

/// HTTP client wrapper with retry and auth support.
pub struct HttpClient {
    inner: reqwest::Client,
    config: Config,
    auth: Arc<dyn AuthStrategy>,
    hooks: Option<Arc<dyn BasecampHooks>>,
}

impl HttpClient {
    /// Create a new HTTP client.
    pub fn new(config: Config, auth: impl AuthStrategy + 'static) -> Result<Self, HttpError>;
    
    /// Set hooks for observability.
    pub fn with_hooks(self, hooks: impl BasecampHooks + 'static) -> Self;
}
```

## HTTP Methods

```rust
impl HttpClient {
    /// GET request with optional query parameters.
    pub async fn get(
        &self,
        path: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<Response, HttpError>;
    
    /// GET request to an absolute URL (no base URL prefix).
    pub async fn get_absolute(
        &self,
        url: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<Response, HttpError>;
    
    /// POST request with JSON body.
    pub async fn post(
        &self,
        path: &str,
        json_body: Option<&serde_json::Value>,
        operation: Option<&str>,
    ) -> Result<Response, HttpError>;
    
    /// PUT request with JSON body.
    pub async fn put(
        &self,
        path: &str,
        json_body: Option<&serde_json::Value>,
        operation: Option<&str>,
    ) -> Result<Response, HttpError>;
    
    /// DELETE request.
    pub async fn delete(
        &self,
        path: &str,
        operation: Option<&str>,
    ) -> Result<Response, HttpError>;
    
    /// POST request with raw bytes.
    pub async fn post_raw(
        &self,
        path: &str,
        content: &[u8],
        content_type: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<Response, HttpError>;
    
    /// Multipart upload request.
    pub async fn request_multipart(
        &self,
        method: Method,
        path: &str,
        field: &str,
        content: &[u8],
        filename: &str,
        content_type: &str,
    ) -> Result<Response, HttpError>;
    
    /// GET request without retry logic.
    /// Used for operations that should not be retried.
    pub async fn get_no_retry(&self, path: &str) -> Result<Response, HttpError>;
}
```

## URL Construction

```
ALGORITHM build_url:
1. IF path starts with "http://" or "https://":
   RETURN path  // Already absolute
2. ELSE:
   RETURN config.base_url + path
```

- `[unit]` Relative paths are prefixed with base URL
- `[unit]` Absolute URLs are used as-is
- `[conformance]` Base URL is `https://3.basecampapi.com` by default

## Three-Gate Retry Algorithm

Requests are retried based on a three-gate algorithm:

### Gate 1: HTTP Method Default

| Method | Retryable by Default |
|--------|---------------------|
| GET | Yes |
| HEAD | Yes |
| PUT | Yes |
| DELETE | Yes |
| POST | No |

### Gate 2: Idempotency Override

Operations can override the default via metadata:

```rust
/// Operation metadata for retry behavior.
pub struct OperationMetadata {
    /// Whether the operation is idempotent (safe to retry).
    pub idempotent: bool,
}
```

- POST operations are retryable only when `idempotent: true`
- Other methods respect their default unless explicitly overridden

### Gate 3: Error Retryability

| Error Type | Retryable |
|------------|-----------|
| `RateLimitError` (429) | Yes |
| `NetworkError` | Yes |
| `ApiError` (500/502/503/504) | Yes |
| `ApiError` (other) | No |
| `AuthError` (401) | Token refresh only |
| `ForbiddenError` (403) | No |
| `NotFoundError` (404) | No |
| `ValidationError` (400/422) | No |

### Retry Algorithm

```
ALGORITHM should_retry(method, operation, error, attempt):
1. IF attempt >= config.max_retries:
   RETURN false
2. IF NOT error.retryable:
   RETURN false
3. IF method == POST AND operation.idempotent != true:
   RETURN false
4. RETURN true

ALGORITHM calculate_backoff(attempt):
  base_ms = config.base_delay.as_millis()
  jitter_ms = random(0, config.max_jitter.as_millis())
  delay_ms = base_ms * (2 ^ attempt) + jitter_ms
  RETURN Duration::from_millis(delay_ms)
```

- `[unit]` GET requests retry on 5xx errors
- `[unit]` POST requests do not retry by default
- `[unit]` POST requests with idempotent=true retry
- `[unit]` 429 errors respect Retry-After header
- `[unit]` Backoff increases exponentially
- `[unit]` Jitter is added to backoff
- `[conformance]` Max retries defaults to 3

## Token Refresh on 401

```
ALGORITHM handle_401(response, request):
1. IF response.status != 401:
   RETURN response
2. IF token_provider NOT refreshable:
   RETURN AuthError
3. IF refresh_attempted:
   RETURN AuthError  // Prevent refresh loop
4. IF token_provider.refresh():
   Set refresh_attempted = true
   Retry request with new token
5. ELSE:
   RETURN AuthError
```

- `[unit]` 401 triggers single refresh attempt
- `[unit]` Failed refresh returns AuthError
- `[unit]` Successful refresh retries request
- `[conformance]` No infinite refresh loops

## Retry-After Header Handling

```rust
/// Parse Retry-After header value.
fn parse_retry_after(value: &str) -> Option<Duration> {
    // Try parsing as seconds (integer)
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    
    // Try parsing as HTTP date
    if let Ok(datetime) = httpdate::parse_http_date(value) {
        let now = SystemTime::now();
        if let Ok(duration) = datetime.duration_since(now) {
            return Some(duration);
        }
    }
    
    None
}
```

- `[unit]` Numeric Retry-After parsed as seconds
- `[unit]` Date Retry-After parsed correctly
- `[unit]` Invalid Retry-After falls back to backoff
- `[conformance]` Retry-After delays are respected

## Request Hooks Integration

```rust
// Hook call sequence for a request:
hooks.on_request_start(RequestInfo {
    method: "GET",
    url: "https://...",
    attempt: 1,
});

// On retry:
hooks.on_retry(RequestInfo { .. }, attempt, error, delay);

// After response:
hooks.on_request_end(RequestInfo { .. }, RequestResult { .. });
```

## Security Requirements

### HTTPS Enforcement

```
ALGORITHM require_https:
1. parsed = parse_url(url)
2. IF parsed.scheme != "https":
   IF NOT is_localhost(parsed.host):
     RETURN SecurityError::HttpsRequired
```

- `[unit]` HTTP URLs are rejected
- `[unit]` localhost URLs are allowed with HTTP
- `[conformance]` Production API requires HTTPS

### CRLF Injection Prevention

```rust
/// Check for CRLF sequences in multipart filenames.
fn validate_multipart_filename(filename: &str) -> Result<(), HttpError> {
    if filename.contains("\r\n") {
        return Err(HttpError::SecurityViolation {
            reason: "CRLF sequence in filename".to_string(),
        });
    }
    Ok(())
}
```

- `[unit]` Filenames with CRLF are rejected
- `[conformance]` Prevents header injection attacks

## Verification

- `[unit]` All HTTP methods add authentication headers
- `[unit]` JSON bodies are serialized correctly
- `[unit]` Query parameters are URL-encoded
- `[unit]` Timeout is enforced
- `[unit]` Connection pool is reused
- `[conformance]` Retry behavior matches specification
- `[conformance]` Backoff timing is correct
