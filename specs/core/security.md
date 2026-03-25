# Security Specification

## Overview

The security module provides utilities for enforcing security requirements including
HTTPS enforcement, body size limits, header redaction, and URL validation.

## Constants

```rust
/// Maximum size for error messages (500 bytes).
pub const MAX_ERROR_MESSAGE_BYTES: usize = 500;

/// Maximum size for response bodies (50 MB).
pub const MAX_RESPONSE_BODY_BYTES: usize = 50 * 1024 * 1024;

/// Maximum size for error response bodies (1 MB).
pub const MAX_ERROR_BODY_BYTES: usize = 1024 * 1024;

/// Headers that contain sensitive information.
pub const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "set-cookie",
    "x-csrf-token",
];
```

## HTTPS Enforcement

```rust
/// Require HTTPS for a URL.
/// 
/// # Errors
/// Returns `SecurityError::HttpsRequired` if the URL is not HTTPS.
/// 
/// # Exceptions
/// Localhost URLs are exempt from this requirement.
pub fn require_https(url: &str) -> Result<(), SecurityError> {
    let parsed = url::Url::parse(url)
        .map_err(|e| SecurityError::InvalidUrl { 
            url: url.to_string(), 
            reason: e.to_string() 
        })?;
    
    if parsed.scheme() != "https" {
        if !is_localhost(parsed.host_str().unwrap_or("")) {
            return Err(SecurityError::HttpsRequired {
                url: url.to_string(),
            });
        }
    }
    
    Ok(())
}

/// Check if a host is localhost.
pub fn is_localhost(host: &str) -> bool {
    matches!(
        host.to_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1" | "0.0.0.0"
    ) || host.starts_with("127.")
}

/// Check if a URL is localhost.
pub fn is_localhost_url(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else { return false };
    is_localhost(parsed.host_str().unwrap_or(""))
}
```

- `[unit]` HTTPS URLs pass
- `[unit]` HTTP URLs fail
- `[unit]` Localhost HTTP URLs pass
- `[unit]` 127.x.x.x addresses are localhost
- `[conformance]` Production API requires HTTPS

## Body Size Limits

```rust
/// Check if a body exceeds the size limit.
/// 
/// # Errors
/// Returns `SecurityError::BodyTooLarge` if the body exceeds max_bytes.
pub fn check_body_size(body: &[u8], max_bytes: usize, label: &str) -> Result<(), SecurityError> {
    if body.len() > max_bytes {
        return Err(SecurityError::BodyTooLarge {
            actual: body.len(),
            max: max_bytes,
            label: label.to_string(),
        });
    }
    Ok(())
}

/// Check response body size during streaming.
pub struct BodySizeLimiter<R> {
    inner: R,
    read: u64,
    max_bytes: u64,
}

impl<R: AsyncRead + Unpin> BodySizeLimiter<R> {
    pub fn new(inner: R, max_bytes: u64) -> Self;
}

impl<R: AsyncRead + Unpin> AsyncRead for BodySizeLimiter<R> {
    // Reads from inner, tracks bytes read, errors if limit exceeded
}
```

- `[unit]` Bodies under limit pass
- `[unit]` Bodies over limit fail
- `[unit]` Streaming limiter enforces limit
- `[conformance]` Response body limited to 50 MB
- `[conformance]` Error body limited to 1 MB

## Header Redaction

```rust
/// Redact sensitive headers for logging.
/// 
/// Returns a new header map with sensitive values replaced with "[REDACTED]".
pub fn redact_headers(headers: &HeaderMap) -> HeaderMap {
    let mut redacted = HeaderMap::new();
    
    for (name, value) in headers.iter() {
        if is_sensitive_header(name.as_str()) {
            redacted.insert(name, "[REDACTED]".parse().unwrap());
        } else {
            redacted.insert(name, value.clone());
        }
    }
    
    redacted
}

/// Check if a header name is sensitive.
pub fn is_sensitive_header(name: &str) -> bool {
    SENSITIVE_HEADERS.contains(&name.to_lowercase().as_str())
}
```

- `[unit]` Authorization header is redacted
- `[unit]` Cookie header is redacted
- `[unit]` Set-Cookie header is redacted
- `[unit]` X-CSRF-Token header is redacted
- `[unit]` Other headers are preserved
- `[manual]` Logs do not contain sensitive values

## String Truncation

```rust
/// Truncate a string to a maximum byte length.
/// 
/// Preserves UTF-8 validity by only truncating at character boundaries.
pub fn truncate(s: &str, max_bytes: usize) -> String {
    let bytes = s.as_bytes();
    
    if bytes.len() <= max_bytes {
        return s.to_string();
    }
    
    // Find the last valid UTF-8 boundary before max_bytes
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    
    // Add ellipsis if truncated
    if end < bytes.len() {
        format!("{}...", &s[..end])
    } else {
        s.to_string()
    }
}

/// Truncate with custom suffix.
pub fn truncate_with(s: &str, max_bytes: usize, suffix: &str) -> String {
    // Similar but uses custom suffix
}
```

- `[unit]` Short strings unchanged
- `[unit]` Long strings truncated
- `[unit]` UTF-8 validity preserved
- `[unit]` Ellipsis added when truncated
- `[conformance]` Error messages truncated to 500 bytes

## URL Resolution and Validation

```rust
/// Resolve a target URL against a base URL.
/// 
/// # Security
/// Validates that the resolved URL is same-origin as the base.
pub fn resolve_url(base: &str, target: &str) -> Result<String, SecurityError> {
    let base_url = url::Url::parse(base)
        .map_err(|e| SecurityError::InvalidUrl {
            url: base.to_string(),
            reason: e.to_string(),
        })?;
    
    let resolved = base_url.join(target)
        .map_err(|e| SecurityError::InvalidUrl {
            url: target.to_string(),
            reason: e.to_string(),
        })?;
    
    // Validate same origin
    if !same_origin(base, resolved.as_str()) {
        return Err(SecurityError::CrossOriginRedirect {
            from: base.to_string(),
            to: resolved.to_string(),
        });
    }
    
    Ok(resolved.to_string())
}

/// Check if two URLs have the same origin.
pub fn same_origin(a: &str, b: &str) -> bool {
    let Ok(url_a) = url::Url::parse(a) else { return false };
    let Ok(url_b) = url::Url::parse(b) else { return false };
    
    url_a.scheme() == url_b.scheme()
        && url_a.host() == url_b.host()
        && url_a.port_or_known_default() == url_b.port_or_known_default()
}
```

- `[unit]` Relative URLs resolve correctly
- `[unit]` Absolute URLs pass through
- `[unit]` Same-origin URLs pass
- `[unit]` Cross-origin URLs fail
- `[conformance]` Prevents open redirect attacks

## CRLF Injection Prevention

```rust
/// Check for CRLF sequences that could enable header injection.
pub fn contains_crlf(s: &str) -> bool {
    s.contains("\r\n") || s.contains("\n\r")
}

/// Validate a string is safe for use in headers.
pub fn validate_header_safe(s: &str, label: &str) -> Result<(), SecurityError> {
    if contains_crlf(s) {
        return Err(SecurityError::HeaderInjection {
            label: label.to_string(),
        });
    }
    
    // Check for null bytes
    if s.contains('\0') {
        return Err(SecurityError::NullByte {
            label: label.to_string(),
        });
    }
    
    Ok(())
}

/// Validate a filename for multipart uploads.
pub fn validate_filename(filename: &str) -> Result<(), SecurityError> {
    validate_header_safe(filename, "filename")?;
    
    // Additional checks for filenames
    if filename.is_empty() {
        return Err(SecurityError::EmptyFilename);
    }
    
    if filename.len() > 255 {
        return Err(SecurityError::FilenameTooLong {
            length: filename.len(),
        });
    }
    
    Ok(())
}
```

- `[unit]` CRLF sequences detected
- `[unit]` Null bytes detected
- `[unit]` Empty filenames rejected
- `[unit]` Long filenames rejected
- `[conformance]` Prevents header injection

## Security Error Type

```rust
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("HTTPS required for URL: {url}")]
    HttpsRequired { url: String },
    
    #[error("Invalid URL '{url}': {reason}")]
    InvalidUrl { url: String, reason: String },
    
    #[error("{label} body too large: {actual} bytes (max: {max})")]
    BodyTooLarge { actual: usize, max: usize, label: String },
    
    #[error("Cross-origin redirect from {from} to {to}")]
    CrossOriginRedirect { from: String, to: String },
    
    #[error("Potential header injection in {label}")]
    HeaderInjection { label: String },
    
    #[error("Null byte in {label}")]
    NullByte { label: String },
    
    #[error("Filename cannot be empty")]
    EmptyFilename,
    
    #[error("Filename too long: {length} bytes (max: 255)")]
    FilenameTooLong { length: usize },
}
```

## Verification

- `[unit]` HTTPS enforcement works correctly
- `[unit]` Localhost exemption works
- `[unit]` Body size limits enforced
- `[unit]` Header redaction complete
- `[unit]` Truncation preserves UTF-8
- `[unit]` Same-origin validation works
- `[unit]` CRLF injection prevented
- `[conformance]` All security constants match specification
- `[manual]` No sensitive data in logs
