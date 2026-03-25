# Pagination Specification

## Overview

The pagination module handles list operations that return multiple pages of results.
Basecamp uses Link header-based pagination (RFC 5988).

## Interface: ListResult

```rust
use serde::de::DeserializeOwned;

/// Paginated list result.
#[derive(Debug, Clone)]
pub struct ListResult<T> {
    /// The items on the current page.
    pub items: Vec<T>,
    
    /// Metadata about the result set.
    pub meta: ListMeta,
}

/// Metadata about a paginated result.
#[derive(Debug, Clone, Default)]
pub struct ListMeta {
    /// Total count of items (if available from X-Total-Count header).
    pub total_count: Option<u64>,
    
    /// Whether results were truncated due to max_pages limit.
    pub truncated: bool,
    
    /// URL for the next page (if available).
    pub next_url: Option<String>,
}

impl<T> ListResult<T> {
    /// Create an empty result.
    pub fn empty() -> Self;
    
    /// Check if there are more pages.
    pub fn has_more(&self) -> bool;
    
    /// Get the number of items on this page.
    pub fn len(&self) -> usize;
    
    /// Check if this page is empty.
    pub fn is_empty(&self) -> bool;
}

impl<T> IntoIterator for ListResult<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}
```

## Link Header Parsing

```rust
/// Parse the Link header to extract the next page URL.
/// 
/// Link header format:
/// <https://api.example.com/page=2>; rel="next"
/// 
/// Returns None if no "next" link is found.
pub fn parse_next_link(link_header: Option<&str>) -> Option<String> {
    let header = link_header?;
    
    for link in header.split(',') {
        let link = link.trim();
        
        // Extract URL and rel
        let (url_part, rel_part) = split_link_parts(link)?;
        
        // Check if this is the "next" link
        if rel_part.contains("rel=\"next\"") || rel_part.contains("rel=next") {
            // Extract URL from angle brackets
            if let Some(url) = extract_url(url_part) {
                return Some(url);
            }
        }
    }
    
    None
}

fn split_link_parts(link: &str) -> Option<(&str, &str)> {
    let semicolon = link.find(';')?;
    Some((&link[..semicolon], &link[semicolon + 1..]))
}

fn extract_url(url_part: &str) -> Option<String> {
    let start = url_part.find('<')?;
    let end = url_part.find('>')?;
    Some(url_part[start + 1..end].to_string())
}
```

- `[unit]` Parses single next link
- `[unit]` Parses multiple links and finds "next"
- `[unit]` Returns None for missing Link header
- `[unit]` Returns None for Link without next
- `[conformance]` RFC 5988 compliant

## Total Count Parsing

```rust
/// Parse X-Total-Count header.
pub fn parse_total_count(headers: &HeaderMap) -> Option<u64> {
    headers
        .get("X-Total-Count")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
}
```

- `[unit]` Parses valid count
- `[unit]` Returns None for missing header
- `[unit]` Returns None for invalid number

## Pagination Algorithm

```rust
impl HttpClient {
    /// Fetch all pages of a paginated resource.
    pub async fn get_paginated<T: DeserializeOwned>(
        &self,
        path: &str,
        params: Option<&[(&str, &str)]>,
    ) -> Result<ListResult<T>, HttpError> {
        let mut all_items = Vec::new();
        let mut current_url = Some(self.build_url(path));
        let mut total_count = None;
        let mut page_count = 0;
        let max_pages = self.config.max_pages;
        
        while let Some(url) = current_url.take() {
            page_count += 1;
            
            if page_count > max_pages {
                return Ok(ListResult {
                    items: all_items,
                    meta: ListMeta {
                        total_count,
                        truncated: true,
                        next_url: Some(url),
                    },
                });
            }
            
            // Notify hooks
            if let Some(ref hooks) = self.hooks {
                hooks.on_paginate(&url, page_count);
            }
            
            let response = self.get_absolute(&url, params).await?;
            
            // Extract total count from first page
            if total_count.is_none() {
                total_count = parse_total_count(response.headers());
            }
            
            // Parse items
            let items: Vec<T> = response.json().await?;
            all_items.extend(items);
            
            // Check for next page
            let link_header = response.headers()
                .get("Link")
                .and_then(|v| v.to_str().ok());
            current_url = parse_next_link(link_header);
        }
        
        Ok(ListResult {
            items: all_items,
            meta: ListMeta {
                total_count,
                truncated: false,
                next_url: None,
            },
        })
    }
}
```

- `[unit]` Fetches all pages
- `[unit]` Respects max_pages limit
- `[unit]` Sets truncated flag when limit hit
- `[unit]` Extracts total_count from first page
- `[unit]` Calls hooks on each page
- `[conformance]` Stops when no next link

## Same-Origin Validation

To prevent SSRF attacks, pagination must validate that next URLs are from the same origin:

```rust
/// Check if two URLs have the same origin.
pub fn same_origin(a: &str, b: &str) -> bool {
    let Ok(url_a) = url::Url::parse(a) else { return false };
    let Ok(url_b) = url::Url::parse(b) else { return false };
    
    url_a.scheme() == url_b.scheme()
        && url_a.host() == url_b.host()
        && url_a.port() == url_b.port()
}

/// Validate next URL is same origin as base URL.
fn validate_next_url(base_url: &str, next_url: &str) -> Result<(), HttpError> {
    if !same_origin(base_url, next_url) {
        return Err(HttpError::SecurityViolation {
            reason: format!(
                "Pagination URL has different origin: {}",
                next_url
            ),
        });
    }
    Ok(())
}
```

- `[unit]` Same origin URLs pass validation
- `[unit]` Different origin URLs fail
- `[unit]` Invalid URLs fail
- `[conformance]` Prevents SSRF via Link header

## Stream-Based Pagination

For memory-efficient iteration over large result sets:

```rust
use futures::Stream;

/// Stream pages of items.
pub fn stream_pages<T: DeserializeOwned>(
    &self,
    path: &str,
    params: Option<&[(&str, &str)]>,
) -> impl Stream<Item = Result<Vec<T>, HttpError>> + '_ {
    async_stream::try_stream! {
        let mut current_url = Some(self.build_url(path));
        let mut page_count = 0;
        
        while let Some(url) = current_url.take() {
            page_count += 1;
            
            if page_count > self.config.max_pages {
                break;
            }
            
            let response = self.get_absolute(&url, params).await?;
            let items: Vec<T> = response.json().await?;
            
            let link_header = response.headers()
                .get("Link")
                .and_then(|v| v.to_str().ok());
            current_url = parse_next_link(link_header);
            
            yield items;
        }
    }
}
```

- `[unit]` Stream yields pages one at a time
- `[unit]` Stream terminates on last page
- `[unit]` Stream respects max_pages

## Convenience Methods

```rust
impl<T> ListResult<T> {
    /// Map items to a different type.
    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> ListResult<U> {
        ListResult {
            items: self.items.into_iter().map(f).collect(),
            meta: self.meta,
        }
    }
    
    /// Filter items by predicate.
    pub fn filter<P: Fn(&T) -> bool>(self, predicate: P) -> ListResult<T> {
        ListResult {
            items: self.items.into_iter().filter(predicate).collect(),
            meta: self.meta,
        }
    }
    
    /// Take first N items.
    pub fn take(self, n: usize) -> ListResult<T> {
        ListResult {
            items: self.items.into_iter().take(n).collect(),
            meta: self.meta,
        }
    }
}

impl<T: Clone> ListResult<T> {
    /// Iterate over items by reference.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }
}
```

## Verification

- `[unit]` `parse_next_link` extracts correct URL
- `[unit]` `parse_total_count` extracts correct count
- `[unit]` Pagination stops at last page
- `[unit]` `max_pages` limit is enforced
- `[unit]` `truncated` flag is set correctly
- `[unit]` Same-origin validation works
- `[conformance]` Link header format is correct
- `[conformance]` Handles real API responses
