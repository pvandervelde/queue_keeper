# Pagination Interface Specification

**Module**: `github-bot-sdk::client::pagination`
**File**: `crates/github-bot-sdk/src/client/pagination.rs`
**Dependencies**: `ApiError`, HTTP response headers

## Overview

Pagination support for GitHub API list operations that return multiple pages of results. GitHub uses RFC 8288 Link headers for pagination.

## Architectural Location

**Layer**: Infrastructure adapter (HTTP response handling)
**Purpose**: Parse and navigate paginated responses
**Pattern**: Iterator-based async stream

## Core Types

### PagedResponse

Generic wrapper for paginated API responses.

```rust
#[derive(Debug, Clone)]
pub struct PagedResponse<T> {
    /// Data items from current page
    pub items: Vec<T>,
    /// Link to next page (if available)
    pub next_page: Option<String>,
    /// Link to previous page (if available)
    pub prev_page: Option<String>,
    /// Link to first page
    pub first_page: Option<String>,
    /// Link to last page
    pub last_page: Option<String>,
}
```

### PageInfo

Parsed pagination information from Link header.

```rust
#[derive(Debug, Clone, Default)]
pub struct PageInfo {
    pub next: Option<String>,
    pub prev: Option<String>,
    pub first: Option<String>,
    pub last: Option<String>,
}
```

## Pagination Functions

### Parse Link Header

```rust
/// Parse GitHub's Link header into structured pagination info.
///
/// # Arguments
///
/// * `link_header` - Raw Link header value
///
/// # Returns
///
/// Returns `PageInfo` with extracted URLs.
///
/// # Format
///
/// GitHub Link headers follow RFC 5988:
/// ```text
/// <https://api.github.com/repos/o/r/issues?page=2>; rel="next",
/// <https://api.github.com/repos/o/r/issues?page=5>; rel="last"
/// ```
///
/// # Examples
///
/// ```rust
/// let header = r#"<https://api.github.com/issues?page=2>; rel="next""#;
/// let info = parse_link_header(header);
/// assert!(info.next.is_some());
/// ```
pub fn parse_link_header(link_header: &str) -> PageInfo;
```

**Implementation Notes**:

- Split on commas to get individual links
- Extract URL between `<` and `>`
- Extract rel value from `rel="..."`
- Build `PageInfo` struct with found links

### Extract Page Number

```rust
/// Extract page number from a URL.
///
/// # Arguments
///
/// * `url` - Full URL containing page query parameter
///
/// # Returns
///
/// Returns page number if found, None otherwise.
///
/// # Examples
///
/// ```rust
/// let url = "https://api.github.com/repos/o/r/issues?page=3";
/// assert_eq!(extract_page_number(url), Some(3));
/// ```
pub fn extract_page_number(url: &str) -> Option<u32>;
```

## Usage with InstallationClient

### List with Pagination

Example integration with list operations:

```rust
impl InstallationClient {
    /// List issues with pagination support.
    ///
    /// Returns first page of results with pagination info.
    pub async fn list_issues_paginated(
        &self,
        owner: &str,
        repo: &str,
        page: Option<u32>,
    ) -> Result<PagedResponse<Issue>, ApiError> {
        let path = if let Some(p) = page {
            format!("repos/{}/{}/issues?page={}", owner, repo, p)
        } else {
            format!("repos/{}/{}/issues", owner, repo)
        };

        let response = self.get(&path).await?;

        // Parse Link header for pagination
        let page_info = response
            .headers()
            .get("Link")
            .and_then(|h| h.to_str().ok())
            .map(parse_link_header)
            .unwrap_or_default();

        // Parse response body
        let items: Vec<Issue> = response.json().await
            .map_err(|e| ApiError::ParseError {
                message: format!("Failed to parse issues: {}", e),
            })?;

        Ok(PagedResponse {
            items,
            next_page: page_info.next,
            prev_page: page_info.prev,
            first_page: page_info.first,
            last_page: page_info.last,
        })
    }
}
```

### Iterate All Pages

Helper to fetch all pages:

```rust
/// Fetch all pages of a paginated endpoint.
///
/// # Warning
///
/// This loads all results into memory. Use with caution for
/// repositories with many items.
///
/// # Examples
///
/// ```rust
/// let mut all_issues = Vec::new();
/// let mut page = Some(1);
///
/// while let Some(p) = page {
///     let response = client.list_issues_paginated("owner", "repo", Some(p)).await?;
///     all_issues.extend(response.items);
///     page = response.next_page.and_then(|url| extract_page_number(&url));
/// }
/// ```
pub async fn fetch_all_pages<T, F, Fut>(
    mut fetch_page: F,
) -> Result<Vec<T>, ApiError>
where
    F: FnMut(Option<u32>) -> Fut,
    Fut: std::future::Future<Output = Result<PagedResponse<T>, ApiError>>,
{
    let mut all_items = Vec::new();
    let mut page = Some(1);

    while let Some(p) = page {
        let response = fetch_page(Some(p)).await?;
        all_items.extend(response.items);
        page = response.next_page.and_then(|url| extract_page_number(&url));
    }

    Ok(all_items)
}
```

## GitHub API Pagination Details

### Per Page Limit

- Default: 30 items per page
- Maximum: 100 items per page
- Set via `per_page` query parameter

### Page Numbering

- Pages are 1-indexed (first page is `page=1`)
- Omitting `page` parameter returns first page

### Link Header Format

```text
Link: <https://api.github.com/resource?page=2>; rel="next",
      <https://api.github.com/resource?page=5>; rel="last",
      <https://api.github.com/resource?page=1>; rel="first",
      <https://api.github.com/resource?page=1>; rel="prev"
```

## Usage Examples

### Fetch Single Page

```rust
let response = client.list_issues_paginated("owner", "repo", Some(1)).await?;

println!("Got {} issues", response.items.len());

if let Some(next_url) = response.next_page {
    println!("More pages available: {}", next_url);
}
```

### Fetch All Pages

```rust
let all_issues = fetch_all_pages(|page| {
    client.list_issues_paginated("owner", "repo", page)
}).await?;

println!("Total issues: {}", all_issues.len());
```

## Implementation Notes

### Performance Considerations

- Each page requires separate API request
- Fetching all pages can be slow for large datasets
- Consider using pagination for user-facing operations
- Cache results when appropriate

### Error Handling

- Network errors during pagination should be retried
- Use exponential backoff for rate limit errors
- Partial results are lost on error (no resumption)

### Testing Strategy

- Mock Link headers in test responses
- Test parsing various Link header formats
- Test edge cases (no Link header, empty pages, single page)
- Verify page number extraction

## Future Enhancements

### Async Stream API

Future: Implement async stream for page iteration:

```rust
// Future enhancement (requires async stream support)
pub async fn list_issues_stream(
    &self,
    owner: &str,
    repo: &str,
) -> impl Stream<Item = Result<Issue, ApiError>> {
    // Stream that yields items from each page
}
```

### Cursor-Based Pagination

Some endpoints use cursor-based pagination. Add support when needed.

## References

- RFC 5988: [Web Linking](https://tools.ietf.org/html/rfc5988)
- GitHub API: [Pagination](https://docs.github.com/en/rest/guides/using-pagination-in-the-rest-api)
