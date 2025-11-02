// GENERATED FROM: github-bot-sdk-specs/interfaces/pagination.md
// Pagination support for GitHub API

use serde::{Deserialize, Serialize};

/// Paginated response wrapper.
///
/// GitHub API returns paginated results with Link headers for navigation.
///
/// See github-bot-sdk-specs/interfaces/pagination.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedResponse<T> {
    /// Items in this page
    pub items: Vec<T>,

    /// Total count (if available from API)
    pub total_count: Option<u64>,

    /// Pagination metadata
    pub pagination: Pagination,
}

/// Pagination metadata extracted from Link headers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// URL for next page (if available)
    pub next: Option<String>,

    /// URL for previous page (if available)
    pub prev: Option<String>,

    /// URL for first page (if available)
    pub first: Option<String>,

    /// URL for last page (if available)
    pub last: Option<String>,

    /// Current page number
    pub page: Option<u64>,

    /// Items per page
    pub per_page: Option<u64>,
}

impl Pagination {
    /// Check if there are more pages available.
    pub fn has_next(&self) -> bool {
        self.next.is_some()
    }

    /// Check if there are previous pages available.
    pub fn has_prev(&self) -> bool {
        self.prev.is_some()
    }

    /// Get the next page number.
    pub fn next_page(&self) -> Option<u64> {
        self.page.map(|p| p + 1)
    }

    /// Get the previous page number.
    pub fn prev_page(&self) -> Option<u64> {
        self.page
            .and_then(|p| if p > 1 { Some(p - 1) } else { None })
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            next: None,
            prev: None,
            first: None,
            last: None,
            page: Some(1),
            per_page: Some(30), // GitHub's default
        }
    }
}

impl<T> PagedResponse<T> {
    /// Check if there are more pages available.
    ///
    /// Returns true if a next page URL exists in the pagination metadata.
    pub fn has_next(&self) -> bool {
        self.pagination.has_next()
    }

    /// Get the next page number from the pagination URL.
    ///
    /// Extracts the page number from the next page URL if available.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use github_bot_sdk::client::{PagedResponse, Pagination};
    ///
    /// let mut pagination = Pagination::default();
    /// pagination.next = Some("https://api.github.com/repos/o/r/issues?page=3".to_string());
    ///
    /// let response = PagedResponse {
    ///     items: vec![1, 2, 3],
    ///     total_count: None,
    ///     pagination,
    /// };
    ///
    /// assert_eq!(response.next_page_number(), Some(3));
    /// ```
    pub fn next_page_number(&self) -> Option<u32> {
        self.pagination
            .next
            .as_ref()
            .and_then(|url| extract_page_number(url))
    }

    /// Check if this is the last page.
    ///
    /// Returns true if there is no next page available.
    pub fn is_last_page(&self) -> bool {
        !self.has_next()
    }
}

/// Parse pagination metadata from Link header.
///
/// GitHub returns Link headers like:
/// `<https://api.github.com/resource?page=2>; rel="next", <https://api.github.com/resource?page=5>; rel="last"`
///
/// See github-bot-sdk-specs/interfaces/pagination.md
pub fn parse_link_header(link_header: Option<&str>) -> Pagination {
    let mut pagination = Pagination::default();

    if let Some(header) = link_header {
        for link in header.split(',') {
            let parts: Vec<&str> = link.split(';').collect();
            if parts.len() != 2 {
                continue;
            }

            let url = parts[0]
                .trim()
                .trim_start_matches('<')
                .trim_end_matches('>');
            let rel = parts[1]
                .trim()
                .trim_start_matches("rel=\"")
                .trim_end_matches('"');

            match rel {
                "next" => pagination.next = Some(url.to_string()),
                "prev" => pagination.prev = Some(url.to_string()),
                "first" => pagination.first = Some(url.to_string()),
                "last" => pagination.last = Some(url.to_string()),
                _ => {}
            }
        }
    }

    pagination
}

/// Extract page number from a URL.
///
/// Parses the query string to find the `page` parameter.
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
/// use github_bot_sdk::client::extract_page_number;
///
/// let url = "https://api.github.com/repos/o/r/issues?page=3";
/// assert_eq!(extract_page_number(url), Some(3));
/// ```
pub fn extract_page_number(url: &str) -> Option<u32> {
    // Parse the URL and extract query parameters
    url.split('?')
        .nth(1) // Get query string part
        .and_then(|query| {
            // Split by & to get individual parameters
            query.split('&').find_map(|param| {
                // Split by = to get key-value pairs
                let mut parts = param.split('=');
                let key = parts.next()?;
                let value = parts.next()?;

                // Check if this is the page parameter
                if key == "page" {
                    // Parse the value as u32
                    value.parse::<u32>().ok()
                } else {
                    None
                }
            })
        })
}

#[cfg(test)]
#[path = "pagination_tests.rs"]
mod tests;
