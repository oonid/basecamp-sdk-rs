# Download Specification

## Overview

The download module handles file downloads from Basecamp, implementing a two-hop
download flow for authenticated file access.

## Two-Hop Download Flow

Basecamp file downloads require a two-step process:

1. **First request**: Authenticated request to Basecamp API returns a redirect
2. **Second request**: Unauthenticated request to signed CDN URL returns the file

```
Client                Basecamp API              CDN
  |                        |                      |
  |--- GET /uploads/123 -->|                      |
  |                        |                      |
  |<-- 302 Location: CDN --|                      |
  |                        |                      |
  |--- GET signed URL ------------------------->|
  |                        |                      |
  |<------------------ file data ----------------|
```

## Interface: DownloadResult

```rust
/// Result of a file download.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// The file content.
    pub body: Vec<u8>,
    
    /// Content-Type header value.
    pub content_type: String,
    
    /// Content-Length (body size).
    pub content_length: u64,
    
    /// Suggested filename from Content-Disposition.
    pub filename: String,
}

impl DownloadResult {
    /// Get the body as a string (if valid UTF-8).
    pub fn as_text(&self) -> Option<&str>;
    
    /// Write the file to a path.
    pub async fn write_to_file(&self, path: impl AsRef<Path>) -> Result<(), DownloadError>;
}
```

## Download Functions

```rust
/// Download a file from a raw URL.
/// 
/// Implements the two-hop download flow:
/// 1. Authenticated request to Basecamp (follows redirect)
/// 2. Unauthenticated fetch from signed CDN URL
pub async fn download(
    http_client: &HttpClient,
    config: &Config,
    raw_url: &str,
) -> Result<DownloadResult, DownloadError>;

/// Download and write directly to a file.
/// More memory-efficient for large files.
pub async fn download_to_file(
    http_client: &HttpClient,
    config: &Config,
    raw_url: &str,
    path: impl AsRef<Path>,
) -> Result<DownloadResult, DownloadError>;
```

## URL Rewriting

```rust
/// Rewrite a raw URL to use the configured base URL.
/// 
/// Basecamp URLs may be returned as absolute URLs with the API host.
/// This rewrites them to use the configured base URL.
pub fn rewrite_url(raw_url: &str, base_url: &str) -> String {
    // If URL is already absolute with correct host, use as-is
    if raw_url.starts_with(base_url) {
        return raw_url.to_string();
    }
    
    // Extract path from raw URL
    if let Ok(parsed) = url::Url::parse(raw_url) {
        // Reconstruct with base URL
        format!("{}{}", base_url.trim_end_matches('/'), parsed.path())
    } else {
        raw_url.to_string()
    }
}
```

- `[unit]` URLs with matching host pass through
- `[unit]` URLs with different host are rewritten
- `[conformance]` Preserves path and query parameters

## Filename Extraction

```rust
/// Extract filename from a URL or Content-Disposition header.
pub fn filename_from_url(url: &str) -> String {
    let parsed = url::Url::parse(url).ok();
    
    // Try to get filename from path
    if let Some(ref p) = parsed {
        if let Some(segment) = p.path_segments().and_then(|s| s.last()) {
            if !segment.is_empty() {
                return urlencoding::decode(segment)
                    .unwrap_or_else(|_| segment.to_string())
                    .into_owned();
            }
        }
    }
    
    // Fallback to timestamp-based name
    format!("download-{}", chrono::Utc::now().timestamp())
}

/// Extract filename from Content-Disposition header.
/// 
/// Handles formats:
/// - attachment; filename="file.txt"
/// - attachment; filename=file.txt
/// - inline; filename="file.txt"
pub fn filename_from_content_disposition(header: &str) -> Option<String> {
    // Parse Content-Disposition header
    for part in header.split(';') {
        let part = part.trim();
        if part.starts_with("filename=") {
            let filename = part.strip_prefix("filename=").unwrap();
            
            // Remove quotes if present
            let filename = filename.trim_matches('"');
            
            // Decode percent-encoding
            return Some(
                urlencoding::decode(filename)
                    .unwrap_or_else(|_| filename.to_string())
                    .into_owned()
            );
        }
    }
    
    None
}
```

- `[unit]` Extracts filename from URL path
- `[unit]` Falls back to timestamp name
- `[unit]` Parses quoted Content-Disposition
- `[unit]` Parses unquoted Content-Disposition
- `[unit]` Decodes percent-encoded filenames

## Download Algorithm

```rust
/// Download implementation.
async fn download_impl(
    http_client: &HttpClient,
    config: &Config,
    raw_url: &str,
) -> Result<DownloadResult, DownloadError> {
    // 1. Rewrite URL to use base URL
    let url = rewrite_url(raw_url, &config.base_url);
    
    // 2. Make authenticated request (will follow redirect)
    let response = http_client
        .get_absolute(&url, None)
        .await?;
    
    // 3. Get final URL (may be signed CDN URL after redirect)
    let final_url = response.url().to_string();
    
    // 4. Check if we need to make a second request
    // If the response is a redirect to a different origin,
    // we need to fetch from the new URL without auth
    if response.status().is_redirection() {
        let location = response
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .ok_or(DownloadError::MissingRedirect)?;
        
        // Make unauthenticated request to CDN
        let cdn_response = http_client
            .get_absolute_no_auth(location, None)
            .await?;
        
        return extract_download_result(cdn_response).await;
    }
    
    // 5. If no redirect, extract from direct response
    extract_download_result(response).await;
}

async fn extract_download_result(response: Response) -> Result<DownloadResult, DownloadError> {
    // Extract content type
    let content_type = response
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    
    // Extract filename
    let filename = response
        .headers()
        .get("Content-Disposition")
        .and_then(|v| v.to_str().ok())
        .and_then(filename_from_content_disposition)
        .unwrap_or_else(|| filename_from_url(response.url().as_str()));
    
    // Read body with size limit
    let body = response.bytes().await?;
    
    // Validate size
    if body.len() > MAX_RESPONSE_BODY_BYTES {
        return Err(DownloadError::FileTooLarge {
            size: body.len(),
            max: MAX_RESPONSE_BODY_BYTES,
        });
    }
    
    Ok(DownloadResult {
        body: body.to_vec(),
        content_type,
        content_length: body.len() as u64,
        filename,
    })
}
```

## Streaming Download

For large files, streaming download avoids loading entire file into memory:

```rust
use tokio::io::AsyncWrite;

/// Stream download directly to a writer.
pub async fn download_to_writer(
    http_client: &HttpClient,
    config: &Config,
    raw_url: &str,
    writer: &mut impl AsyncWrite + Unpin,
) -> Result<DownloadMetadata, DownloadError>;

/// Metadata for streamed download.
#[derive(Debug, Clone)]
pub struct DownloadMetadata {
    pub content_type: String,
    pub content_length: Option<u64>,
    pub filename: String,
    pub bytes_written: u64,
}
```

## AccountClient Integration

```rust
impl AccountClient {
    /// Download a file from a raw URL.
    /// 
    /// # Example
    /// ```rust
    /// let result = account.download_url("https://3.basecampapi.com/...").await?;
    /// std::fs::write(&result.filename, &result.body)?;
    /// ```
    pub async fn download_url(&self, raw_url: &str) -> Result<DownloadResult, DownloadError>;
}
```

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("HTTP error: {0}")]
    Http(#[from] HttpError),
    
    #[error("File too large: {size} bytes (max: {max})")]
    FileTooLarge { size: usize, max: usize },
    
    #[error("Missing redirect location")]
    MissingRedirect,
    
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

## Security Considerations

```rust
/// Validate download URL.
fn validate_download_url(url: &str, base_url: &str) -> Result<(), DownloadError> {
    // 1. Check URL is valid
    let parsed = url::Url::parse(url)
        .map_err(|e| DownloadError::InvalidUrl(e.to_string()))?;
    
    // 2. Initial request must be to base URL origin
    if !same_origin(url, base_url) {
        return Err(DownloadError::InvalidUrl(
            "Download URL must be same-origin as base URL".to_string()
        ));
    }
    
    // 3. HTTPS required (except localhost)
    require_https(url)?;
    
    Ok(())
}
```

- `[unit]` Validates URL format
- `[unit]` Enforces same-origin for initial request
- `[unit]` Allows cross-origin redirect to CDN
- `[conformance]` HTTPS required

## Verification

- `[unit]` URL rewriting works correctly
- `[unit]` Filename extraction from URL
- `[unit]` Filename extraction from Content-Disposition
- `[unit]` Size limit enforced
- `[unit]` Streaming download works
- `[conformance]` Two-hop flow follows redirects
- `[conformance]` Second request is unauthenticated
- `[conformance]` Handles signed CDN URLs
