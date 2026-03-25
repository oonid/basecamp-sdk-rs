# Generated Types Specification

## Overview

This specification defines the patterns for generated data types from the OpenAPI schema.
All types are auto-generated and should not be manually edited.

## Generation Rules

### Source
- **Input**: OpenAPI schema components
- **Output**: Rust type modules in `src/generated/types/`
- **Rule**: NEVER edit generated files manually

## Type Naming

| OpenAPI Schema | Rust Type |
|----------------|-----------|
| `Project` | `Project` |
| `Todo` | `Todo` |
| `MessageBoard` | `MessageBoard` |
| `CreateTodoRequest` | `CreateTodoRequest` |

- PascalCase type names
- Preserve original schema names
- Request/Response suffixes preserved

## Type Structure

### Resource Types

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// A project resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Project {
    /// Unique identifier.
    pub id: i64,
    
    /// Resource status.
    pub status: String,
    
    /// Project name.
    pub name: String,
    
    /// Project description (may contain HTML).
    pub description: Option<String>,
    
    /// Whether the project is client-visible.
    pub clients_enabled: bool,
    
    /// Project color (hex).
    pub color: Option<String>,
    
    /// Icon name.
    pub icon: Option<String>,
    
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    
    /// The account this project belongs to.
    pub account: Option<AccountSummary>,
    
    /// Creator of the project.
    pub creator: Option<Person>,
    
    /// Dock items (tools enabled on this project).
    pub dock: Vec<DockItem>,
    
    /// Bookmark URL.
    pub bookmark_url: Option<String>,
    
    /// URL to the project in Basecamp.
    pub app_url: Option<String>,
}
```

### Optional Fields

Fields that may be absent use `Option<T>`:

```rust
pub struct Todo {
    pub id: i64,
    pub content: String,
    
    // Optional fields
    pub description: Option<String>,
    pub due_on: Option<chrono::NaiveDate>,
    pub starts_on: Option<chrono::NaiveDate>,
    pub assignees: Option<Vec<Person>>,
    pub completion_subscriber_ids: Option<Vec<i64>>,
}
```

### Date/Time Handling

```rust
// Full timestamps
pub created_at: DateTime<Utc>

// Date only
pub due_on: Option<chrono::NaiveDate>

// Time only (for schedule entries)
pub starts_at: Option<DateTime<Utc>>
pub ends_at: Option<DateTime<Utc>>
```

### Collections

```rust
pub struct MessageBoard {
    pub id: i64,
    pub name: String,
    
    // Always-present collections
    pub messages: Vec<MessageSummary>,
    
    // Optional collections
    pub categories: Option<Vec<Category>>,
}
```

## Request Types

### Create Requests

```rust
/// Request body for creating a todo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateTodoRequest {
    /// Todo content (required).
    pub content: String,
    
    /// Todo description (optional, may contain HTML).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// IDs of people to assign.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee_ids: Option<Vec<i64>>,
    
    /// Due date (YYYY-MM-DD).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_on: Option<String>,
    
    /// Start date (YYYY-MM-DD).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starts_on: Option<String>,
}
```

### Update Requests

```rust
/// Request body for updating a todo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateTodoRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee_ids: Option<Vec<i64>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_on: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starts_on: Option<String>,
}
```

## Response Types

### Error Responses

```rust
/// Bad request error response.
#[derive(Debug, Clone, Deserialize)]
pub struct BadRequestErrorResponse {
    pub error: String,
}

/// Validation error response.
#[derive(Debug, Clone, Deserialize)]
pub struct ValidationErrorResponse {
    pub error: String,
    pub errors: Option<std::collections::HashMap<String, Vec<String>>>,
}

/// Not found error response.
#[derive(Debug, Clone, Deserialize)]
pub struct NotFoundErrorResponse {
    pub error: String,
}
```

### Wrapped Responses

Some endpoints return wrapped responses:

```rust
/// Paginated todos wrapped in a container.
#[derive(Debug, Clone, Deserialize)]
pub struct TodosResponse {
    pub todos: Vec<Todo>,
    pub completed_count: i64,
    pub remaining_count: i64,
}
```

## Enum Types

### String Enums

```rust
/// Project status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Active,
    Archived,
    Trashed,
}

/// Todo status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Active,
    Completed,
    Trashed,
}
```

### Newtype Enums

For resources with a "type" field:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Recording {
    Todo(Todo),
    Todolist(Todolist),
    Message(Message),
    Comment(Comment),
    Document(Document),
    Upload(Upload),
    // ...
}
```

## Summary Types

Lightweight types for nested references:

```rust
/// Minimal account information.
#[derive(Debug, Clone, Deserialize)]
pub struct AccountSummary {
    pub id: i64,
    pub name: String,
}

/// Minimal person information.
#[derive(Debug, Clone, Deserialize)]
pub struct PersonSummary {
    pub id: i64,
    pub name: String,
    pub email_address: String,
    pub avatar_url: Option<String>,
}

/// Person with full details.
#[derive(Debug, Clone, Deserialize)]
pub struct Person {
    pub id: i64,
    pub name: String,
    pub email_address: String,
    pub title: Option<String>,
    pub avatar_url: Option<String>,
    pub company: Option<ClientCompany>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

## Special Types

### Attachments

```rust
/// File attachment.
#[derive(Debug, Clone, Deserialize)]
pub struct Attachment {
    pub id: i64,
    pub name: String,
    pub content_type: String,
    pub byte_size: i64,
    pub url: String,
    pub app_url: Option<String>,
    pub creator: Option<PersonSummary>,
    pub created_at: DateTime<Utc>,
}
```

### Polymorphic Types

Some resources use inheritance:

```rust
/// Base recording type.
#[derive(Debug, Clone, Deserialize)]
pub struct RecordingBase {
    pub id: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(rename = "type")]
    pub recording_type: String,
    pub url: String,
    pub app_url: Option<String>,
    pub comments_count: i64,
    pub comments_url: Option<String>,
    pub parent: Option<ParentRef>,
    pub bucket: Option<BucketRef>,
    pub creator: Option<Person>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParentRef {
    pub id: i64,
    #[serde(rename = "type")]
    pub parent_type: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BucketRef {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub bucket_type: String,
}
```

## Builder Patterns

Optional: Provide builders for request types:

```rust
impl CreateTodoRequest {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            description: None,
            assignee_ids: None,
            due_on: None,
            starts_on: None,
        }
    }
    
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
    
    pub fn assignee_ids(mut self, ids: Vec<i64>) -> Self {
        self.assignee_ids = Some(ids);
        self
    }
    
    pub fn due_on(mut self, date: impl Into<String>) -> Self {
        self.due_on = Some(date.into());
        self
    }
    
    pub fn starts_on(mut self, date: impl Into<String>) -> Self {
        self.starts_on = Some(date.into());
        self
    }
}
```

## Type Organization

```
src/generated/types/
├── mod.rs              # Re-exports all types
├── account.rs          # Account types
├── project.rs          # Project types
├── person.rs           # Person types
├── todo.rs             # Todo types
├── message.rs          # Message types
├── comment.rs          # Comment types
├── upload.rs           # Upload types
├── campfire.rs         # Campfire types
├── schedule.rs         # Schedule types
├── vault.rs            # Vault types
├── card.rs             # Card table types
├── webhook.rs          # Webhook types
├── errors.rs           # Error response types
└── requests.rs         # Request body types
```

## Verification

- `[static]` All types derive Serialize/Deserialize
- `[static]` Optional fields use Option<T>
- `[unit]` Serde renames are correct
- `[unit]` Date parsing works
- `[conformance]` ~150 types are generated
- `[conformance]` Field names match API
