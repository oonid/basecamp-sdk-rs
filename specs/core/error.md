# Error Specification

## Overview

The error module defines a comprehensive error taxonomy with codes, HTTP status mapping,
retryability flags, and user-friendly hints.

## Error Taxonomy

```rust
use thiserror::Error;

/// SDK error type.
#[derive(Debug, Error)]
pub enum BasecampError {
    #[error("Usage error: {message}")]
    Usage { message: String, hint: Option<String> },
    
    #[error("Resource not found")]
    NotFound { 
        resource_type: Option<String>,
        resource_id: Option<String>,
        request_id: Option<String>,
    },
    
    #[error("Authentication required")]
    AuthRequired {
        hint: Option<String>,
        request_id: Option<String>,
    },
    
    #[error("Access forbidden")]
    Forbidden {
        reason: Option<String>,
        request_id: Option<String>,
    },
    
    #[error("Rate limit exceeded")]
    RateLimit {
        retry_after: Option<u64>,
        request_id: Option<String>,
    },
    
    #[error("Network error: {message}")]
    Network { 
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("API error")]
    Api {
        status: u16,
        message: String,
        request_id: Option<String>,
    },
    
    #[error("Ambiguous request: {message}")]
    Ambiguous { message: String },
    
    #[error("Validation error: {message}")]
    Validation {
        message: String,
        fields: Vec<FieldError>,
        request_id: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}
```

## Error Codes

```rust
/// Error codes for programmatic handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::IntoStaticStr)]
pub enum ErrorCode {
    Usage = "usage",
    NotFound = "not_found",
    AuthRequired = "auth_required",
    Forbidden = "forbidden",
    RateLimit = "rate_limit",
    Network = "network",
    Api = "api_error",
    Ambiguous = "ambiguous",
    Validation = "validation",
}

impl BasecampError {
    /// Get the error code.
    pub fn code(&self) -> ErrorCode;
    
    /// Whether this error is retryable.
    pub fn retryable(&self) -> bool;
    
    /// Get a user-friendly hint.
    pub fn hint(&self) -> Option<&str>;
    
    /// Get the HTTP status code if applicable.
    pub fn http_status(&self) -> Option<u16>;
    
    /// Get the request ID if available.
    pub fn request_id(&self) -> Option<&str>;
    
    /// Get the retry-after delay in seconds if applicable.
    pub fn retry_after(&self) -> Option<u64>;
    
    /// Get the CLI exit code.
    pub fn exit_code(&self) -> i32;
}
```

## Error Code Mapping

| ErrorCode | HTTP Status | Exit Code | Retryable |
|-----------|-------------|-----------|-----------|
| `usage` | - | 1 | No |
| `not_found` | 404 | 2 | No |
| `auth_required` | 401 | 3 | No* |
| `forbidden` | 403 | 4 | No |
| `rate_limit` | 429 | 5 | Yes |
| `network` | - | 6 | Yes |
| `api_error` | 500/502/503/504 | 7 | Yes** |
| `ambiguous` | - | 8 | No |
| `validation` | 400/422 | 9 | No |

* Auth errors trigger token refresh, not standard retry
** Only 500/502/503/504 are retryable

## HTTP Status to Error Mapping

```rust
/// Map HTTP status to error type.
pub fn map_status_to_error(status: StatusCode, body: &str, headers: &HeaderMap) -> BasecampError {
    match status {
        400 => BasecampError::Validation {
            message: parse_error_message(body),
            fields: parse_field_errors(body),
            request_id: get_request_id(headers),
        },
        401 => BasecampError::AuthRequired {
            hint: Some("Check your access token or OAuth credentials".to_string()),
            request_id: get_request_id(headers),
        },
        403 => BasecampError::Forbidden {
            reason: parse_error_message(body).into(),
            request_id: get_request_id(headers),
        },
        404 => BasecampError::NotFound {
            resource_type: None,
            resource_id: None,
            request_id: get_request_id(headers),
        },
        422 => BasecampError::Validation {
            message: parse_error_message(body),
            fields: parse_field_errors(body),
            request_id: get_request_id(headers),
        },
        429 => BasecampError::RateLimit {
            retry_after: parse_retry_after(headers),
            request_id: get_request_id(headers),
        },
        500 | 502 | 503 | 504 => BasecampError::Api {
            status: status.as_u16(),
            message: parse_error_message(body),
            request_id: get_request_id(headers),
        },
        _ => BasecampError::Api {
            status: status.as_u16(),
            message: parse_error_message(body),
            request_id: get_request_id(headers),
        },
    }
}
```

- `[unit]` 400 → Validation
- `[unit]` 401 → AuthRequired
- `[unit]` 403 → Forbidden
- `[unit]` 404 → NotFound
- `[unit]` 422 → Validation
- `[unit]` 429 → RateLimit
- `[unit]` 500/502/503/504 → Api (retryable)
- `[unit]` Other 5xx → Api (not retryable)
- `[conformance]` Status codes map correctly per specification

## Error Message Parsing

```rust
const MAX_ERROR_MESSAGE_BYTES: usize = 500;
const MAX_ERROR_BODY_BYTES: usize = 1024 * 1024; // 1 MB

/// Parse error message from response body.
/// Truncates to MAX_ERROR_MESSAGE_BYTES.
fn parse_error_message(body: &str) -> String {
    // Try JSON error format first
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = json.get("error").and_then(|v| v.as_str()) {
            return truncate(msg, MAX_ERROR_MESSAGE_BYTES);
        }
        if let Some(msg) = json.get("message").and_then(|v| v.as_str()) {
            return truncate(msg, MAX_ERROR_MESSAGE_BYTES);
        }
    }
    
    // Fallback to raw body (truncated)
    truncate(body, MAX_ERROR_MESSAGE_BYTES)
}

/// Truncate string to max bytes, preserving UTF-8 validity.
fn truncate(s: &str, max_bytes: usize) -> String {
    let bytes = s.as_bytes();
    if bytes.len() <= max_bytes {
        return s.to_string();
    }
    
    // Find valid UTF-8 boundary
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    
    format!("{}...", &s[..end])
}
```

- `[unit]` JSON error field is extracted
- `[unit]` JSON message field is fallback
- `[unit]` Raw body is fallback
- `[unit]` Message is truncated to 500 bytes
- `[unit]` Truncation preserves UTF-8 validity
- `[conformance]` Body size limited to 1 MB

## Field Error Parsing

```rust
/// Parse field-level validation errors.
fn parse_field_errors(body: &str) -> Vec<FieldError> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    
    let Some(errors) = json.get("errors").and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    
    errors.iter()
        .flat_map(|(field, messages)| {
            match messages {
                serde_json::Value::String(msg) => {
                    vec![FieldError {
                        field: field.clone(),
                        message: msg.clone(),
                    }]
                }
                serde_json::Value::Array(arr) => {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|msg| FieldError {
                            field: field.clone(),
                            message: msg.to_string(),
                        })
                        .collect()
                }
                _ => Vec::new(),
            }
        })
        .collect()
}
```

- `[unit]` Parses `{ "errors": { "field": ["msg1", "msg2"] } }`
- `[unit]` Parses `{ "errors": { "field": "message" } }`
- `[unit]` Returns empty vec on parse failure

## Request ID Extraction

```rust
/// Extract request ID from response headers.
fn get_request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("X-Request-Id")
        .or_else(|| headers.get("X-Request-ID"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}
```

- `[unit]` Extracts X-Request-Id header
- `[unit]` Returns None if header missing

## Retry-After Parsing

```rust
/// Parse Retry-After header value.
fn parse_retry_after(headers: &HeaderMap) -> Option<u64> {
    let value = headers.get("Retry-After")?.to_str().ok()?;
    
    // Try seconds format
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds);
    }
    
    // Try HTTP date format
    if let Ok(datetime) = httpdate::parse_http_date(value) {
        let now = SystemTime::now();
        if let Ok(duration) = datetime.duration_since(now) {
            return Some(duration.as_secs());
        }
    }
    
    None
}
```

- `[unit]` Parses numeric seconds
- `[unit]` Parses HTTP date format
- `[unit]` Returns None on invalid format

## Verification

- `[unit]` All error codes have correct exit codes
- `[unit]` `retryable()` returns correct values
- `[unit]` `hint()` returns appropriate guidance
- `[unit]` Message truncation works correctly
- `[unit]` Field errors are parsed correctly
- `[conformance]` HTTP status codes map correctly
- `[conformance]` Request IDs are preserved
