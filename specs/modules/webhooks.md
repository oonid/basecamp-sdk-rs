# Webhooks Specification

## Overview

The webhooks module provides utilities for receiving and verifying webhook payloads
from Basecamp, including HMAC signature verification and event handling.

## WebhookReceiver

```rust
/// Webhook receiver for handling Basecamp webhooks.
pub struct WebhookReceiver {
    secret: Option<String>,
    signature_header: String,
    dedup_window: DeduplicationWindow,
    handlers: Vec<HandlerEntry>,
    middlewares: Vec<Box<dyn Middleware>>,
}

/// Handler entry with pattern matching.
struct HandlerEntry {
    pattern: String,
    glob: glob::Pattern,
    handler: Box<dyn WebhookHandler>,
}

impl WebhookReceiver {
    /// Create a new webhook receiver.
    pub fn new() -> Self;
    
    /// Set the webhook secret for signature verification.
    pub fn with_secret(self, secret: impl Into<String>) -> Self;
    
    /// Set the signature header name (default: "X-Basecamp-Signature").
    pub fn with_signature_header(self, header: impl Into<String>) -> Self;
    
    /// Set the deduplication window size.
    pub fn with_dedup_window_size(self, size: usize) -> Self;
    
    /// Register a handler for a specific event kind.
    /// Supports glob patterns (e.g., "todo_*" matches "todo_created", "todo_completed").
    pub fn on(self, pattern: &str, handler: impl WebhookHandler + 'static) -> Self;
    
    /// Register a handler that receives all events.
    pub fn on_any(self, handler: impl WebhookHandler + 'static) -> Self;
    
    /// Add middleware to the processing chain.
    pub fn use_middleware(self, middleware: impl Middleware + 'static) -> Self;
    
    /// Handle an incoming webhook request.
    pub async fn handle_request(
        &self,
        raw_body: &[u8],
        headers: &HeaderMap,
    ) -> Result<WebhookResponse, WebhookError>;
}
```

## Signature Verification

```rust
/// Default signature header name.
pub const DEFAULT_SIGNATURE_HEADER: &str = "X-Basecamp-Signature";

/// Compute HMAC-SHA256 signature for a payload.
pub fn compute_signature(payload: &[u8], secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(payload);
    hex::encode(mac.finalize().into_bytes())
}

/// Verify a signature against a payload.
/// Returns true if the signature matches.
pub fn verify_signature(payload: &[u8], secret: &str, signature: &str) -> bool {
    // Constant-time comparison to prevent timing attacks
    let expected = compute_signature(payload, secret);
    constant_time_compare(&expected, signature)
}

/// Constant-time string comparison.
fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    let mut result = 0u8;
    for (a_byte, b_byte) in a.bytes().zip(b.bytes()) {
        result |= a_byte ^ b_byte;
    }
    
    result == 0
}
```

- `[unit]` Signature computed correctly
- `[unit]` Valid signatures verify
- `[unit]` Invalid signatures rejected
- `[unit]` Comparison is constant-time
- `[conformance]` Uses HMAC-SHA256
- `[conformance]` Output is hex-encoded

## Event Kinds

```rust
/// Webhook event kind constants.
pub mod event_kinds {
    // Todos
    pub const TODO_CREATED: &str = "todo_created";
    pub const TODO_UPDATED: &str = "todo_updated";
    pub const TODO_COMPLETED: &str = "todo_completed";
    pub const TODO_UNCOMPLETED: &str = "todo_uncompleted";
    pub const TODO_TRASHED: &str = "todo_trashed";
    pub const TODO_RESTORED: &str = "todo_restored";
    
    // Messages
    pub const MESSAGE_CREATED: &str = "message_created";
    pub const MESSAGE_UPDATED: &str = "message_updated";
    pub const MESSAGE_TRASHED: &str = "message_trashed";
    pub const MESSAGE_RESTORED: &str = "message_restored";
    
    // Comments
    pub const COMMENT_CREATED: &str = "comment_created";
    pub const COMMENT_UPDATED: &str = "comment_updated";
    pub const COMMENT_TRASHED: &str = "comment_trashed";
    pub const COMMENT_RESTORED: &str = "comment_restored";
    
    // Projects
    pub const PROJECT_CREATED: &str = "project_created";
    pub const PROJECT_UPDATED: &str = "project_updated";
    pub const PROJECT_TRASHED: &str = "project_trashed";
    pub const PROJECT_RESTORED: &str = "project_restored";
    pub const PROJECT_ARCHIVED: &str = "project_archived";
    pub const PROJECT_UNARCHIVED: &str = "project_unarchived";
    
    // Todolists
    pub const TODOLIST_CREATED: &str = "todolist_created";
    pub const TODOLIST_UPDATED: &str = "todolist_updated";
    pub const TODOLIST_TRASHED: &str = "todolist_trashed";
    pub const TODOLIST_RESTORED: &str = "todolist_restored";
    
    // Documents
    pub const DOCUMENT_CREATED: &str = "document_created";
    pub const DOCUMENT_UPDATED: &str = "document_updated";
    pub const DOCUMENT_TRASHED: &str = "document_trashed";
    pub const DOCUMENT_RESTORED: &str = "document_restored";
    
    // Uploads
    pub const UPLOAD_CREATED: &str = "upload_created";
    pub const UPLOAD_UPDATED: &str = "upload_updated";
    pub const UPLOAD_TRASHED: &str = "upload_trashed";
    pub const UPLOAD_RESTORED: &str = "upload_restored";
    
    // Campfire
    pub const CAMPFIRE_LINE_CREATED: &str = "campfire_line_created";
    pub const CAMPFIRE_LINE_TRASHED: &str = "campfire_line_trashed";
    
    // ... additional event kinds
}

/// Parse an event kind string into type and action.
/// 
/// # Example
/// parse_event_kind("todo_created") -> ("todo", "created")
pub fn parse_event_kind(kind: &str) -> Option<(&str, &str)>;
```

## Webhook Payload

```rust
/// Webhook payload structure.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookPayload {
    /// Event kind (e.g., "todo_created").
    pub kind: String,
    
    /// Project/bucket ID.
    #[serde(rename = "recordingBucketId")]
    pub recording_bucket_id: Option<i64>,
    
    /// Recording ID.
    #[serde(rename = "recordingId")]
    pub recording_id: Option<i64>,
    
    /// Creator information.
    pub creator: Option<WebhookCreator>,
    
    /// Creation timestamp.
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    
    /// Resource-specific payload.
    #[serde(flatten)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebhookCreator {
    pub id: i64,
    pub name: String,
    pub email_address: String,
    #[serde(rename = "type")]
    pub creator_type: String,
}
```

## Handler Trait

```rust
/// Trait for webhook handlers.
#[async_trait]
pub trait WebhookHandler: Send + Sync {
    /// Handle a webhook event.
    async fn handle(&self, event: &WebhookPayload) -> Result<(), WebhookError>;
}

/// Implementation for closures.
impl<F> WebhookHandler for F
where
    F: Fn(&WebhookPayload) -> Result<(), WebhookError> + Send + Sync,
{
    async fn handle(&self, event: &WebhookPayload) -> Result<(), WebhookError> {
        self(event)
    }
}
```

## Middleware

```rust
/// Middleware for webhook processing.
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Process a webhook before/after the handler.
    async fn process(
        &self,
        event: &WebhookPayload,
        next: Next<'_>,
    ) -> Result<WebhookResponse, WebhookError>;
}

/// Next middleware in the chain.
pub struct Next<'a> {
    // private
}

/// Logging middleware.
pub struct LoggingMiddleware;

/// Rate limiting middleware.
pub struct RateLimitMiddleware {
    // private
}
```

## Deduplication

```rust
use std::collections::VecDeque;

/// LRU deduplication window.
struct DeduplicationWindow {
    size: usize,
    seen: VecDeque<String>,
}

impl DeduplicationWindow {
    fn new(size: usize) -> Self;
    
    /// Check if an ID has been seen, and add it if not.
    fn check_and_add(&mut self, id: &str) -> bool;
}

/// Extract a unique ID from the webhook payload.
fn extract_webhook_id(payload: &WebhookPayload) -> String {
    // Use combination of kind, recording_id, and created_at
    format!(
        "{}:{}:{}",
        payload.kind,
        payload.recording_id.unwrap_or(0),
        payload.created_at.as_deref().unwrap_or("")
    )
}
```

- `[unit]` Deduplication prevents duplicate processing
- `[unit]` Window evicts oldest entries
- `[unit]` Default window size is 1000

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    
    #[error("Missing signature header")]
    MissingSignature,
    
    #[error("Invalid payload: {0}")]
    InvalidPayload(String),
    
    #[error("Duplicate webhook")]
    DuplicateWebhook,
    
    #[error("No handler for event kind: {0}")]
    NoHandler(String),
    
    #[error("Handler error: {0}")]
    HandlerError(String),
}
```

## Response

```rust
/// Webhook processing response.
#[derive(Debug, Clone)]
pub struct WebhookResponse {
    /// Whether the webhook was processed successfully.
    pub success: bool,
    
    /// HTTP status code to return.
    pub status: u16,
    
    /// Optional message.
    pub message: Option<String>,
}

impl WebhookResponse {
    /// Success response.
    pub fn ok() -> Self {
        Self {
            success: true,
            status: 200,
            message: None,
        }
    }
    
    /// Acknowledged response (202 Accepted).
    pub fn accepted() -> Self {
        Self {
            success: true,
            status: 202,
            message: None,
        }
    }
}
```

## Usage Example

```rust
use basecamp_sdk::webhooks::{WebhookReceiver, event_kinds};

let receiver = WebhookReceiver::new()
    .with_secret("your-webhook-secret")
    .on(event_kinds::TODO_CREATED, |event| {
        println!("Todo created: {:?}", event.data);
        Ok(())
    })
    .on("todo_*", |event| {
        println!("Todo event: {}", event.kind);
        Ok(())
    })
    .on_any(|event| {
        println!("Received event: {}", event.kind);
        Ok(())
    });

// In your HTTP handler:
async fn handle_webhook(body: Bytes, headers: HeaderMap) -> Response {
    match receiver.handle_request(&body, &headers).await {
        Ok(response) => Response::new(response.status),
        Err(e) => Response::new(500).body(e.to_string()),
    }
}
```

## Verification

- `[unit]` Signature verification works correctly
- `[unit]` Glob patterns match correctly
- `[unit]` Deduplication prevents duplicates
- `[unit]` Middleware chain executes in order
- `[unit]` Error handling is complete
- `[conformance]` HMAC-SHA256 signature algorithm
- `[conformance]` Header name is X-Basecamp-Signature
- `[conformance]` Event kinds match API documentation
