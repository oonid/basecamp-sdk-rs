# Conformance Test Requirements

This document lists all conformance test requirements extracted from the specifications.
These tests verify the SDK behaves correctly against the actual Basecamp API.

## Core Module

### Client (`core/client.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| C-001 | `for_account(id)` returns client with correct account context | `[conformance]` |
| C-002 | All 40 services are accessible | `[conformance]` |
| C-003 | Services use correct account context | `[conformance]` |

### Config (`core/config.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| CF-001 | Environment variables override defaults | `[conformance]` |
| CF-002 | Configuration affects HTTP client behavior | `[conformance]` |

### Auth (`core/auth.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| A-001 | Token URL is `https://launchpad.37signals.com/authorization/token` | `[conformance]` |
| A-002 | Uses legacy form format for Launchpad compatibility | `[conformance]` |
| A-003 | 401 response triggers token refresh flow | `[conformance]` |
| A-004 | Refreshed token is used for retry | `[conformance]` |

### HTTP (`core/http.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| H-001 | Base URL is `https://3.basecampapi.com` by default | `[conformance]` |
| H-002 | Max retries defaults to 3 | `[conformance]` |
| H-003 | Retry behavior matches specification | `[conformance]` |
| H-004 | Backoff timing is correct | `[conformance]` |
| H-005 | No infinite refresh loops | `[conformance]` |
| H-006 | Retry-After delays are respected | `[conformance]` |
| H-007 | Production API requires HTTPS | `[conformance]` |
| H-008 | Prevents header injection attacks | `[conformance]` |

### Error (`core/error.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| E-001 | HTTP status codes map correctly per specification | `[conformance]` |
| E-002 | Request IDs are preserved | `[conformance]` |

### Pagination (`core/pagination.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| P-001 | RFC 5988 Link header parsing compliant | `[conformance]` |
| P-002 | Prevents SSRF via Link header | `[conformance]` |
| P-003 | Handles real API responses | `[conformance]` |

### Security (`core/security.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| S-001 | Production API requires HTTPS | `[conformance]` |
| S-002 | Response body limited to 50 MB | `[conformance]` |
| S-003 | Error body limited to 1 MB | `[conformance]` |
| S-004 | Error messages truncated to 500 bytes | `[conformance]` |
| S-005 | Prevents open redirect attacks | `[conformance]` |
| S-006 | Prevents header injection | `[conformance]` |
| S-007 | All security constants match specification | `[conformance]` |

## Modules

### OAuth (`modules/oauth.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| OA-001 | Discovery endpoint is `/.well-known/oauth-authorization-server` | `[conformance]` |
| OA-002 | Launchpad URL is `https://launchpad.37signals.com` | `[conformance]` |
| OA-003 | RFC 7636 PKCE compliant | `[conformance]` |
| OA-004 | Parameters are URL-encoded | `[conformance]` |
| OA-005 | Token URL is `https://launchpad.37signals.com/authorization/token` | `[conformance]` |
| OA-006 | Uses legacy format for Launchpad compatibility | `[conformance]` |

### Webhooks (`modules/webhooks.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| W-001 | HMAC-SHA256 signature algorithm | `[conformance]` |
| W-002 | Header name is X-Basecamp-Signature | `[conformance]` |
| W-003 | Event kinds match API documentation | `[conformance]` |

### Download (`modules/download.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| D-001 | Two-hop flow follows redirects | `[conformance]` |
| D-002 | Second request is unauthenticated | `[conformance]` |
| D-003 | Handles signed CDN URLs | `[conformance]` |
| D-004 | HTTPS required | `[conformance]` |

## Generated

### Services (`generated/services.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| GS-001 | 40 services are generated | `[conformance]` |
| GS-002 | ~175 operations are implemented | `[conformance]` |
| GS-003 | Handles "basecamp", "campfire" system actors | `[conformance]` |

### Types (`generated/types.md`)

| ID | Requirement | Verification |
|----|-------------|--------------|
| GT-001 | ~150 types are generated | `[conformance]` |
| GT-002 | Field names match API | `[conformance]` |

## Manual Verification

| ID | Requirement | Module |
|----|-------------|--------|
| M-001 | Logs do not contain sensitive values | Security |
| M-002 | Output format matches specification | Hooks |
| M-003 | Tracing integration works | Hooks |

## Conformance Test Files

Reference conformance tests from the vendor SDK:

```
vendor/basecamp-sdk/conformance/tests/
├── auth.json           # Authentication behavior
├── error-mapping.json  # HTTP status → error code mapping
├── idempotency.json    # POST retry eligibility
├── integer-precision.json  # 64-bit ID handling
├── pagination.json     # Link header pagination
├── paths.json          # URL construction
├── retry.json          # Retry behavior
├── security.json       # HTTPS enforcement
└── status-codes.json   # HTTP status handling
```

These JSON files define test cases that should be ported to Rust tests.
