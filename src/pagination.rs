use reqwest::header::HeaderMap;
use std::iter::IntoIterator;
use std::slice::Iter;

#[derive(Debug, Clone, Default)]
pub struct ListMeta {
    pub total_count: Option<u64>,
    pub truncated: bool,
    pub next_url: Option<String>,
}

impl ListMeta {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_total_count(mut self, count: u64) -> Self {
        self.total_count = Some(count);
        self
    }

    pub fn with_truncated(mut self, truncated: bool) -> Self {
        self.truncated = truncated;
        self
    }

    pub fn with_next_url(mut self, url: impl Into<String>) -> Self {
        self.next_url = Some(url.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct ListResult<T> {
    pub items: Vec<T>,
    pub meta: ListMeta,
}

impl<T> ListResult<T> {
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            meta: ListMeta::new(),
        }
    }

    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            meta: ListMeta::new(),
        }
    }

    pub fn with_meta(items: Vec<T>, meta: ListMeta) -> Self {
        Self { items, meta }
    }

    pub fn has_more(&self) -> bool {
        self.meta.next_url.is_some()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> Iter<'_, T> {
        self.items.iter()
    }

    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> ListResult<U> {
        ListResult {
            items: self.items.into_iter().map(f).collect(),
            meta: self.meta,
        }
    }

    pub fn filter<P: Fn(&T) -> bool>(self, predicate: P) -> ListResult<T> {
        ListResult {
            items: self.items.into_iter().filter(predicate).collect(),
            meta: self.meta,
        }
    }

    pub fn take(self, n: usize) -> ListResult<T> {
        ListResult {
            items: self.items.into_iter().take(n).collect(),
            meta: self.meta,
        }
    }
}

impl<T> IntoIterator for ListResult<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<T> AsRef<[T]> for ListResult<T> {
    fn as_ref(&self) -> &[T] {
        &self.items
    }
}

pub fn parse_next_link(link_header: Option<&str>) -> Option<String> {
    let header = link_header?;

    for part in header.split(',') {
        let part = part.trim();

        if part.contains("rel=\"next\"") || part.contains("rel=next") {
            if let Some(url) = extract_url(part) {
                return Some(url);
            }
        }
    }

    None
}

fn extract_url(part: &str) -> Option<String> {
    let start = part.find('<')?;
    let end = part.find('>')?;

    if start < end {
        Some(part[start + 1..end].to_string())
    } else {
        None
    }
}

pub fn parse_total_count(headers: &HeaderMap) -> Option<u64> {
    headers
        .get("X-Total-Count")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod list_meta {
        use super::*;

        #[test]
        fn test_new_creates_default() {
            let meta = ListMeta::new();
            assert!(meta.total_count.is_none());
            assert!(!meta.truncated);
            assert!(meta.next_url.is_none());
        }

        #[test]
        fn test_default_is_same_as_new() {
            let meta = ListMeta::default();
            assert!(meta.total_count.is_none());
            assert!(!meta.truncated);
            assert!(meta.next_url.is_none());
        }

        #[test]
        fn test_with_total_count() {
            let meta = ListMeta::new().with_total_count(42);
            assert_eq!(meta.total_count, Some(42));
        }

        #[test]
        fn test_with_truncated() {
            let meta = ListMeta::new().with_truncated(true);
            assert!(meta.truncated);
        }

        #[test]
        fn test_with_next_url() {
            let meta = ListMeta::new().with_next_url("https://api.example.com/page2");
            assert_eq!(
                meta.next_url,
                Some("https://api.example.com/page2".to_string())
            );
        }

        #[test]
        fn test_builder_chain() {
            let meta = ListMeta::new()
                .with_total_count(100)
                .with_truncated(true)
                .with_next_url("https://api.example.com/next");
            assert_eq!(meta.total_count, Some(100));
            assert!(meta.truncated);
            assert!(meta.next_url.is_some());
        }
    }

    mod list_result {
        use super::*;

        #[test]
        fn test_empty() {
            let result: ListResult<i32> = ListResult::empty();
            assert!(result.is_empty());
            assert_eq!(result.len(), 0);
        }

        #[test]
        fn test_new_with_items() {
            let result = ListResult::new(vec![1, 2, 3]);
            assert_eq!(result.len(), 3);
            assert!(!result.is_empty());
        }

        #[test]
        fn test_with_meta() {
            let meta = ListMeta::new().with_total_count(42);
            let result = ListResult::with_meta(vec!["a", "b"], meta);
            assert_eq!(result.len(), 2);
            assert_eq!(result.meta.total_count, Some(42));
        }

        #[test]
        fn test_has_more_true() {
            let meta = ListMeta::new().with_next_url("https://example.com/next");
            let result = ListResult::with_meta(vec![1], meta);
            assert!(result.has_more());
        }

        #[test]
        fn test_has_more_false() {
            let result = ListResult::new(vec![1, 2, 3]);
            assert!(!result.has_more());
        }

        #[test]
        fn test_iter() {
            let result = ListResult::new(vec![1, 2, 3]);
            let collected: Vec<_> = result.iter().copied().collect();
            assert_eq!(collected, vec![1, 2, 3]);
        }

        #[test]
        fn test_into_iter() {
            let result = ListResult::new(vec![10, 20, 30]);
            let collected: Vec<_> = result.into_iter().collect();
            assert_eq!(collected, vec![10, 20, 30]);
        }

        #[test]
        fn test_as_ref() {
            let result = ListResult::new(vec!["a", "b", "c"]);
            assert_eq!(result.as_ref(), ["a", "b", "c"]);
        }

        #[test]
        fn test_clone() {
            let result = ListResult::new(vec![1, 2, 3]);
            let cloned = result.clone();
            assert_eq!(result.items, cloned.items);
        }

        #[test]
        fn test_debug() {
            let result = ListResult::new(vec![1, 2]);
            let debug_str = format!("{:?}", result);
            assert!(debug_str.contains("items"));
            assert!(debug_str.contains("meta"));
        }

        #[test]
        fn test_map() {
            let result = ListResult::new(vec![1, 2, 3]);
            let mapped = result.map(|x| x * 2);
            assert_eq!(mapped.items, vec![2, 4, 6]);
        }

        #[test]
        fn test_filter() {
            let result = ListResult::new(vec![1, 2, 3, 4, 5]);
            let filtered = result.filter(|x| *x % 2 == 0);
            assert_eq!(filtered.items, vec![2, 4]);
        }

        #[test]
        fn test_take() {
            let result = ListResult::new(vec![1, 2, 3, 4, 5]);
            let taken = result.take(3);
            assert_eq!(taken.items, vec![1, 2, 3]);
        }
    }

    mod parse_next_link_tests {
        use super::*;

        #[test]
        fn test_standard_link_header() {
            let header = r#"<https://api.example.com/page2>; rel="next""#;
            assert_eq!(
                parse_next_link(Some(header)),
                Some("https://api.example.com/page2".to_string())
            );
        }

        #[test]
        fn test_multiple_rels() {
            let header = r#"<https://api.example.com/first>; rel="first", <https://api.example.com/page2>; rel="next""#;
            assert_eq!(
                parse_next_link(Some(header)),
                Some("https://api.example.com/page2".to_string())
            );
        }

        #[test]
        fn test_next_not_first() {
            let header = r#"<https://api.example.com/prev>; rel="prev", <https://api.example.com/next>; rel="next", <https://api.example.com/last>; rel="last""#;
            assert_eq!(
                parse_next_link(Some(header)),
                Some("https://api.example.com/next".to_string())
            );
        }

        #[test]
        fn test_no_next_rel() {
            let header = r#"<https://api.example.com/prev>; rel="prev""#;
            assert_eq!(parse_next_link(Some(header)), None);
        }

        #[test]
        fn test_none_header() {
            assert_eq!(parse_next_link(None), None);
        }

        #[test]
        fn test_empty_header() {
            assert_eq!(parse_next_link(Some("")), None);
        }

        #[test]
        fn test_whitespace_in_header() {
            let header = r#"  <https://api.example.com/page2> ;  rel="next"  "#;
            assert_eq!(
                parse_next_link(Some(header)),
                Some("https://api.example.com/page2".to_string())
            );
        }

        #[test]
        fn test_unquoted_rel() {
            let header = r#"<https://api.example.com/page2>; rel=next"#;
            assert_eq!(
                parse_next_link(Some(header)),
                Some("https://api.example.com/page2".to_string())
            );
        }

        #[test]
        fn test_url_with_query_params() {
            let header = r#"<https://api.example.com/items?page=2&per_page=10>; rel="next""#;
            assert_eq!(
                parse_next_link(Some(header)),
                Some("https://api.example.com/items?page=2&per_page=10".to_string())
            );
        }

        #[test]
        fn test_malformed_no_angle_brackets() {
            let header = r#"rel="next""#;
            assert_eq!(parse_next_link(Some(header)), None);
        }

        #[test]
        fn test_malformed_empty_url() {
            let header = r#"<>; rel="next""#;
            assert_eq!(parse_next_link(Some(header)), Some("".to_string()));
        }

        #[test]
        fn test_rfc_5988_format() {
            let header = r#"<https://api.basecamp.com/999999999/buckets/123/messages.json?page=2>; rel="next", <https://api.basecamp.com/999999999/buckets/123/messages.json?page=1>; rel="first""#;
            assert_eq!(
                parse_next_link(Some(header)),
                Some(
                    "https://api.basecamp.com/999999999/buckets/123/messages.json?page=2"
                        .to_string()
                )
            );
        }
    }

    mod parse_total_count_tests {
        use super::*;
        use reqwest::header::HeaderValue;

        fn make_headers(key: &'static str, value: &'static str) -> HeaderMap {
            let mut headers = HeaderMap::new();
            headers.insert(key, HeaderValue::from_static(value));
            headers
        }

        #[test]
        fn test_present() {
            let headers = make_headers("X-Total-Count", "42");
            assert_eq!(parse_total_count(&headers), Some(42));
        }

        #[test]
        fn test_large_value() {
            let headers = make_headers("X-Total-Count", "1000000");
            assert_eq!(parse_total_count(&headers), Some(1000000));
        }

        #[test]
        fn test_missing() {
            let headers = HeaderMap::new();
            assert_eq!(parse_total_count(&headers), None);
        }

        #[test]
        fn test_non_numeric() {
            let headers = make_headers("X-Total-Count", "abc");
            assert_eq!(parse_total_count(&headers), None);
        }

        #[test]
        fn test_negative_value() {
            let headers = make_headers("X-Total-Count", "-5");
            assert_eq!(parse_total_count(&headers), None);
        }

        #[test]
        fn test_zero() {
            let headers = make_headers("X-Total-Count", "0");
            assert_eq!(parse_total_count(&headers), Some(0));
        }
    }
}
