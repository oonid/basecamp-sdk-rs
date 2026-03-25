# Authentication Specification

## Overview

The authentication module provides flexible authentication strategies including
static bearer tokens, OAuth token providers with automatic refresh, and custom
authentication strategies.

## Trait: AuthStrategy

The core authentication trait that allows pluggable authentication.

```rust
use http::HeaderMap;

/// Authentication strategy trait.
pub trait AuthStrategy: Send + Sync {
    /// Add authentication headers to the request.
    /// 
    /// This method is called for every HTTP request.
    /// Implementations should add the appropriate Authorization header.
    fn authenticate(&self, headers: &mut HeaderMap);
}
```

## Trait: TokenProvider

Interface for token providers that support refresh.

```rust
/// Token provider trait for dynamic token management.
pub trait TokenProvider: Send + Sync {
    /// Get the current access token.
    fn access_token(&self) -> String;
    
    /// Attempt to refresh the token.
    /// Returns true if refresh was successful, false if not needed or failed.
    fn refresh(&self) -> impl Future<Output = bool> + Send;
    
    /// Whether this provider supports token refresh.
    fn refreshable(&self) -> bool;
}
```

## Implementation: StaticTokenProvider

Simple token provider that wraps a static string.

```rust
/// Static token provider with no refresh capability.
#[derive(Debug, Clone)]
pub struct StaticTokenProvider {
    token: String,
}

impl StaticTokenProvider {
    /// Create a new static token provider.
    pub fn new(token: impl Into<String>) -> Self;
}

impl TokenProvider for StaticTokenProvider {
    fn access_token(&self) -> String {
        self.token.clone()
    }
    
    async fn refresh(&self) -> bool {
        false
    }
    
    fn refreshable(&self) -> bool {
        false
    }
}
```

## Implementation: OAuthTokenProvider

Token provider with automatic refresh capability.

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

/// OAuth token with metadata.
#[derive(Debug, Clone)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub scope: Option<String>,
}

impl OAuthToken {
    /// Check if the token is expired or will expire soon.
    /// Uses a 60-second buffer by default.
    pub fn is_expired(&self, buffer_seconds: i64) -> bool;
}

/// Callback for token refresh events.
pub type OnRefreshCallback = Arc<dyn Fn(&OAuthToken) + Send + Sync>;

/// OAuth token provider with automatic refresh.
pub struct OAuthTokenProvider {
    client: reqwest::Client,
    token_url: String,
    client_id: String,
    client_secret: String,
    token: Arc<RwLock<OAuthToken>>,
    on_refresh: Option<OnRefreshCallback>,
}

impl OAuthTokenProvider {
    /// Create a new OAuth token provider.
    pub fn new(
        access_token: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self;
    
    /// Set the refresh token.
    pub fn with_refresh_token(self, token: impl Into<String>) -> Self;
    
    /// Set the token expiration time.
    pub fn with_expires_at(self, expires_at: DateTime<Utc>) -> Self;
    
    /// Set a callback for token refresh events.
    pub fn with_on_refresh(self, callback: OnRefreshCallback) -> Self;
    
    /// Get the current token.
    pub async fn current_token(&self) -> OAuthToken;
}

impl TokenProvider for OAuthTokenProvider {
    fn access_token(&self) -> String;
    async fn refresh(&self) -> bool;
    fn refreshable(&self) -> bool;
}
```

### Token Refresh Logic

```
ALGORITHM refresh:
1. IF refresh_token is None:
   RETURN false
2. IF currently refreshing (lock held):
   RETURN false  
3. POST to token_url with:
   - grant_type: "refresh_token"
   - refresh_token: <current refresh token>
   - client_id: <client id>
   - client_secret: <client secret>
4. IF response is 200:
   - Parse new token
   - Update stored token
   - Call on_refresh callback if set
   - RETURN true
5. ELSE:
   - RETURN false
```

- `[unit]` Refresh uses correct grant_type
- `[unit]` Refresh updates stored token on success
- `[unit]` Refresh calls on_refresh callback
- `[conformance]` Token URL is `https://launchpad.37signals.com/authorization/token`
- `[conformance]` Uses legacy form format for Launchpad compatibility

### Thread Safety

- `[static]` `OAuthTokenProvider` implements Send + Sync
- `[unit]` Concurrent access to token is safe (uses RwLock)
- `[unit]` Prevents concurrent refresh attempts

## Implementation: BearerAuth

The default authentication strategy using bearer tokens.

```rust
/// Bearer token authentication strategy.
pub struct BearerAuth {
    provider: Arc<dyn TokenProvider>,
}

impl BearerAuth {
    /// Create a new bearer auth with a token provider.
    pub fn new(provider: impl TokenProvider + 'static) -> Self;
    
    /// Create from a static token string.
    pub fn from_token(token: impl Into<String>) -> Self;
}

impl AuthStrategy for BearerAuth {
    fn authenticate(&self, headers: &mut HeaderMap) {
        let token = self.provider.access_token();
        headers.insert(
            http::header::AUTHORIZATION,
            format!("Bearer {}", token).parse().unwrap(),
        );
    }
}
```

## HTTP Client Integration

The HTTP client must handle 401 responses with token refresh:

```
ALGORITHM handle_401:
1. IF response.status == 401 AND token_provider.refreshable():
   IF token_provider.refresh():
     Retry request ONCE with new token
2. RETURN original error
```

- `[unit]` 401 triggers refresh when provider is refreshable
- `[unit]` Refresh is attempted only once per request
- `[unit]` Successful refresh triggers retry
- `[conformance]` Failed refresh returns original 401 error

## Verification

- `[unit]` `StaticTokenProvider` returns configured token
- `[unit]` `StaticTokenProvider::refreshable()` returns false
- `[unit]` `OAuthTokenProvider::is_expired()` respects buffer
- `[unit]` `BearerAuth` adds correct Authorization header
- `[unit]` Header value is "Bearer {token}"
- `[conformance]` 401 response triggers token refresh flow
- `[conformance]` Refreshed token is used for retry
