use url::Url;

pub const MAX_RESPONSE_BODY_BYTES: usize = 50 * 1024 * 1024;
pub const MAX_ERROR_BODY_BYTES: usize = 1024 * 1024;
pub const MAX_ERROR_MESSAGE_BYTES: usize = 500;

pub const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "set-cookie",
    "x-api-key",
    "x-auth-token",
];

pub fn require_https(url: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;

    if parsed.scheme() != "https" && !is_localhost(parsed.host_str().unwrap_or("")) {
        return Err(format!("HTTPS required for production API. URL: {}", url));
    }

    Ok(())
}

pub fn is_localhost(host: &str) -> bool {
    let host = host.to_lowercase();

    if host == "localhost" {
        return true;
    }

    if host == "127.0.0.1" || host == "::1" {
        return true;
    }

    if host.starts_with("127.") {
        return true;
    }

    if host.ends_with(".localhost") {
        return true;
    }

    if host.starts_with("[::1]") {
        return true;
    }

    false
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

pub fn same_origin(url1: &str, url2: &str) -> bool {
    let parse_origin = |url: &str| -> Option<String> {
        let parsed = Url::parse(url).ok()?;
        Some(format!(
            "{}://{}{}",
            parsed.scheme(),
            parsed.host_str()?,
            parsed.port().map(|p| format!(":{}", p)).unwrap_or_default()
        ))
    };

    let origin1 = parse_origin(url1);
    let origin2 = parse_origin(url2);

    match (origin1, origin2) {
        (Some(o1), Some(o2)) => o1.to_lowercase() == o2.to_lowercase(),
        _ => false,
    }
}

pub fn check_body_size(size: usize, max_bytes: usize) -> Result<(), String> {
    if size > max_bytes {
        return Err(format!(
            "Response body too large: {} bytes (max: {})",
            size, max_bytes
        ));
    }
    Ok(())
}

pub fn redact_headers(
    headers: &reqwest::header::HeaderMap,
) -> std::collections::HashMap<String, String> {
    let mut result = std::collections::HashMap::new();

    for (name, value) in headers.iter() {
        let name_str = name.as_str().to_lowercase();
        if SENSITIVE_HEADERS.contains(&name_str.as_str()) {
            result.insert(name_str, "[REDACTED]".to_string());
        } else if let Ok(v) = value.to_str() {
            result.insert(name_str, v.to_string());
        }
    }

    result
}

pub fn contains_crlf(s: &str) -> bool {
    s.contains("\r\n") || s.contains("\r") || s.contains("\n")
}

pub fn validate_header_value(value: &str) -> Result<(), String> {
    if contains_crlf(value) {
        return Err("Header value contains CRLF sequence".to_string());
    }
    Ok(())
}

pub fn validate_url_for_redirect(url: &str, base_url: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;

    if parsed.scheme() != "https" && !is_localhost(parsed.host_str().unwrap_or("")) {
        return Err("Protocol downgrade not allowed".to_string());
    }

    if !same_origin(url, base_url) {
        return Err("Redirect to different origin not allowed".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderMap;

    mod require_https {
        use super::*;

        #[test]
        fn test_https_url_accepted() {
            assert!(require_https("https://api.example.com/path").is_ok());
        }

        #[test]
        fn test_http_url_rejected() {
            assert!(require_https("http://api.example.com/path").is_err());
        }

        #[test]
        fn test_localhost_http_accepted() {
            assert!(require_https("http://localhost/path").is_ok());
        }

        #[test]
        fn test_127_0_0_1_http_accepted() {
            assert!(require_https("http://127.0.0.1/path").is_ok());
        }

        #[test]
        fn test_127_subnet_http_accepted() {
            assert!(require_https("http://127.0.0.5:8080/path").is_ok());
        }

        #[test]
        fn test_subdomain_localhost_http_accepted() {
            assert!(require_https("http://api.localhost/path").is_ok());
            assert!(require_https("http://test.localhost:3000/path").is_ok());
        }

        #[test]
        fn test_ipv6_localhost_accepted() {
            assert!(require_https("http://[::1]/path").is_ok());
        }

        #[test]
        fn test_invalid_url_rejected() {
            assert!(require_https("not-a-url").is_err());
        }
    }

    mod is_localhost {
        use super::*;

        #[test]
        fn test_localhost() {
            assert!(is_localhost("localhost"));
            assert!(is_localhost("LOCALHOST"));
            assert!(is_localhost("LocalHost"));
        }

        #[test]
        fn test_127_0_0_1() {
            assert!(is_localhost("127.0.0.1"));
        }

        #[test]
        fn test_127_subnet() {
            assert!(is_localhost("127.0.0.5"));
            assert!(is_localhost("127.255.255.255"));
        }

        #[test]
        fn test_ipv6_loopback() {
            assert!(is_localhost("::1"));
        }

        #[test]
        fn test_subdomain_localhost() {
            assert!(is_localhost("api.localhost"));
            assert!(is_localhost("test.localhost"));
        }

        #[test]
        fn test_not_localhost() {
            assert!(!is_localhost("example.com"));
            assert!(!is_localhost("192.168.1.1"));
            assert!(!is_localhost("10.0.0.1"));
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
            assert_eq!(result, "hello...");
        }

        #[test]
        fn test_preserves_utf8_boundary() {
            let s = "héllo wörld";
            let result = truncate(s, 7);
            assert!(result.starts_with("hél"));
            assert!(result.ends_with("..."));
            assert!(result.as_bytes().len() <= 7);
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

        #[test]
        fn test_multibyte_char_at_boundary() {
            let s = "日本語テスト";
            let result = truncate(s, 10);
            assert!(result.as_bytes().len() <= 10);
            assert!(result.ends_with("...") || result.len() < 10);
        }
    }

    mod same_origin {
        use super::*;

        #[test]
        fn test_same_origin_exact_match() {
            assert!(same_origin(
                "https://api.example.com/path1",
                "https://api.example.com/path2"
            ));
        }

        #[test]
        fn test_same_origin_with_port() {
            assert!(same_origin(
                "https://api.example.com:8080/a",
                "https://api.example.com:8080/b"
            ));
        }

        #[test]
        fn test_different_scheme() {
            assert!(!same_origin(
                "https://api.example.com/a",
                "http://api.example.com/b"
            ));
        }

        #[test]
        fn test_different_host() {
            assert!(!same_origin(
                "https://api1.example.com/a",
                "https://api2.example.com/b"
            ));
        }

        #[test]
        fn test_different_port() {
            assert!(!same_origin(
                "https://api.example.com:8080/a",
                "https://api.example.com:9090/b"
            ));
        }

        #[test]
        fn test_case_insensitive() {
            assert!(same_origin(
                "https://API.Example.COM/a",
                "https://api.example.com/b"
            ));
        }

        #[test]
        fn test_invalid_url() {
            assert!(!same_origin("not-a-url", "https://api.example.com/b"));
            assert!(!same_origin("https://api.example.com/a", "not-a-url"));
        }
    }

    mod check_body_size {
        use super::*;

        #[test]
        fn test_within_limit() {
            assert!(check_body_size(100, 1000).is_ok());
        }

        #[test]
        fn test_exact_limit() {
            assert!(check_body_size(1000, 1000).is_ok());
        }

        #[test]
        fn test_exceeds_limit() {
            assert!(check_body_size(1001, 1000).is_err());
        }

        #[test]
        fn test_default_max_response_body() {
            assert!(check_body_size(MAX_RESPONSE_BODY_BYTES, MAX_RESPONSE_BODY_BYTES).is_ok());
            assert!(check_body_size(MAX_RESPONSE_BODY_BYTES + 1, MAX_RESPONSE_BODY_BYTES).is_err());
        }
    }

    mod redact_headers {
        use super::*;

        fn make_header_map(headers: Vec<(&str, &str)>) -> HeaderMap {
            use reqwest::header::HeaderName;
            let mut map = HeaderMap::new();
            for (name, value) in headers {
                let header_name: HeaderName = name.parse().unwrap();
                map.insert(header_name, value.parse().unwrap());
            }
            map
        }

        #[test]
        fn test_redacts_authorization() {
            let headers = make_header_map(vec![
                ("authorization", "Bearer secret-token"),
                ("content-type", "application/json"),
            ]);
            let redacted = redact_headers(&headers);
            assert_eq!(
                redacted.get("authorization"),
                Some(&"[REDACTED]".to_string())
            );
            assert_eq!(
                redacted.get("content-type"),
                Some(&"application/json".to_string())
            );
        }

        #[test]
        fn test_redacts_cookie() {
            let headers = make_header_map(vec![("cookie", "session=abc123")]);
            let redacted = redact_headers(&headers);
            assert_eq!(redacted.get("cookie"), Some(&"[REDACTED]".to_string()));
        }

        #[test]
        fn test_redacts_set_cookie() {
            let headers = make_header_map(vec![("set-cookie", "session=abc123; Path=/")]);
            let redacted = redact_headers(&headers);
            assert_eq!(redacted.get("set-cookie"), Some(&"[REDACTED]".to_string()));
        }

        #[test]
        fn test_redacts_x_api_key() {
            let headers = make_header_map(vec![("x-api-key", "my-secret-key")]);
            let redacted = redact_headers(&headers);
            assert_eq!(redacted.get("x-api-key"), Some(&"[REDACTED]".to_string()));
        }

        #[test]
        fn test_redacts_x_auth_token() {
            let headers = make_header_map(vec![("x-auth-token", "token123")]);
            let redacted = redact_headers(&headers);
            assert_eq!(
                redacted.get("x-auth-token"),
                Some(&"[REDACTED]".to_string())
            );
        }

        #[test]
        fn test_preserves_safe_headers() {
            let headers = make_header_map(vec![
                ("content-type", "application/json"),
                ("x-request-id", "req-123"),
            ]);
            let redacted = redact_headers(&headers);
            assert_eq!(
                redacted.get("content-type"),
                Some(&"application/json".to_string())
            );
            assert_eq!(redacted.get("x-request-id"), Some(&"req-123".to_string()));
        }

        #[test]
        fn test_empty_headers() {
            let headers = HeaderMap::new();
            let redacted = redact_headers(&headers);
            assert!(redacted.is_empty());
        }
    }

    mod contains_crlf {
        use super::*;

        #[test]
        fn test_no_crlf() {
            assert!(!contains_crlf("normal text"));
        }

        #[test]
        fn test_contains_crlf() {
            assert!(contains_crlf("text\r\nmore"));
        }

        #[test]
        fn test_contains_cr_only() {
            assert!(contains_crlf("text\rmore"));
        }

        #[test]
        fn test_contains_lf_only() {
            assert!(contains_crlf("text\nmore"));
        }

        #[test]
        fn test_empty_string() {
            assert!(!contains_crlf(""));
        }

        #[test]
        fn test_multiple_crlf() {
            assert!(contains_crlf("a\r\nb\r\nc"));
        }
    }

    mod validate_header_value {
        use super::*;

        #[test]
        fn test_valid_value() {
            assert!(validate_header_value("normal-value").is_ok());
        }

        #[test]
        fn test_rejects_crlf() {
            assert!(validate_header_value("value\r\nInjected: header").is_err());
        }

        #[test]
        fn test_rejects_cr() {
            assert!(validate_header_value("value\rmore").is_err());
        }

        #[test]
        fn test_rejects_lf() {
            assert!(validate_header_value("value\nmore").is_err());
        }
    }

    mod validate_url_for_redirect {
        use super::*;

        #[test]
        fn test_same_origin_https_accepted() {
            assert!(validate_url_for_redirect(
                "https://api.example.com/page2",
                "https://api.example.com/page1"
            )
            .is_ok());
        }

        #[test]
        fn test_protocol_downgrade_rejected() {
            assert!(validate_url_for_redirect(
                "http://api.example.com/page2",
                "https://api.example.com/page1"
            )
            .is_err());
        }

        #[test]
        fn test_different_origin_rejected() {
            assert!(validate_url_for_redirect(
                "https://evil.com/page",
                "https://api.example.com/page1"
            )
            .is_err());
        }

        #[test]
        fn test_localhost_http_accepted() {
            assert!(
                validate_url_for_redirect("http://localhost/page2", "http://localhost/page1")
                    .is_ok()
            );
        }

        #[test]
        fn test_invalid_url_rejected() {
            assert!(
                validate_url_for_redirect("not-a-url", "https://api.example.com/page1").is_err()
            );
        }
    }

    mod constants {
        use super::*;

        #[test]
        fn test_max_response_body_bytes() {
            assert_eq!(MAX_RESPONSE_BODY_BYTES, 50 * 1024 * 1024);
        }

        #[test]
        fn test_max_error_body_bytes() {
            assert_eq!(MAX_ERROR_BODY_BYTES, 1024 * 1024);
        }

        #[test]
        fn test_max_error_message_bytes() {
            assert_eq!(MAX_ERROR_MESSAGE_BYTES, 500);
        }
    }
}
