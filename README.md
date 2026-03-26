# Basecamp Rust SDK

[![crates.io](https://img.shields.io/crates/v/basecamp-sdk-rs.svg)](https://crates.io/crates/basecamp-sdk-rs)
[![Documentation](https://docs.rs/basecamp-sdk-rs/badge.svg)](https://docs.rs/basecamp-sdk-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Unofficial** Rust SDK for the [Basecamp API](https://github.com/basecamp/bc3-api).

> **Note**: This is a community-maintained SDK. For official SDKs, see [basecamp/basecamp-sdk](https://github.com/basecamp/basecamp-sdk) (TypeScript, Go, Python, Ruby).

## Features

- Full coverage of Basecamp API services
- OAuth 2.0 and static token authentication
- Automatic retry with exponential backoff
- Pagination handling with `get_paginated`
- Structured errors with CLI-friendly exit codes
- HTTPS enforcement for non-localhost URLs
- Observability hooks for logging, metrics, and tracing

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
basecamp-sdk-rs = "0.1"
tokio = { version = "1", features = ["full"] }
```

Requires Rust 1.70+.

## Quick Start

### Using a Static Token

```rust
use basecamp_sdk_rs::{Client, Config, BearerAuth};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the client
    let config = Config::builder()
        .base_url("https://3.basecampapi.com/999")
        .build()?;

    // Use a static token
    let auth = BearerAuth::from_token("your-access-token");
    let client = Client::new(config, auth)?;

    // List all projects
    let projects = client.get_paginated::<serde_json::Value>("/projects.json", None).await?;
    for project in projects.items {
        println!("{:?}", project);
    }

    Ok(())
}
```

## Configuration

### Builder Pattern

```rust
use basecamp_sdk_rs::Config;
use std::time::Duration;

let config = Config::builder()
    .base_url("https://3.basecampapi.com/999")
    .timeout(Duration::from_secs(60))
    .max_retries(5)
    .max_pages(100)
    .max_items(1000)
    .build()?;
```

### Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `base_url` | `String` | `https://3.basecampapi.com` | API base URL |
| `timeout` | `Duration` | 30s | Request timeout |
| `max_retries` | `u32` | 3 | Maximum retry attempts |
| `base_delay` | `Duration` | 1s | Base delay for retries |
| `max_jitter` | `Duration` | 100ms | Maximum jitter for retries |
| `max_pages` | `u32` | 10000 | Maximum pages for pagination |
| `max_items` | `Option<u32>` | `None` | Maximum items to return |

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `BASECAMP_TOKEN` | API token | Required |
| `BASECAMP_ACCOUNT_ID` | Account ID | Required |
| `BASECAMP_BASE_URL` | API base URL | `https://3.basecampapi.com` |

## Authentication

### Static Token

```rust
use basecamp_sdk_rs::{BearerAuth, Config, Client};

let auth = BearerAuth::from_token("your-token");
let client = Client::new(config, auth)?;
```

### OAuth 2.0 with Token Refresh

```rust
use basecamp_sdk_rs::{OAuthToken, Config, Client};

// Implement TokenProvider for automatic token refresh
let token_provider = MyTokenProvider::new();
let client = Client::new(config, token_provider)?;
```

## API Coverage

### Projects & Organization

| Service | Methods |
|---------|---------|
| Projects | list, get, create, update, trash |
| Templates | list, get, createProject |
| Tools | get, list, update |
| People | list, get, me, listPingable |

### To-dos

| Service | Methods |
|---------|---------|
| Todos | list, get, create, update, trash, complete, uncomplete, reposition |
| Todolists | list, get, create, update, trash |
| Todosets | get |
| Todolist Groups | list, get, create, reposition |

### Messages & Communication

| Service | Methods |
|---------|---------|
| Messages | list, get, create, update, pin, unpin |
| Message Boards | get |
| Message Types | list, get, create, update, delete |
| Comments | list, get, create, update |
| Campfires | list, get, listLines, getLine, createLine, deleteLine |

### Card Tables (Kanban)

| Service | Methods |
|---------|---------|
| Card Tables | get, listColumns |
| Cards | list, get, create, update, move |
| Card Columns | get, create, update, move |
| Card Steps | list, get, create, update, complete, uncomplete |

### Scheduling

| Service | Methods |
|---------|---------|
| Schedules | get, listEntries, getEntry, createEntry, updateEntry, trashEntry |
| Lineup | create, update, delete |
| Checkins | get, listQuestions, getQuestion, listAnswers, getAnswer |

### Files & Documents

| Service | Methods |
|---------|---------|
| Vaults | list, get, create, update |
| Documents | list, get, create, update, trash |
| Uploads | list, get, create, update, trash |
| Attachments | createUploadUrl, create |

### Integrations & Events

| Service | Methods |
|---------|---------|
| Webhooks | list, get, create, update, delete |
| Subscriptions | get, subscribe, unsubscribe, update |
| Events | list, listForRecording |
| Recordings | archive, unarchive, trash |

### Search & Reports

| Service | Methods |
|---------|---------|
| Search | search |
| Reports | progress, upcoming, assigned, overdue, personProgress |
| Timesheets | forRecording, forProject, report |
| Timeline | get |

## Pagination

Use `get_paginated` for automatic pagination:

```rust
let config = Config::builder()
    .base_url("https://3.basecampapi.com/999")
    .max_pages(10)      // Limit pages
    .max_items(500)     // Limit total items
    .build()?;

let client = Client::new(config, auth)?;

// Fetch all pages automatically
let result = client.get_paginated::<serde_json::Value>("/projects.json", None).await?;

println!("Fetched {} items", result.items.len());
println!("Truncated: {}", result.meta.truncated);
println!("Total count: {:?}", result.meta.total_count);
```

## Error Handling

The SDK provides structured errors with codes for programmatic handling:

```rust
use basecamp_sdk_rs::{BasecampError, ErrorCode};

match client.get("/projects/99999.json", None).await {
    Ok(response) => println!("Success: {:?}", response.status()),
    Err(e) => {
        match e {
            BasecampError::NotFound { message, .. } => {
                eprintln!("Not found: {}", message);
            }
            BasecampError::AuthRequired { message, .. } => {
                eprintln!("Auth required: {}", message);
            }
            BasecampError::RateLimit { retry_after, .. } => {
                eprintln!("Rate limited. Retry after: {:?}", retry_after);
            }
            BasecampError::Validation { message, fields, .. } => {
                eprintln!("Validation error: {}", message);
                for field in fields {
                    eprintln!("  {}: {}", field.field, field.message);
                }
            }
            _ => eprintln!("Error: {}", e),
        }
        
        // Use exit codes for CLI applications
        std::process::exit(e.exit_code());
    }
}
```

### Error Codes

| Code | HTTP Status | Exit Code | Description |
|------|-------------|-----------|-------------|
| `auth_required` | 401 | 3 | Authentication required |
| `forbidden` | 403 | 4 | Access denied |
| `not_found` | 404 | 2 | Resource not found |
| `rate_limit` | 429 | 5 | Rate limit exceeded (retryable) |
| `network` | - | 6 | Network error (retryable) |
| `api_error` | 5xx | 7 | Server error |
| `ambiguous` | - | 8 | Multiple matches found |
| `validation` | 400, 422 | 9 | Invalid request data |
| `usage` | - | 1 | Configuration or argument error |

## Retry Behavior

The SDK automatically retries requests on transient failures:

- **Retryable errors**: 429 (rate limit) and 503 (service unavailable)
- **Backoff**: Exponential with jitter
- **Rate limits**: Respects `Retry-After` header
- **Max retries**: 3 attempts by default

Non-idempotent operations (POST) are not retried by default.

## Observability

### Console Logging

```rust
use basecamp_sdk_rs::{Client, Config, BearerAuth, console_hooks};

let config = Config::builder().base_url("https://3.basecampapi.com/999").build()?;
let auth = BearerAuth::from_token("token");
let hooks = console_hooks();
let client = Client::with_hooks(config, auth, hooks)?;
```

Output:
```
[Basecamp] -> GET https://3.basecampapi.com/999/projects.json
[Basecamp] <- GET https://3.basecampapi.com/999/projects.json 200 (145ms)
```

### Custom Hooks

Implement `BasecampHooks` for custom observability:

```rust
use basecamp_sdk_rs::{BasecampHooks, OperationInfo, OperationResult, RequestInfo, RequestResult};

struct MetricsHooks;

impl BasecampHooks for MetricsHooks {
    fn on_operation_start(&self, info: &OperationInfo) {
        println!("Starting: {}.{}", info.service, info.operation);
    }

    fn on_operation_end(&self, info: &OperationInfo, result: &OperationResult) {
        println!("Completed in {}ms", result.duration_ms);
    }

    fn on_request_end(&self, info: &RequestInfo, result: &RequestResult) {
        println!("{} {} -> {} ({}ms)", 
            info.method, info.url, result.status, result.duration_ms);
    }
}
```

## HTTPS Enforcement

The SDK enforces HTTPS for non-localhost URLs:

```rust
use basecamp_sdk_rs::Config;

// This will fail
let result = Config::builder()
    .base_url("http://api.example.com/999")
    .build();
assert!(result.is_err());

// Localhost is allowed
let config = Config::builder()
    .base_url("http://localhost:8080/999")
    .build()?;
```

## Examples

### Working with Todos

```rust
// List todos in a todolist
let todos = client.get_paginated::<Todo>(
    &format!("/buckets/{}/todolists/{}/todos.json", bucket_id, todolist_id),
    None
).await?;

// Create a todo
let todo = client.post(
    &format!("/buckets/{}/todolists/{}/todos.json", bucket_id, todolist_id),
    Some(&serde_json::json!({
        "content": "Review pull request",
        "description": "<p>Check the new auth flow</p>",
        "due_on": "2026-02-01",
        "assignee_ids": [12345, 67890]
    })),
    None
).await?;

// Complete a todo
client.post(&format!("/buckets/{}/todos/{}/completions.json", bucket_id, todo_id), None, None).await?;
```

### Working with Messages

```rust
// List messages
let messages = client.get_paginated::<Message>(
    &format!("/buckets/{}/message_boards/{}/messages.json", bucket_id, board_id),
    None
).await?;

// Create a message
let msg = client.post(
    &format!("/buckets/{}/message_boards/{}/messages.json", bucket_id, board_id),
    Some(&serde_json::json!({
        "subject": "Weekly Update",
        "content": "<p>Here's what we accomplished...</p>",
    })),
    None
).await?;
```

### Working with Webhooks

```rust
// Create a webhook
let webhook = client.post(
    &format!("/buckets/{}/webhooks.json", bucket_id),
    Some(&serde_json::json!({
        "payload_url": "https://example.com/webhook",
        "types": ["Todo", "Comment"]
    })),
    None
).await?;

// List webhooks
let webhooks = client.get_paginated::<Webhook>(
    &format!("/buckets/{}/webhooks.json", bucket_id),
    None
).await?;

// Delete a webhook
client.delete(&format!("/buckets/{}/webhooks/{}.json", bucket_id, webhook_id), None).await?;
```

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run conformance tests
cargo test --test conformance_test

# Check formatting
cargo fmt --check

# Run linter
cargo clippy -- -D warnings

# Generate coverage
cargo llvm-cov
```

## License

MIT
