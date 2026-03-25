# Client Specification

## Overview

The client module provides the main entry points for interacting with the Basecamp API.
It implements a two-tier architecture: `Client` for authentication and account discovery,
and `AccountClient` for account-scoped operations.

## Interface: Client

The root client for the SDK. Manages HTTP transport and authentication.

### Construction

```rust
pub struct Client {
    // private fields
}

pub struct ClientBuilder {
    // private fields
}

impl Client {
    /// Create a new client with an access token.
    pub fn new(access_token: impl Into<String>) -> Self;
    
    /// Create a client builder for advanced configuration.
    pub fn builder() -> ClientBuilder;
}

impl ClientBuilder {
    /// Set the access token directly.
    pub fn access_token(self, token: impl Into<String>) -> Self;
    
    /// Set a token provider for automatic token refresh.
    pub fn token_provider(self, provider: impl TokenProvider + 'static) -> Self;
    
    /// Set an authentication strategy directly.
    pub fn auth(self, auth: impl AuthStrategy + 'static) -> Self;
    
    /// Set custom configuration.
    pub fn config(self, config: Config) -> Self;
    
    /// Set hooks for observability.
    pub fn hooks(self, hooks: impl BasecampHooks + 'static) -> Self;
    
    /// Set custom user agent.
    pub fn user_agent(self, user_agent: impl Into<String>) -> Self;
    
    /// Build the client.
    pub fn build(self) -> Result<Client, ClientError>;
}
```

### Constraints

- **Exactly-one-of**: Must provide exactly one of:
  - `access_token` - Static bearer token
  - `token_provider` - Dynamic token with refresh capability
  - `auth` - Custom authentication strategy
- `[static]` Multiple auth sources must fail at compile time or return `ClientError::AmbiguousAuth`

### Methods

```rust
impl Client {
    /// Get an account-scoped client.
    pub fn for_account(&self, account_id: impl Into<i64>) -> AccountClient;
    
    /// Access the authorization service (no account context required).
    pub fn authorization(&self) -> &AuthorizationService;
    
    /// Close the client and release resources.
    pub async fn close(self);
    
    /// Check if the client is closed.
    pub fn is_closed(&self) -> bool;
}

impl Drop for Client {
    /// Ensure resources are cleaned up.
    fn drop(&mut self);
}
```

### Verification

- `[unit]` `new()` creates client with default config
- `[unit]` `builder().access_token(t).build()` succeeds
- `[unit]` `builder().access_token(t).token_provider(p).build()` returns `AmbiguousAuth`
- `[conformance]` `for_account(id)` returns client with correct account context

---

## Interface: AccountClient

Account-scoped client providing access to all generated services.

### Construction

```rust
pub struct AccountClient {
    // private fields
}

// AccountClient is created only via Client::for_account()
```

### Service Properties

The `AccountClient` provides lazy-loaded service accessors for all 40 services:

```rust
impl AccountClient {
    // Project management
    pub fn projects(&self) -> &ProjectsService;
    pub fn todos(&self) -> &TodosService;
    pub fn todolists(&self) -> &TodolistsService;
    pub fn todosets(&self) -> &TodosetsService;
    pub fn todolist_groups(&self) -> &TodolistGroupsService;
    
    // Communication
    pub fn messages(&self) -> &MessagesService;
    pub fn message_boards(&self) -> &MessageBoardsService;
    pub fn message_types(&self) -> &MessageTypesService;
    pub fn comments(&self) -> &CommentsService;
    pub fn campfires(&self) -> &CampfiresService;
    
    // Scheduling
    pub fn schedules(&self) -> &SchedulesService;
    pub fn timeline(&self) -> &TimelineService;
    pub fn lineup(&self) -> &LineupService;
    pub fn checkins(&self) -> &CheckinsService;
    
    // File management
    pub fn vaults(&self) -> &VaultsService;
    pub fn documents(&self) -> &DocumentsService;
    pub fn uploads(&self) -> &UploadsService;
    pub fn attachments(&self) -> &AttachmentsService;
    
    // Card tables (Shape Up)
    pub fn card_tables(&self) -> &CardTablesService;
    pub fn cards(&self) -> &CardsService;
    pub fn card_columns(&self) -> &CardColumnsService;
    pub fn card_steps(&self) -> &CardStepsService;
    
    // Client access
    pub fn client_approvals(&self) -> &ClientApprovalsService;
    pub fn client_correspondences(&self) -> &ClientCorrespondencesService;
    pub fn client_replies(&self) -> &ClientRepliesService;
    pub fn client_visibility(&self) -> &ClientVisibilityService;
    
    // Platform features
    pub fn webhooks(&self) -> &WebhooksService;
    pub fn subscriptions(&self) -> &SubscriptionsService;
    pub fn events(&self) -> &EventsService;
    pub fn automation(&self) -> &AutomationService;
    pub fn boosts(&self) -> &BoostsService;
    
    // Organization
    pub fn people(&self) -> &PeopleService;
    pub fn templates(&self) -> &TemplatesService;
    
    // Integrations
    pub fn tools(&self) -> &ToolsService;
    pub fn hill_charts(&self) -> &HillChartsService;
    pub fn forwards(&self) -> &ForwardsService;
    
    // Reporting
    pub fn search(&self) -> &SearchService;
    pub fn reports(&self) -> &ReportsService;
    pub fn timesheets(&self) -> &TimesheetsService;
    pub fn recordings(&self) -> &RecordingsService;
    
    // Account
    pub fn account(&self) -> &AccountService;
}
```

### Helper Methods

```rust
impl AccountClient {
    /// Prepend account ID to a path.
    /// Pattern: "/buckets/{account_id}{path}"
    pub fn account_path(&self, path: &str) -> String;
    
    /// Download a file from a raw URL.
    /// Follows the two-hop download flow.
    pub async fn download_url(&self, raw_url: &str) -> Result<DownloadResult, DownloadError>;
    
    /// Get the account ID for this client.
    pub fn account_id(&self) -> i64;
}
```

### Verification

- `[unit]` Services are lazy-loaded (created on first access)
- `[unit]` Same service instance returned on repeated access
- `[unit]` `account_path("/projects.json")` returns `"/buckets/{id}/projects.json"`
- `[conformance]` All 40 services are accessible
- `[conformance]` Services use correct account context

---

## Thread Safety

```rust
// Both Client and AccountClient must be Send + Sync
unsafe impl Send for Client {}
unsafe impl Sync for Client {}

unsafe impl Send for AccountClient {}
unsafe impl Sync for AccountClient {}
```

- `[static]` Client and AccountClient implement Send
- `[static]` Client and AccountClient implement Sync
- `[manual]` Internal state uses appropriate synchronization primitives

---

## Clone Semantics

```rust
impl Clone for Client {
    /// Creates a shallow clone sharing the HTTP pool.
    fn clone(&self) -> Self;
}

impl Clone for AccountClient {
    /// Creates a shallow clone sharing the parent Client.
    fn clone(&self) -> Self;
}
```

- `[unit]` Cloned clients share HTTP connection pool
- `[unit]` Cloned clients share authentication state
- `[unit]` Closing original client does not affect clone
