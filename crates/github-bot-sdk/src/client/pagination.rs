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

#[cfg(test)]
#[path = "pagination_tests.rs"]
mod tests;
