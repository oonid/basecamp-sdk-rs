# Testing Guide

## Test Categories

### Unit Tests

Unit tests verify individual components in isolation. Use mocks for external dependencies.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    
    #[tokio::test]
    async fn test_parse_next_link() {
        let header = r#"<https://api.example.com/page=2>; rel="next""#;
        let result = parse_next_link(Some(header));
        assert_eq!(result, Some("https://api.example.com/page=2".to_string()));
    }
}
```

### Integration Tests

Integration tests verify components work together correctly.

```rust
#[tokio::test]
async fn test_client_list_projects() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("GET"))
        .and(path("/buckets/123/projects.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![
            json!({"id": 1, "name": "Project 1"}),
            json!({"id": 2, "name": "Project 2"}),
        ]))
        .mount(&mock_server)
        .await;
    
    let client = Client::builder()
        .access_token("test-token")
        .config(Config::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap())
        .build()
        .unwrap();
    
    let account = client.for_account(123);
    let projects = account.projects().list(None).await.unwrap();
    
    assert_eq!(projects.len(), 2);
}
```

### Conformance Tests

Conformance tests verify behavior matches the specification. Port from `vendor/basecamp-sdk/conformance/tests/`.

## Test Structure

```
tests/
├── conformance/          # Ported conformance tests
│   ├── auth.rs
│   ├── error_mapping.rs
│   ├── idempotency.rs
│   ├── integer_precision.rs
│   ├── pagination.rs
│   ├── paths.rs
│   ├── retry.rs
│   ├── security.rs
│   └── status_codes.rs
├── integration/          # Integration tests
│   ├── client.rs
│   ├── services/
│   │   ├── projects.rs
│   │   └── todos.rs
│   └── oauth/
└── fixtures/             # Test fixtures
    └── responses/
```

## Test Utilities

### Mock Server

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};

async fn setup_mock_server() -> MockServer {
    MockServer::start().await
}

fn json_response<T: Serialize>(status: u16, body: T) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(body)
}
```

### Test Fixtures

```rust
pub mod fixtures {
    use serde_json::json;
    
    pub fn sample_project() -> serde_json::Value {
        json!({
            "id": 123456789,
            "name": "Test Project",
            "description": "<p>A test project</p>",
            "status": "active",
            "created_at": "2024-01-15T10:30:00Z",
            "updated_at": "2024-01-15T10:30:00Z"
        })
    }
    
    pub fn sample_todo() -> serde_json::Value {
        json!({
            "id": 987654321,
            "content": "Test todo",
            "description": "Test description",
            "status": "active",
            "created_at": "2024-01-15T10:30:00Z",
            "updated_at": "2024-01-15T10:30:00Z"
        })
    }
}
```

### Test Config

```rust
pub fn test_config(base_url: &str) -> Config {
    Config::builder()
        .base_url(base_url)
        .timeout(Duration::from_secs(5))
        .max_retries(1)
        .base_delay(Duration::from_millis(10))
        .max_jitter(Duration::from_millis(5))
        .build()
        .unwrap()
}
```

## Running Tests

```bash
# Run all tests
cargo test

# Run unit tests only
cargo test --lib

# Run integration tests
cargo test --test '*'

# Run specific test
cargo test test_parse_next_link

# Run with output
cargo test -- --nocapture

# Run conformance tests
cargo test --test conformance
```

## Test Coverage

Uses LLVM source-based code coverage via `cargo-llvm-cov`.

### Setup

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Ensure rustup toolchain is available
rustup component add llvm-tools-preview
```

### Generate Coverage

```bash
# Run tests with coverage
cargo llvm-cov

# Generate HTML report
cargo llvm-cov --html

# View report
open target/llvm-cov/html/index.html

# Generate lcov format (for CI tools)
cargo llvm-cov --lcov --output-path lcov.info

# Generate cobertura format (for codecov/coveralls)
cargo llvm-cov --cobertura --output-path cobertura.xml

# Run specific tests with coverage
cargo llvm-cov --lib -- tests::test_parse_next_link
```

### Coverage Flags

| Flag | Description |
|------|-------------|
| `--html` | Generate HTML report |
| `--lcov` | Generate lcov.info format |
| `--cobertura` | Generate cobertura.xml format |
| `--json` | Generate JSON format |
| `--ignore-filename-regex` | Exclude files from coverage |
| `--branch` | Enable branch coverage |

### Excluding Generated Code

```bash
# Exclude generated code from coverage
cargo llvm-cov --html --ignore-filename-regex="generated/"
```

### CI Integration

```yaml
# GitHub Actions example
- name: Install llvm-cov
  run: cargo install cargo-llvm-cov

- name: Generate coverage
  run: cargo llvm-cov --lcov --output-path lcov.info

- name: Upload to codecov
  uses: codecov/codecov-action@v3
  with:
    files: lcov.info
```

## Continuous Integration

Tests should run in CI with:

1. All unit tests pass
2. All integration tests pass
3. All conformance tests pass
4. Code coverage > 80%
5. No clippy warnings
6. `cargo fmt --check` passes
