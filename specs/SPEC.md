# Basecamp SDK Rust - Specification Index

This directory contains the contract specifications for the Basecamp SDK Rust implementation using Specification-Driven Development (SDD).

## Specification Structure

```
specs/
├── SPEC.md                    # This file - specification index
├── CONFORMANCE.md             # Conformance test requirements
├── TESTING.md                 # Testing guide
├── core/                      # Core SDK specifications
│   ├── client.md              # Client and AccountClient contracts
│   ├── config.md              # Configuration contract
│   ├── auth.md                # Authentication strategy contracts
│   ├── http.md                # HTTP client and retry contracts
│   ├── error.md               # Error taxonomy and mapping
│   ├── pagination.md          # Pagination contracts
│   ├── hooks.md               # Observability hooks contracts
│   └── security.md            # Security requirements
├── modules/                   # Feature module specifications
│   ├── oauth.md               # OAuth 2.0 flow contracts
│   ├── webhooks.md            # Webhook handling contracts
│   └── download.md            # File download contracts
└── generated/                 # Generated code specifications
    ├── services.md            # Service generation patterns
    └── types.md               # Type generation patterns
```

## Quick Reference

| Specification | Description | Key Components |
|--------------|-------------|----------------|
| `core/client.md` | SDK entry points | `Client`, `AccountClient`, `ClientBuilder` |
| `core/config.md` | SDK configuration | `Config`, `ConfigBuilder`, env vars |
| `core/auth.md` | Authentication | `AuthStrategy`, `TokenProvider`, `BearerAuth` |
| `core/http.md` | HTTP transport | `HttpClient`, retry algorithm, backoff |
| `core/error.md` | Error handling | `BasecampError`, `ErrorCode`, status mapping |
| `core/pagination.md` | List operations | `ListResult`, `ListMeta`, Link header parsing |
| `core/hooks.md` | Observability | `BasecampHooks`, `OperationInfo`, `RequestInfo` |
| `core/security.md` | Security | HTTPS enforcement, body limits, header redaction |
| `modules/oauth.md` | OAuth 2.0 | PKCE, discovery, token exchange |
| `modules/webhooks.md` | Webhooks | `WebhookReceiver`, HMAC verification |
| `modules/download.md` | File downloads | Two-hop flow, streaming |
| `generated/services.md` | Services | 40 generated services, ~175 operations |
| `generated/types.md` | Types | ~150 generated types |

## Verification Tags

Specifications use verification tags to indicate test requirements:

| Tag | Meaning | Example |
|-----|---------|---------|
| `[conformance]` | Verified against real API behavior | "401 maps to auth_required error" |
| `[static]` | Verified by compiler/type system | "Client implements Send + Sync" |
| `[manual]` | Requires code review | "Logs do not contain sensitive values" |
| `[unit]` | Verified by unit tests | "parse_next_link extracts correct URL" |

## Design Principles

1. **API-First**: All contracts are defined before implementation
2. **Type Safety**: Leverage Rust's type system for correctness
3. **Async-First**: Native async/await support throughout
4. **Zero Cost**: Abstractions should not add runtime overhead
5. **Ergonomic**: API should be intuitive and idiomatic Rust

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Application                          │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────┐
│                      Client                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Config    │  │    Auth     │  │       Hooks         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────┬───────────────────────────────────┘
                          │ for_account(id)
┌─────────────────────────▼───────────────────────────────────┐
│                   AccountClient                             │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐           │
│  │projects │ │  todos  │ │messages │ │  ...    │ (40 svcs) │
│  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘           │
└───────┼──────────┼──────────┼──────────┼───────────────────┘
        │          │          │          │
┌───────▼──────────▼──────────▼──────────▼───────────────────┐
│                     HTTP Client                             │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐           │
│  │  Retry  │ │  Auth   │ │  Hooks  │ │Security │           │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘           │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────┐
│                    Basecamp API                             │
└─────────────────────────────────────────────────────────────┘
```

## Version Compatibility

| Component | Version |
|-----------|---------|
| SDK | 0.1.0 |
| Basecamp API | 2026-01-26 |
| Minimum Rust | 1.75 |

## Reference Sources

| Source | Location | Purpose |
|--------|----------|---------|
| Python SDK | `vendor/basecamp-sdk/python/` | Reference implementation |
| OpenAPI Spec | `vendor/basecamp-sdk/openapi.json` | API definitions |
| Conformance Tests | `vendor/basecamp-sdk/conformance/tests/` | Test cases |
| Smithy Spec | `vendor/basecamp-sdk/spec/basecamp.smithy` | Source of truth |

## Implementation Order

Recommended implementation sequence:

1. **Core Infrastructure**
   - `error.rs` - Error types
   - `config.rs` - Configuration
   - `security.rs` - Security utilities

2. **Authentication**
   - `auth/strategy.rs` - AuthStrategy trait
   - `auth/bearer.rs` - BearerAuth
   - `auth/token.rs` - TokenProvider

3. **HTTP Layer**
   - `http/client.rs` - HTTP client wrapper
   - `http/retry.rs` - Retry logic
   - `pagination.rs` - Pagination

4. **Client Layer**
   - `client.rs` - Client and AccountClient
   - `hooks.rs` - Observability hooks

5. **Generated Code**
   - Code generators for services and types
   - `generated/services/*.rs`
   - `generated/types/*.rs`

6. **Feature Modules**
   - `oauth/` - OAuth 2.0 flows
   - `webhooks/` - Webhook handling
   - `download.rs` - File downloads
