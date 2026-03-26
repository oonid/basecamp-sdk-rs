# Design: Initial Implementation

## Architecture

```
src/
├── lib.rs                    # Public API exports
├── error.rs                  # Error types
├── config.rs                 # Configuration
├── security.rs               # Security utilities
├── pagination.rs             # Pagination types
├── hooks.rs                  # Observability hooks
├── auth/
│   ├── mod.rs
│   ├── strategy.rs           # AuthStrategy trait
│   ├── bearer.rs             # BearerAuth
│   ├── token.rs              # TokenProvider, StaticTokenProvider
│   └── oauth.rs              # OAuthTokenProvider
├── http/
│   ├── mod.rs
│   ├── client.rs             # HttpClient
│   └── retry.rs              # Retry logic
├── client.rs                 # Client, ClientBuilder, AccountClient
└── services/
    ├── mod.rs
    └── authorization.rs      # AuthorizationService (no account context)
```

## Module Dependencies

```
error.rs
    ↑
config.rs → security.rs
    ↑
auth/ (strategy, bearer, token, oauth)
    ↑
http/ (client, retry)
    ↑
pagination.rs + hooks.rs
    ↑
client.rs
    ↑
services/
```

## Key Design Decisions

### D1: Error Taxonomy

Use `thiserror` for error types with a flat enum hierarchy:

```rust
#[derive(Debug, Error)]
pub enum BasecampError {
    #[error("Usage error: {message}")]
    Usage { message: String, hint: Option<String> },
    
    #[error("Resource not found")]
    NotFound { 
        resource_type: Option<String>, 
        resource_id: Option<String>, 
        request_id: Option<String> 
    },
    
    #[error("Authentication required")]
    AuthRequired { hint: Option<String>, request_id: Option<String> },
    
    #[error("Access forbidden")]
    Forbidden { reason: Option<String>, request_id: Option<String> },
    
    #[error("Rate limit exceeded")]
    RateLimit { retry_after: Option<u64>, request_id: Option<String> },
    
    #[error("Network error: {message}")]
    Network { message: String },
    
    #[error("API error")]
    Api { status: u16, message: String, request_id: Option<String> },
    
    #[error("Validation error: {message}")]
    Validation { message: String, fields: Vec<FieldError>, request_id: Option<String> },
}
```

### D2: Configuration Builder

Use builder pattern with validation:

```rust
let config = Config::builder()
    .base_url("https://custom.api.com")
    .timeout(Duration::from_secs(60))
    .max_retries(5)
    .build()?;
```

### D3: Service Lazy Loading

Services are lazy-loaded via `OnceCell`:

```rust
use once_cell::sync::OnceCell;

pub struct AccountClient {
    account_id: i64,
    http: Arc<HttpClient>,
    projects: OnceCell<ProjectsService>,
}

impl AccountClient {
    pub fn projects(&self) -> &ProjectsService {
        self.projects.get_or_init(|| ProjectsService::new(self))
    }
}
```

### D4: Async-First Design

All I/O operations are async:

```rust
impl HttpClient {
    pub async fn get(&self, path: &str) -> Result<Response, HttpError>;
    pub async fn post(&self, path: &str, body: &Value) -> Result<Response, HttpError>;
}
```

### D5: Hook Safety

Hook implementations should not panic the SDK:

```rust
fn safe_hook<F>(hook: &Option<F>)
where
    F: Fn(HookArgs),
{
    if let Some(h) = hook {
        // Hook failures are logged but don't propagate
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| h(args)));
    }
}
```

## External Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `reqwest` | 0.13 | HTTP client |
| `serde` | 1.0 | Serialization |
| `serde_json` | 1.0 | JSON |
| `tokio` | 1.50 | Async runtime |
| `thiserror` | 2.0 | Error derive |
| `url` | 2.5 | URL parsing |
| `chrono` | 0.4 | Date/time |
| `async-trait` | 0.1 | Async traits |
| `once_cell` | 1.21 | Lazy initialization |
| `wiremock` | 0.6 | Mock server (dev) |

## Test Structure

```
tests/
├── conformance/
│   ├── error_mapping.rs
│   ├── retry.rs
│   ├── pagination.rs
│   └── security.rs
└── integration/
    ├── client.rs
    └── auth.rs
```

## Performance Considerations

- Connection pooling via `reqwest::Client`
- Lazy service initialization
- Minimal allocations in hot paths
- Streaming responses for large files (future)
