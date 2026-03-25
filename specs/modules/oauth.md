# OAuth 2.0 Specification

## Overview

The OAuth module provides utilities for implementing OAuth 2.0 authentication flows
with Basecamp/Launchpad, including PKCE support and token management.

## Configuration

### OAuthConfig

```rust
/// OAuth 2.0 server configuration.
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// Issuer URL.
    pub issuer: String,
    
    /// Authorization endpoint URL.
    pub authorization_endpoint: String,
    
    /// Token endpoint URL.
    pub token_endpoint: String,
    
    /// Registration endpoint URL (optional).
    pub registration_endpoint: Option<String>,
    
    /// Supported scopes (optional).
    pub scopes_supported: Option<Vec<String>>,
}

/// Default Launchpad base URL.
pub const LAUNCHPAD_BASE_URL: &str = "https://launchpad.37signals.com";
```

## Discovery (RFC 8414)

```rust
/// Discover OAuth configuration from a base URL.
/// 
/// Fetches the OAuth Authorization Server Metadata from:
/// `{base_url}/.well-known/oauth-authorization-server`
pub async fn discover(base_url: &str) -> Result<OAuthConfig, OAuthError>;

/// Discover OAuth configuration from Launchpad.
pub async fn discover_launchpad() -> Result<OAuthConfig, OAuthError> {
    discover(LAUNCHPAD_BASE_URL).await
}
```

- `[unit]` Parses valid discovery document
- `[unit]` Returns error on HTTP failure
- `[conformance]` Endpoint is `/.well-known/oauth-authorization-server`
- `[conformance]` Launchpad URL is `https://launchpad.37signals.com`

## PKCE (RFC 7636)

```rust
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

/// PKCE challenge and verifier pair.
#[derive(Debug, Clone)]
pub struct PKCE {
    /// The verifier string (43 characters).
    /// Used in the token exchange.
    pub verifier: String,
    
    /// The challenge string (43 characters).
    /// SHA-256 hash of verifier, base64url-encoded.
    pub challenge: String,
    
    /// The challenge method ("S256").
    pub method: String,
}

impl PKCE {
    /// The challenge method for S256.
    pub const METHOD_S256: &'static str = "S256";
    
    /// Generate a new PKCE pair.
    pub fn generate() -> Self;
}

/// Generate a cryptographically random string.
/// Returns a 43-character base64url-encoded string.
pub fn generate_random_string() -> String;

/// Generate a code verifier (43 characters).
pub fn generate_verifier() -> String;

/// Generate a code challenge from a verifier.
pub fn generate_challenge(verifier: &str) -> String;

/// Generate a state parameter (43 characters).
pub fn generate_state() -> String;
```

### PKCE Algorithm

```
ALGORITHM generate_pkce:
1. verifier = random 32 bytes, base64url-encoded (no padding)
   Result: 43 characters
2. challenge = SHA256(verifier), base64url-encoded (no padding)
   Result: 43 characters
3. RETURN PKCE { verifier, challenge, method: "S256" }
```

- `[unit]` Verifier is 43 characters
- `[unit]` Challenge is 43 characters
- `[unit]` Method is "S256"
- `[unit]` Challenge is SHA256 of verifier
- `[conformance]` RFC 7636 compliant

## Authorization URL Builder

```rust
/// Build an authorization URL for the OAuth flow.
/// 
/// # Arguments
/// * `endpoint` - Authorization endpoint URL
/// * `client_id` - OAuth client ID
/// * `redirect_uri` - Redirect URI (must match registered URI)
/// * `state` - State parameter for CSRF protection
/// * `pkce` - Optional PKCE pair
/// * `scope` - Optional scope string
pub fn build_authorization_url(
    endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    pkce: Option<&PKCE>,
    scope: Option<&str>,
) -> String;
```

### URL Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `response_type` | Yes | Always "code" |
| `client_id` | Yes | OAuth client ID |
| `redirect_uri` | Yes | Registered redirect URI |
| `state` | Yes | CSRF protection token |
| `code_challenge` | With PKCE | PKCE challenge |
| `code_challenge_method` | With PKCE | Always "S256" |
| `scope` | No | Space-separated scopes |

- `[unit]` URL contains required parameters
- `[unit]` PKCE parameters added when provided
- `[unit]` Scope parameter added when provided
- `[conformance]` Parameters are URL-encoded

## Token Exchange

### OAuthToken

```rust
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// OAuth 2.0 token response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    /// The access token.
    pub access_token: String,
    
    /// Token type (always "Bearer").
    #[serde(default = "default_token_type")]
    pub token_type: String,
    
    /// Refresh token (optional, for offline access).
    pub refresh_token: Option<String>,
    
    /// Token lifetime in seconds.
    pub expires_in: Option<i64>,
    
    /// Token expiration timestamp (calculated from expires_in).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<OffsetDateTime>,
    
    /// Granted scopes.
    pub scope: Option<String>,
}

fn default_token_type() -> String {
    "Bearer".to_string()
}

impl OAuthToken {
    /// Check if the token is expired.
    /// Uses a buffer to account for clock skew.
    pub fn is_expired(&self, buffer_seconds: i64) -> bool;
    
    /// Calculate expires_at from expires_in.
    pub fn with_expires_at(self) -> Self;
}
```

### Token Exchange Functions

```rust
/// Exchange an authorization code for tokens.
/// 
/// # Arguments
/// * `token_endpoint` - Token endpoint URL
/// * `code` - Authorization code from redirect
/// * `redirect_uri` - Same redirect URI used in authorization
/// * `client_id` - OAuth client ID
/// * `client_secret` - OAuth client secret (optional for public clients)
/// * `code_verifier` - PKCE verifier (required if PKCE was used)
/// * `use_legacy_format` - Use legacy form format for Launchpad
pub async fn exchange_code(
    token_endpoint: &str,
    code: &str,
    redirect_uri: &str,
    client_id: &str,
    client_secret: Option<&str>,
    code_verifier: Option<&str>,
    use_legacy_format: bool,
) -> Result<OAuthToken, OAuthError>;

/// Refresh an access token.
/// 
/// # Arguments
/// * `token_endpoint` - Token endpoint URL
/// * `refresh_token` - The refresh token
/// * `client_id` - OAuth client ID
/// * `client_secret` - OAuth client secret (optional for public clients)
/// * `use_legacy_format` - Use legacy form format for Launchpad
pub async fn refresh_token(
    token_endpoint: &str,
    refresh_token: &str,
    client_id: &str,
    client_secret: Option<&str>,
    use_legacy_format: bool,
) -> Result<OAuthToken, OAuthError>;
```

### Token Request Format

Standard format:
```
POST /authorization/token
Content-Type: application/x-www-form-urlencoded

grant_type=authorization_code&code=...&redirect_uri=...&client_id=...&client_secret=...
```

Legacy format (Launchpad):
```
POST /authorization/token
Content-Type: application/x-www-form-urlencoded

type=web_server&code=...&redirect_uri=...&client_id=...&client_secret=...
```

- `[unit]` Exchange sends correct grant_type
- `[unit]` Refresh sends refresh_token
- `[unit]` Legacy format uses "type" instead of "grant_type"
- `[conformance]` Token URL is `https://launchpad.37signals.com/authorization/token`
- `[conformance]` Uses legacy format for Launchpad compatibility

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("Discovery failed: {0}")]
    DiscoveryFailed(String),
    
    #[error("Token exchange failed: {error}")]
    TokenExchangeFailed {
        error: String,
        description: Option<String>,
    },
    
    #[error("Invalid token response: {0}")]
    InvalidTokenResponse(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("PKCE error: {0}")]
    PKCE(String),
}
```

## Complete Flow Example

```rust
async fn oauth_flow_example() -> Result<(), OAuthError> {
    // 1. Discover OAuth configuration
    let config = discover_launchpad().await?;
    
    // 2. Generate PKCE and state
    let pkce = PKCE::generate();
    let state = generate_state();
    
    // 3. Build authorization URL
    let auth_url = build_authorization_url(
        &config.authorization_endpoint,
        "YOUR_CLIENT_ID",
        "https://your-app.com/callback",
        &state,
        Some(&pkce),
        None,
    );
    
    // 4. Redirect user to auth_url
    // ... user authorizes and is redirected back with code ...
    
    // 5. Exchange code for tokens
    let token = exchange_code(
        &config.token_endpoint,
        "AUTHORIZATION_CODE",
        "https://your-app.com/callback",
        "YOUR_CLIENT_ID",
        Some("YOUR_CLIENT_SECRET"),
        Some(&pkce.verifier),
        true, // Use legacy format for Launchpad
    ).await?;
    
    // 6. Use token
    println!("Access token: {}", token.access_token);
    
    // 7. Refresh when expired
    if token.is_expired(60) {
        if let Some(refresh) = token.refresh_token {
            let new_token = refresh_token(
                &config.token_endpoint,
                refresh,
                "YOUR_CLIENT_ID",
                Some("YOUR_CLIENT_SECRET"),
                true,
            ).await?;
        }
    }
    
    Ok(())
}
```

## Verification

- `[unit]` Discovery returns valid config
- `[unit]` PKCE generation creates valid pair
- `[unit]` Authorization URL has all required params
- `[unit]` Token exchange parses response
- `[unit]` Token refresh sends correct request
- `[unit]` `is_expired()` respects buffer
- `[conformance]` Discovery endpoint is correct
- `[conformance]` Legacy format for Launchpad
- `[conformance]` Token URL is correct
