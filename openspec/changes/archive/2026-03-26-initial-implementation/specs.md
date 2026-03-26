# Specs Delta: Initial Implementation

This document captures the specification deltas for the initial implementation change.

## Core Specifications Referenced

- `specs/core/error.md` - Error taxonomy and HTTP status mapping
- `specs/core/config.md` - Configuration builder and environment support
- `specs/core/security.md` - Security utilities (HTTPS, body limits, redaction)
- `specs/core/auth.md` - Authentication strategy contracts
- `specs/core/http.md` - HTTP client with retry and token refresh
- `specs/core/pagination.md` - ListResult and auto-pagination
- `specs/core/hooks.md` - Observability hooks
- `specs/core/client.md` - Client and AccountClient

## Implementation Constraints

### Error Module (src/error.rs)
- Must use `thiserror` for error derive
- `MAX_ERROR_MESSAGE_BYTES = 500`
- `MAX_ERROR_BODY_BYTES = 1MB`
- HTTP status mapping per `specs/core/error.md`:
  - 401 → AuthRequired
  - 403 → Forbidden  
  - 404 → NotFound
  - 429 → RateLimit (retryable)
  - 400/422 → Validation
  - 500/502/503/504 → Api (retryable)

### Configuration Module (src/config.rs)
- Defaults per `specs/core/config.md`:
  - `base_url = "https://3.basecampapi.com"`
  - `timeout = 30s`
  - `max_retries = 3`
  - `base_delay = 1s`
  - `max_jitter = 100ms`
  - `max_pages = 10000`
- Environment variables: `BASECAMP_BASE_URL`, `BASECAMP_TIMEOUT`, `BASECAMP_MAX_RETRIES`

### Security Module (src/security.rs)
- Per `specs/core/security.md`:
  - `MAX_RESPONSE_BODY_BYTES = 50MB`
  - `MAX_ERROR_BODY_BYTES = 1MB`
  - `MAX_ERROR_MESSAGE_BYTES = 500`
  - HTTPS required except for localhost (127.x.x.x, *.localhost)
  - CRLF injection prevention in headers
  - Same-origin validation for pagination links
  - Sensitive headers for redaction (per Python SDK):
    - `authorization`, `cookie`, `set-cookie`, `x-api-key`, `x-auth-token`, `x-csrf-token`

### Authentication (src/auth/)
- Per `specs/core/auth.md`:
  - `TokenProvider` trait with `refreshable()` and `refresh()`
  - `AuthStrategy` trait with `apply_auth()`
  - `BearerAuth` implementation
  - Token URL: `https://launchpad.37signals.com/authorization/token`
  - Legacy form format for Launchpad compatibility:
    - OAuth refresh uses `type=refresh` (not `grant_type=refresh_token`)
    - Per SPEC.md §4: "Uses legacy form format for Launchpad compatibility"
    - Python SDK sends: `type=refresh`, `refresh_token`, `client_id`, `client_secret`
    - TypeScript SDK sends both `type=refresh` AND `grant_type=refresh_token`

### HTTP Client (src/http/)
- Per `specs/core/http.md`:
  - Three-gate retry algorithm
  - Exponential backoff: `base * 2^(attempt-1) + jitter`
  - Single token refresh on 401
  - Retry-After header support (seconds and HTTP-date)

### Pagination (src/pagination.rs)
- Per `specs/core/pagination.md`:
  - RFC 5988 Link header parsing
  - Same-origin validation (SSRF prevention)
  - `max_pages` cap with truncated flag
  - `max_items` cap stops early

### Client Layer (src/client.rs)
- Per `specs/core/client.md`:
  - `Client` with `ClientBuilder`
  - `AccountClient` with lazy-loaded services
  - Exactly-one-of auth sources (access_token, token_provider, auth)
  - `Send + Sync` for thread safety

## Conformance Requirements

All conformance tests from `specs/CONFORMANCE.md` must pass:
- C-001 to C-003: Client behavior
- CF-001 to CF-002: Configuration
- A-001 to A-004: Authentication
- H-001 to H-011: HTTP client
- E-001 to E-008: Error mapping
- P-001 to P-006: Pagination
- S-001 to S-009: Security
