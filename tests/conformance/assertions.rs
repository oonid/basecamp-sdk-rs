use basecamp_sdk_rs::BasecampError;

use super::types::{Assertion, OperationResult, RequestTracker, TestCase, TestResult};

pub fn check_assertions(
    tc: &TestCase,
    tracker: &RequestTracker,
    result: &OperationResult,
) -> Option<TestResult> {
    let has_link_next = tc.mock_responses.iter().any(|mr| {
        mr.headers
            .get("Link")
            .map(|l| l.contains("rel=\"next\""))
            .unwrap_or(false)
    });

    for assertion in &tc.assertions {
        if let Some(failure) = check_single_assertion(tc, assertion, tracker, result, has_link_next)
        {
            return Some(failure);
        }
    }
    None
}

fn check_single_assertion(
    tc: &TestCase,
    assertion: &Assertion,
    tracker: &RequestTracker,
    result: &OperationResult,
    has_link_next_header: bool,
) -> Option<TestResult> {
    match assertion.assertion_type.as_str() {
        "requestCount" => check_request_count(tc, assertion, tracker, has_link_next_header),
        "delayBetweenRequests" => check_delay_between_requests(tc, assertion, tracker),
        "noError" => check_no_error(tc, result),
        "statusCode" => check_status_code(tc, assertion, result),
        "errorCode" => check_error_code(tc, assertion, result),
        "errorType" => check_error_type(tc, assertion, result),
        "errorMessage" => check_error_message(tc, assertion, result),
        "errorField" => check_error_field(tc, assertion, result),
        "requestPath" => check_request_path(tc, assertion, tracker),
        "headerInjected" => check_header_injected(tc, assertion, tracker),
        "headerPresent" => check_header_present(tc, assertion, tracker),
        "headerValue" => check_header_value(tc, assertion),
        "responseBody" => check_response_body(tc, assertion, result),
        "responseMeta" => check_response_meta(tc, assertion, result),
        "responseStatus" => check_response_status(tc, assertion, result),
        "requestScheme" => check_request_scheme(tc, assertion, result),
        "urlOrigin" => check_url_origin(tc, assertion, tracker),
        _ => Some(TestResult::fail(
            &tc.name,
            &format!("Unknown assertion type: {}", assertion.assertion_type),
        )),
    }
}

fn check_request_count(
    tc: &TestCase,
    assertion: &Assertion,
    tracker: &RequestTracker,
    has_link_next_header: bool,
) -> Option<TestResult> {
    let expected = assertion
        .expected
        .as_ref()
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    if has_link_next_header {
        if tracker.count < expected {
            return Some(TestResult::fail(
                &tc.name,
                &format!(
                    "Expected >= {} requests (SDK auto-paginates), got {}",
                    expected, tracker.count
                ),
            ));
        }
    } else if tracker.count != expected {
        return Some(TestResult::fail(
            &tc.name,
            &format!("Expected {} requests, got {}", expected, tracker.count),
        ));
    }
    None
}

fn check_delay_between_requests(
    tc: &TestCase,
    assertion: &Assertion,
    tracker: &RequestTracker,
) -> Option<TestResult> {
    let min_delay = assertion.min.unwrap_or(0.0) as u64;
    let delays = tracker.delays_between_requests_ms();

    if !delays.is_empty() && delays.iter().any(|&d| d < min_delay) {
        return Some(TestResult::fail(
            &tc.name,
            &format!(
                "Expected minimum delay of {}ms, got {}ms",
                min_delay,
                delays.iter().min().unwrap_or(&0)
            ),
        ));
    }
    None
}

fn check_no_error(tc: &TestCase, result: &OperationResult) -> Option<TestResult> {
    if result.error.is_some() {
        let msg = match &result.error {
            Some(BasecampError::Network { message }) => message.clone(),
            Some(e) => e.to_string(),
            None => "Unknown error".to_string(),
        };
        return Some(TestResult::fail(
            &tc.name,
            &format!("Expected no error, got: {}", msg),
        ));
    }
    None
}

fn check_status_code(
    tc: &TestCase,
    assertion: &Assertion,
    result: &OperationResult,
) -> Option<TestResult> {
    let expected = assertion
        .expected
        .as_ref()
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u16;

    match &result.error {
        Some(err) => {
            if let Some(status) = err.http_status() {
                if status != expected {
                    return Some(TestResult::fail(
                        &tc.name,
                        &format!("Expected status {}, got {}", expected, status),
                    ));
                }
            } else if expected >= 400 {
                return Some(TestResult::fail(
                    &tc.name,
                    &format!(
                        "Expected error with status {}, but error has no HTTP status",
                        expected
                    ),
                ));
            }
        }
        None => {
            if expected >= 400 {
                return Some(TestResult::fail(
                    &tc.name,
                    &format!(
                        "Expected error with status {}, but operation succeeded",
                        expected
                    ),
                ));
            }
        }
    }
    None
}

fn check_error_code(
    tc: &TestCase,
    assertion: &Assertion,
    result: &OperationResult,
) -> Option<TestResult> {
    let expected = match &assertion.expected {
        Some(serde_json::Value::String(s)) => s.as_str(),
        _ => {
            return Some(TestResult::fail(
                &tc.name,
                "errorCode assertion missing expected string",
            ))
        }
    };

    match &result.error {
        Some(err) => {
            let actual = err.code().as_str();
            if actual != expected {
                return Some(TestResult::fail(
                    &tc.name,
                    &format!("Expected error code {:?}, got {:?}", expected, actual),
                ));
            }
        }
        None => {
            return Some(TestResult::fail(
                &tc.name,
                &format!("Expected error code {:?}, but got no error", expected),
            ));
        }
    }
    None
}

fn check_error_type(
    tc: &TestCase,
    assertion: &Assertion,
    result: &OperationResult,
) -> Option<TestResult> {
    let expected_type = match &assertion.expected {
        Some(serde_json::Value::String(s)) => s.as_str(),
        _ => {
            return Some(TestResult::fail(
                &tc.name,
                "errorType assertion missing expected string",
            ))
        }
    };

    match &result.error {
        Some(_) => {}
        None => {
            return Some(TestResult::fail(
                &tc.name,
                &format!("Expected error type {:?}, but got no error", expected_type),
            ));
        }
    }
    None
}

fn check_error_message(
    tc: &TestCase,
    assertion: &Assertion,
    result: &OperationResult,
) -> Option<TestResult> {
    let expected = match &assertion.expected {
        Some(serde_json::Value::String(s)) => s.as_str(),
        _ => {
            return Some(TestResult::fail(
                &tc.name,
                "errorMessage assertion missing expected string",
            ))
        }
    };

    match &result.error {
        Some(err) => {
            let msg = err.to_string();
            if !msg.contains(expected) {
                return Some(TestResult::fail(
                    &tc.name,
                    &format!(
                        "Expected error message containing {:?}, got {:?}",
                        expected, msg
                    ),
                ));
            }
        }
        None => {
            return Some(TestResult::fail(
                &tc.name,
                &format!(
                    "Expected error message containing {:?}, but got no error",
                    expected
                ),
            ));
        }
    }
    None
}

fn check_error_field(
    tc: &TestCase,
    assertion: &Assertion,
    result: &OperationResult,
) -> Option<TestResult> {
    let field_path = match &assertion.path {
        Some(p) => p.as_str(),
        None => {
            return Some(TestResult::fail(
                &tc.name,
                "errorField assertion missing path",
            ))
        }
    };

    let expected = &assertion.expected;

    match &result.error {
        Some(err) => {
            let actual = get_error_field_value(err, field_path);
            if !values_match(expected, &actual) {
                return Some(TestResult::fail(
                    &tc.name,
                    &format!(
                        "Expected error.{} = {:?}, got {:?}",
                        field_path, expected, actual
                    ),
                ));
            }
        }
        None => {
            return Some(TestResult::fail(
                &tc.name,
                &format!("Expected error field {}, but got no error", field_path),
            ));
        }
    }
    None
}

fn check_request_path(
    tc: &TestCase,
    assertion: &Assertion,
    tracker: &RequestTracker,
) -> Option<TestResult> {
    let expected = match &assertion.expected {
        Some(serde_json::Value::String(s)) => s.as_str(),
        _ => {
            return Some(TestResult::fail(
                &tc.name,
                "requestPath assertion missing expected string",
            ))
        }
    };

    if tracker.paths.is_empty() {
        return Some(TestResult::fail(
            &tc.name,
            "Expected a request, but none recorded",
        ));
    }

    let actual_path = extract_path(&tracker.paths[0]);
    if actual_path != expected {
        return Some(TestResult::fail(
            &tc.name,
            &format!(
                "Expected request path {:?}, got {:?}",
                expected, actual_path
            ),
        ));
    }
    None
}

fn check_header_injected(
    tc: &TestCase,
    assertion: &Assertion,
    tracker: &RequestTracker,
) -> Option<TestResult> {
    let header_name = match &assertion.path {
        Some(p) => p.as_str(),
        None => {
            return Some(TestResult::fail(
                &tc.name,
                "headerInjected assertion missing path",
            ))
        }
    };

    let expected = match &assertion.expected {
        Some(serde_json::Value::String(s)) => s.as_str(),
        _ => {
            return Some(TestResult::fail(
                &tc.name,
                "headerInjected assertion missing expected string",
            ))
        }
    };

    if tracker.headers.is_empty() {
        return Some(TestResult::fail(
            &tc.name,
            &format!(
                "Expected header {}={:?}, but no requests recorded",
                header_name, expected
            ),
        ));
    }

    let actual = find_header(&tracker.headers[0], header_name);
    if actual != Some(expected) {
        return Some(TestResult::fail(
            &tc.name,
            &format!(
                "Expected header {}={:?}, got {:?}",
                header_name, expected, actual
            ),
        ));
    }
    None
}

fn check_header_present(
    tc: &TestCase,
    assertion: &Assertion,
    tracker: &RequestTracker,
) -> Option<TestResult> {
    let header_name = match &assertion.path {
        Some(p) => p.as_str(),
        None => {
            return Some(TestResult::fail(
                &tc.name,
                "headerPresent assertion missing path",
            ))
        }
    };

    if tracker.headers.is_empty() {
        return Some(TestResult::fail(
            &tc.name,
            &format!(
                "Expected header {} to be present, but no requests recorded",
                header_name
            ),
        ));
    }

    let actual = find_header(&tracker.headers[0], header_name);
    if actual.is_none() || actual == Some("") {
        return Some(TestResult::fail(
            &tc.name,
            &format!(
                "Expected header {} to be present, but it was empty or missing",
                header_name
            ),
        ));
    }
    None
}

fn check_header_value(tc: &TestCase, assertion: &Assertion) -> Option<TestResult> {
    let header_name = match &assertion.path {
        Some(p) => p.as_str(),
        None => {
            return Some(TestResult::fail(
                &tc.name,
                "headerValue assertion missing path",
            ))
        }
    };

    let expected = match &assertion.expected {
        Some(serde_json::Value::String(s)) => s.as_str(),
        _ => {
            return Some(TestResult::fail(
                &tc.name,
                "headerValue assertion missing expected string",
            ))
        }
    };

    if tc.mock_responses.is_empty() {
        return Some(TestResult::fail(
            &tc.name,
            &format!(
                "Expected response header {}={:?}, but no mock responses defined",
                header_name, expected
            ),
        ));
    }

    let actual = tc.mock_responses[0].headers.get(header_name);
    if actual != Some(&expected.to_string()) {
        return Some(TestResult::fail(
            &tc.name,
            &format!(
                "Expected response header {}={:?}, got {:?}",
                header_name, expected, actual
            ),
        ));
    }
    None
}

fn check_response_body(
    tc: &TestCase,
    assertion: &Assertion,
    result: &OperationResult,
) -> Option<TestResult> {
    let field_path = match &assertion.path {
        Some(p) => p.as_str(),
        None => {
            return Some(TestResult::fail(
                &tc.name,
                "responseBody assertion missing path",
            ))
        }
    };

    match &result.body {
        Some(body) => {
            let actual = dig_path(body, field_path);
            if !values_match(&assertion.expected, &actual) {
                return Some(TestResult::fail(
                    &tc.name,
                    &format!(
                        "Expected responseBody.{} = {:?}, got {:?}",
                        field_path, assertion.expected, actual
                    ),
                ));
            }
        }
        None => {
            return Some(TestResult::fail(
                &tc.name,
                &format!(
                    "Expected responseBody.{}, but no result returned",
                    field_path
                ),
            ));
        }
    }
    None
}

fn check_response_meta(
    tc: &TestCase,
    assertion: &Assertion,
    result: &OperationResult,
) -> Option<TestResult> {
    let field_path = match &assertion.path {
        Some(p) => p.as_str(),
        None => {
            return Some(TestResult::fail(
                &tc.name,
                "responseMeta assertion missing path",
            ))
        }
    };

    match &result.meta {
        Some(meta) => {
            let actual = meta.get(field_path).cloned();
            if !values_match(&assertion.expected, &actual) {
                return Some(TestResult::fail(
                    &tc.name,
                    &format!(
                        "Expected meta.{} = {:?}, got {:?}",
                        field_path, assertion.expected, actual
                    ),
                ));
            }
        }
        None => {
            return Some(TestResult::fail(
                &tc.name,
                &format!(
                    "Expected response meta {}, but no metadata returned",
                    field_path
                ),
            ));
        }
    }
    None
}

fn check_response_status(
    tc: &TestCase,
    assertion: &Assertion,
    result: &OperationResult,
) -> Option<TestResult> {
    let expected = assertion
        .expected
        .as_ref()
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u16;

    if let Some(err) = &result.error {
        if let Some(status) = err.http_status() {
            if status != expected {
                return Some(TestResult::fail(
                    &tc.name,
                    &format!("Expected response status {}, got {}", expected, status),
                ));
            }
        }
    } else if expected >= 400 {
        return Some(TestResult::fail(
            &tc.name,
            &format!(
                "Expected error with status {}, but operation succeeded",
                expected
            ),
        ));
    }
    None
}

fn check_request_scheme(
    tc: &TestCase,
    assertion: &Assertion,
    result: &OperationResult,
) -> Option<TestResult> {
    let expected = match &assertion.expected {
        Some(serde_json::Value::String(s)) => s.as_str(),
        _ => return None,
    };

    if expected == "https" && result.error.is_none() {
        return Some(TestResult::fail(
            &tc.name,
            "Expected HTTPS enforcement error, but request succeeded over HTTP",
        ));
    }
    None
}

fn check_url_origin(
    tc: &TestCase,
    assertion: &Assertion,
    tracker: &RequestTracker,
) -> Option<TestResult> {
    let expected = match &assertion.expected {
        Some(serde_json::Value::String(s)) => s.as_str(),
        _ => return None,
    };

    if expected == "rejected" && tracker.count > 1 {
        return Some(TestResult::fail(
            &tc.name,
            &format!(
                "Expected cross-origin URL rejection (1 request), but {} requests were made",
                tracker.count
            ),
        ));
    }
    None
}

fn get_error_field_value(err: &BasecampError, field: &str) -> Option<serde_json::Value> {
    match field {
        "httpStatus" => err.http_status().map(|s| serde_json::json!(s)),
        "retryable" => Some(serde_json::json!(err.retryable())),
        "requestId" => err.request_id().map(|s| serde_json::json!(s)),
        "code" => Some(serde_json::json!(err.code().as_str())),
        "message" => Some(serde_json::json!(err.to_string())),
        _ => None,
    }
}

fn find_header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn extract_path(url: &str) -> &str {
    if let Some(pos) = url.find("://") {
        let rest = &url[pos + 3..];
        if let Some(slash_pos) = rest.find('/') {
            return &rest[slash_pos..];
        }
    }
    url
}

fn dig_path(value: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;

    for part in parts {
        if part.is_empty() {
            continue;
        }
        current = if let Ok(idx) = part.parse::<usize>() {
            current.get(idx)?
        } else {
            current.get(part)?
        };
    }

    Some(current.clone())
}

fn values_match(expected: &Option<serde_json::Value>, actual: &Option<serde_json::Value>) -> bool {
    match (expected, actual) {
        (None, None) => true,
        (Some(e), None) => e.is_null(),
        (None, Some(a)) => a.is_null(),
        (Some(e), Some(a)) => match (e, a) {
            (serde_json::Value::Number(en), serde_json::Value::Number(an)) => {
                if let (Some(ei), Some(ai)) = (en.as_i64(), an.as_i64()) {
                    return ei == ai;
                }
                if let (Some(ef), Some(af)) = (en.as_f64(), an.as_f64()) {
                    return (ef - af).abs() < f64::EPSILON;
                }
                en == an
            }
            (serde_json::Value::Bool(eb), serde_json::Value::Bool(ab)) => eb == ab,
            (serde_json::Value::String(es), serde_json::Value::String(as_)) => es == as_,
            _ => e == a,
        },
    }
}
