use std::time::SystemTime;
use thiserror::Error;

pub const MAX_ERROR_MESSAGE_BYTES: usize = 500;
pub const MAX_ERROR_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Usage,
    NotFound,
    AuthRequired,
    Forbidden,
    RateLimit,
    Network,
    Api,
    Ambiguous,
    Validation,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::Usage => "usage",
            ErrorCode::NotFound => "not_found",
            ErrorCode::AuthRequired => "auth_required",
            ErrorCode::Forbidden => "forbidden",
            ErrorCode::RateLimit => "rate_limit",
            ErrorCode::Network => "network",
            ErrorCode::Api => "api_error",
            ErrorCode::Ambiguous => "ambiguous",
            ErrorCode::Validation => "validation",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum BasecampError {
    #[error("Usage error: {message}")]
    Usage {
        message: String,
        hint: Option<String>,
    },

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
    Network { message: String },

    #[error("API error: {message}")]
    Api {
        status: u16,
        message: String,
        request_id: Option<String>,
        retryable: bool,
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

impl BasecampError {
    pub fn code(&self) -> ErrorCode {
        match self {
            BasecampError::Usage { .. } => ErrorCode::Usage,
            BasecampError::NotFound { .. } => ErrorCode::NotFound,
            BasecampError::AuthRequired { .. } => ErrorCode::AuthRequired,
            BasecampError::Forbidden { .. } => ErrorCode::Forbidden,
            BasecampError::RateLimit { .. } => ErrorCode::RateLimit,
            BasecampError::Network { .. } => ErrorCode::Network,
            BasecampError::Api { .. } => ErrorCode::Api,
            BasecampError::Ambiguous { .. } => ErrorCode::Ambiguous,
            BasecampError::Validation { .. } => ErrorCode::Validation,
        }
    }

    pub fn retryable(&self) -> bool {
        match self {
            BasecampError::RateLimit { .. } => true,
            BasecampError::Network { .. } => true,
            BasecampError::Api { retryable, .. } => *retryable,
            _ => false,
        }
    }

    pub fn hint(&self) -> Option<&str> {
        match self {
            BasecampError::Usage { hint, .. } => hint.as_deref(),
            BasecampError::AuthRequired { hint, .. } => hint.as_deref(),
            _ => None,
        }
    }

    pub fn http_status(&self) -> Option<u16> {
        match self {
            BasecampError::NotFound { .. } => Some(404),
            BasecampError::AuthRequired { .. } => Some(401),
            BasecampError::Forbidden { .. } => Some(403),
            BasecampError::RateLimit { .. } => Some(429),
            BasecampError::Api { status, .. } => Some(*status),
            BasecampError::Validation { .. } => Some(422),
            _ => None,
        }
    }

    pub fn request_id(&self) -> Option<&str> {
        match self {
            BasecampError::NotFound { request_id, .. } => request_id.as_deref(),
            BasecampError::AuthRequired { request_id, .. } => request_id.as_deref(),
            BasecampError::Forbidden { request_id, .. } => request_id.as_deref(),
            BasecampError::RateLimit { request_id, .. } => request_id.as_deref(),
            BasecampError::Api { request_id, .. } => request_id.as_deref(),
            BasecampError::Validation { request_id, .. } => request_id.as_deref(),
            _ => None,
        }
    }

    pub fn retry_after(&self) -> Option<u64> {
        match self {
            BasecampError::RateLimit { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            BasecampError::Usage { .. } => 1,
            BasecampError::NotFound { .. } => 2,
            BasecampError::AuthRequired { .. } => 3,
            BasecampError::Forbidden { .. } => 4,
            BasecampError::RateLimit { .. } => 5,
            BasecampError::Network { .. } => 6,
            BasecampError::Api { .. } => 7,
            BasecampError::Ambiguous { .. } => 8,
            BasecampError::Validation { .. } => 9,
        }
    }
}

pub fn truncate(s: &str, max_bytes: usize) -> String {
    let bytes = s.as_bytes();
    if bytes.len() <= max_bytes {
        return s.to_string();
    }

    if max_bytes <= 3 {
        return s.as_bytes()[..max_bytes]
            .to_vec()
            .iter()
            .map(|&c| c as char)
            .collect();
    }

    let target_len = max_bytes - 3;
    let mut end = target_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }

    if end == 0 {
        return "...".to_string();
    }

    format!("{}...", &s[..end])
}

pub fn parse_error_message(body: &str) -> String {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = json.get("error").and_then(|v| v.as_str()) {
            return truncate(msg, MAX_ERROR_MESSAGE_BYTES);
        }
        if let Some(msg) = json.get("message").and_then(|v| v.as_str()) {
            return truncate(msg, MAX_ERROR_MESSAGE_BYTES);
        }
    }

    truncate(body, MAX_ERROR_MESSAGE_BYTES)
}

pub fn parse_field_errors(body: &str) -> Vec<FieldError> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };

    let Some(errors) = json.get("errors").and_then(|v| v.as_object()) else {
        return Vec::new();
    };

    errors
        .iter()
        .flat_map(|(field, messages)| match messages {
            serde_json::Value::String(msg) => vec![FieldError {
                field: field.clone(),
                message: msg.clone(),
            }],
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|msg| FieldError {
                    field: field.clone(),
                    message: msg.to_string(),
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

pub fn get_request_id(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .or_else(|| headers.get("X-Request-Id"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

pub fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let value = headers
        .get("retry-after")
        .or_else(|| headers.get("Retry-After"))?
        .to_str()
        .ok()?;

    if let Ok(seconds) = value.parse::<u64>() {
        return if seconds > 0 { Some(seconds) } else { None };
    }

    if let Ok(datetime) = httpdate::parse_http_date(value) {
        let now = SystemTime::now();
        if let Ok(duration) = datetime.duration_since(now) {
            let secs = duration.as_secs();
            return if secs > 0 { Some(secs) } else { None };
        }
    }

    None
}

pub fn error_from_response(
    status: u16,
    body: &str,
    headers: &reqwest::header::HeaderMap,
) -> BasecampError {
    let request_id = get_request_id(headers);
    let retry_after = parse_retry_after(headers);
    let message = parse_error_message(body);

    match status {
        401 => BasecampError::AuthRequired {
            hint: Some("Check your access token or OAuth credentials".to_string()),
            request_id,
        },
        403 => BasecampError::Forbidden {
            reason: if message.is_empty() {
                Some("Access denied".to_string())
            } else {
                Some(message)
            },
            request_id,
        },
        404 => BasecampError::NotFound {
            resource_type: None,
            resource_id: None,
            request_id,
        },
        429 => BasecampError::RateLimit {
            retry_after,
            request_id,
        },
        400 | 422 => BasecampError::Validation {
            message: if message.is_empty() {
                "Validation failed".to_string()
            } else {
                message
            },
            fields: parse_field_errors(body),
            request_id,
        },
        500 => BasecampError::Api {
            status,
            message: "Server error (500)".to_string(),
            request_id,
            retryable: true,
        },
        502..=504 => BasecampError::Api {
            status,
            message: format!("Gateway error ({})", status),
            request_id,
            retryable: true,
        },
        _ => BasecampError::Api {
            status,
            message: if message.is_empty() {
                format!("Request failed (HTTP {})", status)
            } else {
                message
            },
            request_id,
            retryable: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod error_codes {
        use super::*;

        #[test]
        fn test_error_code_as_str() {
            assert_eq!(ErrorCode::Usage.as_str(), "usage");
            assert_eq!(ErrorCode::NotFound.as_str(), "not_found");
            assert_eq!(ErrorCode::AuthRequired.as_str(), "auth_required");
            assert_eq!(ErrorCode::Forbidden.as_str(), "forbidden");
            assert_eq!(ErrorCode::RateLimit.as_str(), "rate_limit");
            assert_eq!(ErrorCode::Network.as_str(), "network");
            assert_eq!(ErrorCode::Api.as_str(), "api_error");
            assert_eq!(ErrorCode::Ambiguous.as_str(), "ambiguous");
            assert_eq!(ErrorCode::Validation.as_str(), "validation");
        }

        #[test]
        fn test_error_code_display() {
            assert_eq!(format!("{}", ErrorCode::Usage), "usage");
            assert_eq!(format!("{}", ErrorCode::NotFound), "not_found");
        }
    }

    mod error_properties {
        use super::*;

        #[test]
        fn test_usage_error() {
            let err = BasecampError::Usage {
                message: "Invalid input".to_string(),
                hint: Some("Check your parameters".to_string()),
            };
            assert_eq!(err.code(), ErrorCode::Usage);
            assert!(!err.retryable());
            assert_eq!(err.hint(), Some("Check your parameters"));
            assert_eq!(err.http_status(), None);
            assert_eq!(err.exit_code(), 1);
        }

        #[test]
        fn test_not_found_error() {
            let err = BasecampError::NotFound {
                resource_type: Some("Project".to_string()),
                resource_id: Some("123".to_string()),
                request_id: Some("req-abc".to_string()),
            };
            assert_eq!(err.code(), ErrorCode::NotFound);
            assert!(!err.retryable());
            assert_eq!(err.http_status(), Some(404));
            assert_eq!(err.request_id(), Some("req-abc"));
            assert_eq!(err.exit_code(), 2);
        }

        #[test]
        fn test_auth_required_error() {
            let err = BasecampError::AuthRequired {
                hint: Some("Token expired".to_string()),
                request_id: Some("req-def".to_string()),
            };
            assert_eq!(err.code(), ErrorCode::AuthRequired);
            assert!(!err.retryable());
            assert_eq!(err.hint(), Some("Token expired"));
            assert_eq!(err.http_status(), Some(401));
            assert_eq!(err.exit_code(), 3);
        }

        #[test]
        fn test_forbidden_error() {
            let err = BasecampError::Forbidden {
                reason: Some("No access".to_string()),
                request_id: Some("req-ghi".to_string()),
            };
            assert_eq!(err.code(), ErrorCode::Forbidden);
            assert!(!err.retryable());
            assert_eq!(err.http_status(), Some(403));
            assert_eq!(err.exit_code(), 4);
        }

        #[test]
        fn test_rate_limit_error() {
            let err = BasecampError::RateLimit {
                retry_after: Some(60),
                request_id: Some("req-jkl".to_string()),
            };
            assert_eq!(err.code(), ErrorCode::RateLimit);
            assert!(err.retryable());
            assert_eq!(err.retry_after(), Some(60));
            assert_eq!(err.http_status(), Some(429));
            assert_eq!(err.exit_code(), 5);
        }

        #[test]
        fn test_network_error() {
            let err = BasecampError::Network {
                message: "Connection timeout".to_string(),
            };
            assert_eq!(err.code(), ErrorCode::Network);
            assert!(err.retryable());
            assert_eq!(err.http_status(), None);
            assert_eq!(err.exit_code(), 6);
        }

        #[test]
        fn test_api_error_retryable() {
            let err = BasecampError::Api {
                status: 503,
                message: "Service unavailable".to_string(),
                request_id: Some("req-mno".to_string()),
                retryable: true,
            };
            assert_eq!(err.code(), ErrorCode::Api);
            assert!(err.retryable());
            assert_eq!(err.http_status(), Some(503));
            assert_eq!(err.exit_code(), 7);
        }

        #[test]
        fn test_api_error_not_retryable() {
            let err = BasecampError::Api {
                status: 418,
                message: "I'm a teapot".to_string(),
                request_id: None,
                retryable: false,
            };
            assert_eq!(err.code(), ErrorCode::Api);
            assert!(!err.retryable());
            assert_eq!(err.exit_code(), 7);
        }

        #[test]
        fn test_ambiguous_error() {
            let err = BasecampError::Ambiguous {
                message: "Multiple matches".to_string(),
            };
            assert_eq!(err.code(), ErrorCode::Ambiguous);
            assert!(!err.retryable());
            assert_eq!(err.exit_code(), 8);
        }

        #[test]
        fn test_validation_error() {
            let err = BasecampError::Validation {
                message: "Invalid data".to_string(),
                fields: vec![FieldError {
                    field: "name".to_string(),
                    message: "cannot be empty".to_string(),
                }],
                request_id: Some("req-pqr".to_string()),
            };
            assert_eq!(err.code(), ErrorCode::Validation);
            assert!(!err.retryable());
            assert_eq!(err.http_status(), Some(422));
            assert_eq!(err.exit_code(), 9);
        }
    }

    mod truncate {
        use super::*;

        #[test]
        fn test_short_string_unchanged() {
            assert_eq!(truncate("hello", 100), "hello");
        }

        #[test]
        fn test_exact_length_unchanged() {
            assert_eq!(truncate("hello", 5), "hello");
        }

        #[test]
        fn test_truncates_long_string() {
            let result = truncate("hello world", 8);
            // 8 bytes: 5 for content + 3 for "..."
            assert_eq!(result, "hello...");
        }

        #[test]
        fn test_truncation_preserves_utf8() {
            let s = "héllo wörld"; // 11 bytes, but 10 chars
            let result = truncate(s, 7);
            assert!(result.starts_with("hél"));
            assert!(result.ends_with("..."));
        }

        #[test]
        fn test_empty_string() {
            assert_eq!(truncate("", 100), "");
        }

        #[test]
        fn test_very_small_max_bytes() {
            let result = truncate("hello", 2);
            assert_eq!(result, "he");
        }
    }

    mod parse_error_message {
        use super::*;

        #[test]
        fn test_json_error_field() {
            let body = r#"{"error": "Something went wrong"}"#;
            assert_eq!(parse_error_message(body), "Something went wrong");
        }

        #[test]
        fn test_json_message_field() {
            let body = r#"{"message": "Internal error"}"#;
            assert_eq!(parse_error_message(body), "Internal error");
        }

        #[test]
        fn test_error_takes_precedence() {
            let body = r#"{"error": "Error text", "message": "Message text"}"#;
            assert_eq!(parse_error_message(body), "Error text");
        }

        #[test]
        fn test_invalid_json_falls_back_to_raw() {
            let body = "Not JSON";
            assert_eq!(parse_error_message(body), "Not JSON");
        }

        #[test]
        fn test_empty_body() {
            assert_eq!(parse_error_message(""), "");
        }

        #[test]
        fn test_truncates_long_message() {
            let long_msg = "x".repeat(1000);
            let body = format!(r#"{{"error": "{}"}}"#, long_msg);
            let result = parse_error_message(&body);
            assert!(result.len() <= MAX_ERROR_MESSAGE_BYTES + 3); // +3 for "..."
        }
    }

    mod parse_field_errors {
        use super::*;

        #[test]
        fn test_array_errors() {
            let body = r#"{"errors": {"name": ["cannot be empty", "too short"]}}"#;
            let fields = parse_field_errors(body);
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].field, "name");
            assert_eq!(fields[0].message, "cannot be empty");
            assert_eq!(fields[1].message, "too short");
        }

        #[test]
        fn test_string_error() {
            let body = r#"{"errors": {"email": "invalid format"}}"#;
            let fields = parse_field_errors(body);
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].field, "email");
            assert_eq!(fields[0].message, "invalid format");
        }

        #[test]
        fn test_no_errors_field() {
            let body = r#"{"error": "Something went wrong"}"#;
            assert!(parse_field_errors(body).is_empty());
        }

        #[test]
        fn test_invalid_json() {
            assert!(parse_field_errors("not json").is_empty());
        }

        #[test]
        fn test_empty_body() {
            assert!(parse_field_errors("{}").is_empty());
        }
    }

    mod http_status_mapping {
        use super::*;
        use reqwest::header::HeaderMap;

        fn make_headers() -> HeaderMap {
            HeaderMap::new()
        }

        fn make_headers_with_request_id(id: &str) -> HeaderMap {
            let mut headers = HeaderMap::new();
            headers.insert("x-request-id", id.parse().unwrap());
            headers
        }

        fn make_headers_with_retry_after(secs: &str) -> HeaderMap {
            let mut headers = HeaderMap::new();
            headers.insert("retry-after", secs.parse().unwrap());
            headers
        }

        #[test]
        fn test_401_maps_to_auth_required() {
            let headers = make_headers();
            let err = error_from_response(401, "", &headers);
            assert!(matches!(err, BasecampError::AuthRequired { .. }));
            assert_eq!(err.http_status(), Some(401));
            assert!(!err.retryable());
        }

        #[test]
        fn test_403_maps_to_forbidden() {
            let headers = make_headers();
            let err = error_from_response(403, r#"{"error": "No access"}"#, &headers);
            assert!(matches!(err, BasecampError::Forbidden { .. }));
            assert_eq!(err.http_status(), Some(403));
            assert!(!err.retryable());
        }

        #[test]
        fn test_404_maps_to_not_found() {
            let headers = make_headers_with_request_id("req-123");
            let err = error_from_response(404, "", &headers);
            assert!(matches!(err, BasecampError::NotFound { .. }));
            assert_eq!(err.http_status(), Some(404));
            assert_eq!(err.request_id(), Some("req-123"));
            assert!(!err.retryable());
        }

        #[test]
        fn test_429_maps_to_rate_limit() {
            let headers = make_headers_with_retry_after("60");
            let err = error_from_response(429, "", &headers);
            assert!(matches!(err, BasecampError::RateLimit { .. }));
            assert_eq!(err.http_status(), Some(429));
            assert!(err.retryable());
            assert_eq!(err.retry_after(), Some(60));
        }

        #[test]
        fn test_400_maps_to_validation() {
            let headers = make_headers();
            let err = error_from_response(400, r#"{"error": "Bad request"}"#, &headers);
            assert!(matches!(err, BasecampError::Validation { .. }));
            assert!(!err.retryable());
        }

        #[test]
        fn test_422_maps_to_validation() {
            let headers = make_headers();
            let err = error_from_response(422, r#"{"errors": {"name": ["required"]}}"#, &headers);
            assert!(matches!(err, BasecampError::Validation { .. }));
            if let BasecampError::Validation { fields, .. } = err {
                assert_eq!(fields.len(), 1);
            }
        }

        #[test]
        fn test_500_maps_to_api_retryable() {
            let headers = make_headers();
            let err = error_from_response(500, "", &headers);
            assert!(matches!(err, BasecampError::Api { .. }));
            assert_eq!(err.http_status(), Some(500));
            assert!(err.retryable());
        }

        #[test]
        fn test_502_maps_to_api_retryable() {
            let headers = make_headers();
            let err = error_from_response(502, "", &headers);
            assert!(matches!(err, BasecampError::Api { .. }));
            assert!(err.retryable());
        }

        #[test]
        fn test_503_maps_to_api_retryable() {
            let headers = make_headers();
            let err = error_from_response(503, "", &headers);
            assert!(matches!(err, BasecampError::Api { .. }));
            assert!(err.retryable());
        }

        #[test]
        fn test_504_maps_to_api_retryable() {
            let headers = make_headers();
            let err = error_from_response(504, "", &headers);
            assert!(matches!(err, BasecampError::Api { .. }));
            assert!(err.retryable());
        }

        #[test]
        fn test_other_status_maps_to_api_not_retryable() {
            let headers = make_headers();
            let err = error_from_response(418, r#"{"error": "I'm a teapot"}"#, &headers);
            assert!(matches!(err, BasecampError::Api { .. }));
            assert!(!err.retryable());
        }
    }

    mod retry_after_parsing {
        use super::*;
        use reqwest::header::HeaderMap;

        fn make_headers_with_retry_after(value: &str) -> HeaderMap {
            let mut headers = HeaderMap::new();
            headers.insert("retry-after", value.parse().unwrap());
            headers
        }

        #[test]
        fn test_numeric_seconds() {
            let headers = make_headers_with_retry_after("60");
            assert_eq!(parse_retry_after(&headers), Some(60));
        }

        #[test]
        fn test_zero_returns_none() {
            let headers = make_headers_with_retry_after("0");
            assert_eq!(parse_retry_after(&headers), None);
        }

        #[test]
        fn test_missing_header_returns_none() {
            let headers = HeaderMap::new();
            assert_eq!(parse_retry_after(&headers), None);
        }

        #[test]
        fn test_http_date_in_past_returns_none() {
            let headers = make_headers_with_retry_after("Wed, 09 Jun 2021 10:18:14 GMT");
            assert_eq!(parse_retry_after(&headers), None);
        }

        #[test]
        fn test_invalid_format_returns_none() {
            let headers = make_headers_with_retry_after("invalid");
            assert_eq!(parse_retry_after(&headers), None);
        }
    }

    mod request_id_extraction {
        use super::*;
        use reqwest::header::HeaderMap;

        #[test]
        fn test_extracts_x_request_id() {
            let mut headers = HeaderMap::new();
            headers.insert("x-request-id", "req-abc-123".parse().unwrap());
            assert_eq!(get_request_id(&headers), Some("req-abc-123".to_string()));
        }

        #[test]
        fn test_extracts_capitalized_header() {
            let mut headers = HeaderMap::new();
            headers.insert("X-Request-Id", "req-def-456".parse().unwrap());
            assert_eq!(get_request_id(&headers), Some("req-def-456".to_string()));
        }

        #[test]
        fn test_missing_header_returns_none() {
            let headers = HeaderMap::new();
            assert_eq!(get_request_id(&headers), None);
        }
    }
}
