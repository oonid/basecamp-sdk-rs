# Proposal: Initial Implementation

## Summary

Implement the core Basecamp SDK Rust library following Specification-Driven Development (SDD) with Test-Driven Development (TDD) methodology. This change establishes the foundational infrastructure needed for the complete SDK.

## Motivation

The project currently has complete specifications in `specs/` but only a scaffold implementation in `src/`. The Python SDK in `vendor/basecamp-sdk/python/` provides a reference implementation for porting.

## Scope

### In Scope

1. **Core Infrastructure**
   - Error types with taxonomy and HTTP status mapping
   - Configuration with builder pattern and environment support
   - Security utilities (HTTPS enforcement, body limits, header redaction)

2. **Authentication**
   - `AuthStrategy` trait
   - `TokenProvider` trait
   - `StaticTokenProvider` implementation
   - `OAuthTokenProvider` implementation
   - `BearerAuth` strategy

3. **HTTP Layer**
   - `HttpClient` with retry logic
   - Exponential backoff with jitter
   - Token refresh on 401
   - Request/response hooks

4. **Pagination**
   - `ListResult` with metadata
   - Link header parsing (RFC 5988)
   - Auto-pagination with safety caps
   - Same-origin validation

5. **Client Layer**
   - `Client` and `ClientBuilder`
   - `AccountClient` with lazy-loaded services
   - Observability hooks

6. **Conformance Tests**
   - Port tests from vendor SDK
   - Wiremock-based integration tests

### Out of Scope

- Generated services and types (separate change)
- OAuth flow helpers (separate change)
- Webhook handling (separate change)
- File downloads (separate change)

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Error handling | `thiserror` derive | Idiomatic, zero-cost |
| Async runtime | tokio | Industry standard |
| HTTP client | reqwest | Proven, feature-rich |
| Lazy loading | `once_cell::sync::OnceCell` | Thread-safe, no macros |
| Mock server | wiremock | Async-native |

## Success Criteria

1. All unit tests pass (`cargo test`)
2. All conformance tests pass
3. Code coverage >80%
4. No clippy warnings
5. Code formatted (`cargo fmt --check`)

## Risks

| Risk | Mitigation |
|------|------------|
| Token refresh race conditions | RwLock, single refresh attempt |
| Pagination SSRF | Same-origin validation |
| Large response bodies | Streaming with size limits |

## References

- `specs/SPEC.md` - Specification index
- `specs/core/*.md` - Core module specifications
- `vendor/basecamp-sdk/python/` - Reference implementation
- `vendor/basecamp-sdk/conformance/tests/` - Conformance tests
