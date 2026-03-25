# Generated Services Specification

## Overview

This specification defines the patterns and conventions for generated service classes.
All services are auto-generated from the OpenAPI spec and should not be manually edited.

## Generation Rules

### Source
- **Input**: OpenAPI specification from Smithy model
- **Output**: Rust service modules in `src/generated/services/`
- **Rule**: NEVER edit generated files manually

### Service Naming

| OpenAPI Tag | Rust Service Name | File |
|-------------|-------------------|------|
| `Projects` | `ProjectsService` | `projects.rs` |
| `Todos` | `TodosService` | `todos.rs` |
| `Message Boards` | `MessageBoardsService` | `message_boards.rs` |
| `Card Tables` | `CardTablesService` | `card_tables.rs` |

- PascalCase service names
- Pluralized resource names
- Spaces converted to underscores

## Base Service Trait

```rust
/// Base trait for all generated services.
#[async_trait]
pub trait BaseService: Send + Sync {
    /// Get the service name.
    fn service_name(&self) -> &'static str;
}

/// Sync service base functionality.
pub trait SyncService: BaseService {
    /// Internal: Make a request returning JSON.
    fn _request<T: DeserializeOwned>(
        &self,
        info: OperationInfo,
        method: Method,
        path: &str,
        json_body: Option<&serde_json::Value>,
    ) -> Result<T, ServiceError>;
    
    /// Internal: Make a request returning nothing.
    fn _request_void(
        &self,
        info: OperationInfo,
        method: Method,
        path: &str,
        json_body: Option<&serde_json::Value>,
    ) -> Result<(), ServiceError>;
    
    /// Internal: Make a paginated request.
    fn _request_paginated<T: DeserializeOwned>(
        &self,
        info: OperationInfo,
        path: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<ListResult<T>, ServiceError>;
    
    /// Internal: Make a request with raw bytes.
    fn _request_raw<T: DeserializeOwned>(
        &self,
        info: OperationInfo,
        path: &str,
        content: &[u8],
        content_type: &str,
    ) -> Result<T, ServiceError>;
    
    /// Internal: Make a multipart request.
    fn _request_multipart_void(
        &self,
        info: OperationInfo,
        method: Method,
        path: &str,
        field: &str,
        content: &[u8],
        filename: &str,
        content_type: &str,
    ) -> Result<(), ServiceError>;
    
    /// Build a bucket-scoped path.
    fn _bucket_path(&self, project_id: i64, path: &str) -> String;
}

/// Async service base functionality.
#[async_trait]
pub trait AsyncService: BaseService {
    /// Internal: Make a request returning JSON.
    async fn _request<T: DeserializeOwned + Send>(
        &self,
        info: OperationInfo,
        method: Method,
        path: &str,
        json_body: Option<&serde_json::Value>,
    ) -> Result<T, ServiceError>;
    
    // ... other methods similar to SyncService but async
}
```

## Service Method Patterns

### Method Naming

| Operation Pattern | Method Name |
|-------------------|-------------|
| `List*` | `list` |
| `Get*` | `get` |
| `Create*` | `create` |
| `Update*` | `update` |
| `Delete*` | `delete` / `trash` |
| `Complete*` | `complete` |
| `Uncomplete*` | `uncomplete` |
| `Reposition*` | `reposition` |

### Parameter Patterns

All service methods use **keyword-only** parameters:

```rust
// CORRECT
fn list(&self, *, status: Option<&str>) -> Result<ListResult<Todo>, ServiceError>;

// INCORRECT - positional parameters
fn list(&self, status: Option<&str>) -> Result<ListResult<Todo>, ServiceError>;
```

### Return Types

| Operation | Return Type |
|-----------|-------------|
| List | `Result<ListResult<T>, ServiceError>` |
| Get | `Result<T, ServiceError>` |
| Create | `Result<T, ServiceError>` |
| Update | `Result<T, ServiceError>` |
| Delete/Trash | `Result<(), ServiceError>` |
| Action (complete, etc.) | `Result<(), ServiceError>` |

## Service Examples

### ProjectsService

```rust
/// Projects service.
pub struct ProjectsService {
    client: Arc<AccountClient>,
}

impl ProjectsService {
    /// List all projects.
    pub fn list(
        &self,
        *, 
        status: Option<&str>,
    ) -> Result<ListResult<Project>, ServiceError>;
    
    /// Create a new project.
    pub fn create(
        &self,
        *,
        name: &str,
        description: Option<&str>,
    ) -> Result<Project, ServiceError>;
    
    /// Get a project by ID.
    pub fn get(
        &self,
        *,
        project_id: i64,
    ) -> Result<Project, ServiceError>;
    
    /// Update a project.
    pub fn update(
        &self,
        *,
        project_id: i64,
        name: &str,
        description: Option<&str>,
    ) -> Result<Project, ServiceError>;
    
    /// Trash a project.
    pub fn trash(
        &self,
        *,
        project_id: i64,
    ) -> Result<(), ServiceError>;
}
```

### TodosService

```rust
/// Todos service.
pub struct TodosService {
    client: Arc<AccountClient>,
}

impl TodosService {
    /// List todos in a todolist.
    pub fn list(
        &self,
        *,
        todolist_id: i64,
        status: Option<&str>,
        completed: Option<bool>,
    ) -> Result<ListResult<Todo>, ServiceError>;
    
    /// Create a todo.
    pub fn create(
        &self,
        *,
        todolist_id: i64,
        content: &str,
        description: Option<&str>,
        assignee_ids: Option<&[i64]>,
        due_on: Option<&str>,
        starts_on: Option<&str>,
    ) -> Result<Todo, ServiceError>;
    
    /// Get a todo by ID.
    pub fn get(&self, *, todo_id: i64) -> Result<Todo, ServiceError>;
    
    /// Update a todo.
    pub fn update(
        &self,
        *,
        todo_id: i64,
        content: Option<&str>,
        description: Option<&str>,
        assignee_ids: Option<&[i64]>,
        due_on: Option<&str>,
        starts_on: Option<&str>,
    ) -> Result<Todo, ServiceError>;
    
    /// Trash a todo.
    pub fn trash(&self, *, todo_id: i64) -> Result<(), ServiceError>;
    
    /// Complete a todo.
    pub fn complete(&self, *, todo_id: i64) -> Result<(), ServiceError>;
    
    /// Uncomplete a todo.
    pub fn uncomplete(&self, *, todo_id: i64) -> Result<(), ServiceError>;
    
    /// Reposition a todo.
    pub fn reposition(
        &self,
        *,
        todo_id: i64,
        position: i64,
        parent_id: Option<i64>,
    ) -> Result<(), ServiceError>;
}
```

### CampfiresService (with binary upload)

```rust
/// Campfires service.
pub struct CampfiresService {
    client: Arc<AccountClient>,
}

impl CampfiresService {
    /// List campfires.
    pub fn list(&self) -> Result<ListResult<Campfire>, ServiceError>;
    
    /// Get a campfire.
    pub fn get(&self, *, campfire_id: i64) -> Result<Campfire, ServiceError>;
    
    /// List chatbots in a campfire.
    pub fn list_chatbots(
        &self,
        *,
        campfire_id: i64,
    ) -> Result<ListResult<Chatbot>, ServiceError>;
    
    /// Create a chatbot.
    pub fn create_chatbot(
        &self,
        *,
        campfire_id: i64,
        service_name: &str,
        command_url: Option<&str>,
    ) -> Result<Chatbot, ServiceError>;
    
    /// List lines in a campfire.
    pub fn list_lines(
        &self,
        *,
        campfire_id: i64,
        sort: Option<&str>,
        direction: Option<&str>,
    ) -> Result<ListResult<CampfireLine>, ServiceError>;
    
    /// Create a text line.
    pub fn create_line(
        &self,
        *,
        campfire_id: i64,
        content: &str,
        content_type: Option<&str>,
    ) -> Result<CampfireLine, ServiceError>;
    
    /// Create an upload line (binary content).
    pub fn create_upload(
        &self,
        *,
        campfire_id: i64,
        content: &[u8],
        content_type: &str,
        name: &str,
    ) -> Result<CampfireLine, ServiceError>;
}
```

## Person ID Normalization

System actors (like "basecamp", "campfire") have string IDs. Services normalize these:

```rust
/// Normalized person reference.
#[derive(Debug, Clone)]
pub struct PersonRef {
    /// Numeric ID (0 for system actors).
    pub id: i64,
    
    /// Original string ID for system actors.
    pub system_label: Option<String>,
}

/// Normalize a person ID.
fn normalize_person_id(id: &str) -> PersonRef {
    match id.parse::<i64>() {
        Ok(n) => PersonRef { id: n, system_label: None },
        Err(_) => PersonRef { 
            id: 0, 
            system_label: Some(id.to_string()) 
        },
    }
}
```

- `[unit]` Numeric IDs parse correctly
- `[unit]` String IDs return id=0 with system_label
- `[conformance]` Handles "basecamp", "campfire", etc.

## API Path Patterns

| Pattern | Example |
|---------|---------|
| List resources | `/buckets/{account}/projects.json` |
| Create resource | `/buckets/{account}/projects.json` (POST) |
| Get single | `/buckets/{account}/projects/{id}.json` |
| Update | `/buckets/{account}/projects/{id}.json` (PUT) |
| Delete | `/buckets/{account}/projects/{id}.json` (DELETE) |
| Nested list | `/buckets/{account}/todolists/{id}/todos.json` |
| Actions | `/buckets/{account}/todos/{id}/completion.json` |

## Async Variants

Every service has an async variant:

```rust
/// Async projects service.
pub struct AsyncProjectsService {
    client: Arc<AsyncAccountClient>,
}

impl AsyncProjectsService {
    pub async fn list(&self, *, status: Option<&str>) -> Result<ListResult<Project>, ServiceError>;
    pub async fn create(&self, *, name: &str, description: Option<&str>) -> Result<Project, ServiceError>;
    // ...
}
```

## Service Registration

Services are registered with AccountClient:

```rust
impl AccountClient {
    lazy_static! {
        static ref PROJECTS: OnceCell<ProjectsService> = OnceCell::new();
    }
    
    pub fn projects(&self) -> &ProjectsService {
        PROJECTS.get_or_init(|| ProjectsService::new(self.clone()))
    }
}
```

## Verification

- `[static]` All services implement BaseService
- `[static]` Methods use keyword-only parameters
- `[unit]` Service names are correct
- `[unit]` Path construction is correct
- `[unit]` Pagination returns ListResult
- `[conformance]` 40 services are generated
- `[conformance]` ~175 operations are implemented
