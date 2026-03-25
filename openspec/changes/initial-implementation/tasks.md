# Tasks: Initial Implementation

> **Note**: Update `specs.md` when new implementation constraints or constants are added.

## Phase 1: Foundation (TDD)

### T1: Error Types
- [x] Create `src/error.rs` with error taxonomy using `thiserror`
- [x] Write unit tests for all error codes
- [x] Write unit tests for HTTP status → error mapping
- [x] Write unit tests for message truncation (500 bytes)
- [x] Write unit tests for request ID extraction
- [x] Write unit tests for retry-after parsing (seconds and HTTP-date)
- [x] Verify `cargo test` passes

### T2: Configuration
- [x] Create `src/config.rs` with `Config` struct and defaults
- [x] Create `ConfigBuilder` with validation
- [x] Write unit tests for default values
- [x] Write unit tests for builder validation errors
- [x] Write unit tests for `from_env()` parsing
- [x] Add `max_items: Option<u32>` config option
- [x] Verify `cargo test` passes

### T3: Security Utilities
- [x] Create `src/security.rs` with security helpers
- [x] Write unit tests for `require_https()` 
- [x] Write unit tests for `is_localhost()` (including *.localhost)
- [x] Write unit tests for `truncate()` (UTF-8 boundary)
- [x] Write unit tests for `same_origin()`
- [x] Write unit tests for `check_body_size()`
- [x] Write unit tests for `redact_headers()`
- [x] Write unit tests for `contains_crlf()` (header injection)
- [x] Verify `cargo test` passes

## Phase 2: Authentication (TDD)

### T4: Auth Traits
- [x] Create `src/auth/mod.rs`
- [x] Create `src/auth/strategy.rs` with `AuthStrategy` trait
- [x] Create `src/auth/token.rs` with `TokenProvider` trait
- [x] Verify traits are `Send + Sync`
- [x] Verify `cargo test` passes

### T5: Static Token Provider
- [x] Implement `StaticTokenProvider` in `src/auth/token.rs`
- [x] Write unit tests for token retrieval
- [x] Write unit tests verifying `refreshable()` returns false
- [x] Write unit tests verifying `refresh()` returns false
- [x] Verify `cargo test` passes

### T6: OAuth Token Provider
- [x] Create `src/auth/oauth.rs` with `OAuthTokenProvider`
- [x] Write unit tests for `is_expired()` with buffer
- [x] Write unit tests for token refresh (mock HTTP)
- [x] Write unit tests for refresh callback invocation
- [x] Write unit tests for thread safety (concurrent access)
- [x] Verify `cargo test` passes

### T7: Bearer Auth
- [x] Create `src/auth/bearer.rs` with `BearerAuth`
- [x] Write unit tests for Authorization header format
- [x] Write unit tests for header injection with dynamic token
- [x] Verify `cargo test` passes

## Phase 3: HTTP Layer (TDD)

### T8: HTTP Client
- [x] Create `src/http/mod.rs`
- [x] Create `src/http/client.rs` with `HttpClient`
- [x] Write integration tests with `wiremock` for GET
- [x] Write integration tests for POST, PUT, DELETE
- [x] Write integration tests for `get_absolute()`
- [x] Write integration tests for timeout enforcement
- [x] Verify `cargo test` passes

### T9: Retry Logic
- [x] Create `src/http/retry.rs` with backoff calculation
- [x] Write unit tests for backoff formula: `base * 2^(attempt-1) + jitter`
- [x] Write integration tests for GET retry on 503
- [x] Write integration tests for 429 with Retry-After
- [x] Write integration tests for POST NOT retrying by default
- [x] Write integration tests for idempotent POST retrying
- [x] Write integration tests for non-retryable errors (404, 403)
- [x] Verify `cargo test` passes

### T10: Token Refresh on 401
- [x] Write integration tests for 401 → refresh → retry
- [x] Write integration tests for single refresh attempt
- [x] Write integration tests for failed refresh returning original error
- [x] Verify `cargo test` passes

## Phase 4: Pagination (TDD)

### T11: Pagination Types
- [x] Create `src/pagination.rs`
- [x] Write unit tests for `parse_next_link()` with various formats
- [x] Write unit tests for `parse_total_count()`
- [x] Write unit tests for `ListResult<T>` iteration
- [x] Verify `cargo test` passes

### T12: Auto-Pagination
- [x] Add `get_paginated()` to `HttpClient`
- [x] Add minimal hooks stub (`src/hooks.rs`) for T12 compilation
- [x] Add `with_hooks()` builder method to `HttpClient`
- [x] Add `resolve_url()` for relative URL resolution (SSRF-safe)
- [x] Write integration tests for multi-page fetch
- [x] Write integration tests for max_pages limit (truncated flag)
- [x] Write integration tests for max_items cap
- [x] Write integration tests for same-origin validation (SSRF prevention)
- [x] Write integration tests for protocol downgrade rejection
- [x] Verify `cargo test` passes

## Phase 5: Client Layer (TDD)

### T13: Hooks
- [x] Create `src/hooks.rs` with `BasecampHooks` trait (minimal stub created in T12)
- [x] Define `OperationInfo`, `RequestInfo`, `OperationResult`, `RequestResult` (structs only)
- [x] Implement `safe_hook()` panic catching
- [x] Implement `chain_hooks()` composition
- [x] Implement `ConsoleHooks`, `TimingHooks`
- [x] Write unit tests for hook invocation
- [x] Verify `cargo test` passes

### T14: Client
- [ ] Create `src/client.rs` with `Client` and `ClientBuilder`
- [ ] Write unit tests for `new(access_token)` convenience
- [ ] Write unit tests for builder with various auth sources
- [ ] Write unit tests for ambiguous auth detection (returns error)
- [ ] Write unit tests for custom user agent
- [ ] Write unit tests for `authorization()` service access
- [ ] Verify `cargo test` passes

### T15: AccountClient
- [ ] Add `AccountClient` to `src/client.rs`
- [ ] Write unit tests for `for_account(id)` construction
- [ ] Write unit tests for `account_path()` path construction
- [ ] Write unit tests for lazy service loading
- [ ] Write unit tests for `Clone` sharing HTTP pool
- [ ] Verify `cargo test` passes

## Phase 6: Conformance Tests

### T16: Port Conformance Tests
- [ ] Create `tests/conformance/mod.rs`
- [ ] Create `tests/conformance/error_mapping.rs` (port from JSON)
- [ ] Create `tests/conformance/retry.rs` (port from JSON)
- [ ] Create `tests/conformance/pagination.rs` (port from JSON)
- [ ] Create `tests/conformance/security.rs` (port from JSON)
- [ ] Create `tests/conformance/status_codes.rs` (port from JSON)
- [ ] Verify all conformance tests pass

## Phase 7: Code Quality

### T17: Documentation
- [ ] Add rustdoc comments to all public types
- [ ] Add examples to crate root (`src/lib.rs`)
- [ ] Add README.md usage examples

### T18: CI Setup
- [ ] Add `cargo fmt --check` verification
- [ ] Add `cargo clippy -- -D warnings` verification
- [ ] Add `cargo llvm-cov` coverage report
- [ ] Target >80% code coverage

---

## Definition of Done

- [ ] All tasks completed
- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt --check`)
- [ ] Coverage >80% (`cargo llvm-cov`)
- [ ] Conformance tests pass
- [ ] Documentation complete
- [ ] PR reviewed and merged
