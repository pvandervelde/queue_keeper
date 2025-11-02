//! Tests for pagination support.

use super::*;

mod construction {
    use super::*;

    #[test]
    fn test_pagination_default() {
        let pagination = Pagination::default();

        assert_eq!(pagination.page, Some(1));
        assert_eq!(pagination.per_page, Some(30)); // GitHub's default
        assert_eq!(pagination.next, None);
        assert_eq!(pagination.prev, None);
        assert_eq!(pagination.first, None);
        assert_eq!(pagination.last, None);
    }

    #[test]
    fn test_paged_response_creation() {
        let items = vec![1, 2, 3];
        let pagination = Pagination::default();

        let response = PagedResponse {
            items: items.clone(),
            total_count: Some(100),
            pagination: pagination.clone(),
        };

        assert_eq!(response.items, items);
        assert_eq!(response.total_count, Some(100));
        assert_eq!(response.pagination.page, Some(1));
    }
}

mod pagination_methods {
    use super::*;

    #[test]
    fn test_has_next_true() {
        let mut pagination = Pagination::default();
        pagination.next = Some("https://api.github.com/repos/o/r/issues?page=2".to_string());

        assert!(pagination.has_next());
    }

    #[test]
    fn test_has_next_false() {
        let pagination = Pagination::default();

        assert!(!pagination.has_next());
    }

    #[test]
    fn test_has_prev_true() {
        let mut pagination = Pagination::default();
        pagination.prev = Some("https://api.github.com/repos/o/r/issues?page=1".to_string());

        assert!(pagination.has_prev());
    }

    #[test]
    fn test_has_prev_false() {
        let pagination = Pagination::default();

        assert!(!pagination.has_prev());
    }

    #[test]
    fn test_next_page() {
        let mut pagination = Pagination::default();
        pagination.page = Some(3);

        assert_eq!(pagination.next_page(), Some(4));
    }

    #[test]
    fn test_prev_page() {
        let mut pagination = Pagination::default();
        pagination.page = Some(3);

        assert_eq!(pagination.prev_page(), Some(2));
    }

    #[test]
    fn test_prev_page_at_first_page() {
        let mut pagination = Pagination::default();
        pagination.page = Some(1);

        assert_eq!(pagination.prev_page(), None);
    }
}

mod link_header_parsing {
    use super::*;

    #[test]
    fn test_parse_link_header_with_next() {
        let header = r#"<https://api.github.com/resource?page=2>; rel="next""#;
        let pagination = parse_link_header(Some(header));

        assert_eq!(
            pagination.next,
            Some("https://api.github.com/resource?page=2".to_string())
        );
        assert_eq!(pagination.prev, None);
        assert_eq!(pagination.first, None);
        assert_eq!(pagination.last, None);
    }

    #[test]
    fn test_parse_link_header_with_all_links() {
        let header = r#"<https://api.github.com/resource?page=3>; rel="next", <https://api.github.com/resource?page=1>; rel="prev", <https://api.github.com/resource?page=1>; rel="first", <https://api.github.com/resource?page=10>; rel="last""#;
        let pagination = parse_link_header(Some(header));

        assert_eq!(
            pagination.next,
            Some("https://api.github.com/resource?page=3".to_string())
        );
        assert_eq!(
            pagination.prev,
            Some("https://api.github.com/resource?page=1".to_string())
        );
        assert_eq!(
            pagination.first,
            Some("https://api.github.com/resource?page=1".to_string())
        );
        assert_eq!(
            pagination.last,
            Some("https://api.github.com/resource?page=10".to_string())
        );
    }

    #[test]
    fn test_parse_link_header_empty() {
        let pagination = parse_link_header(Some(""));

        assert_eq!(pagination.next, None);
        assert_eq!(pagination.prev, None);
        assert_eq!(pagination.first, None);
        assert_eq!(pagination.last, None);
    }

    #[test]
    fn test_parse_link_header_none() {
        let pagination = parse_link_header(None);

        assert_eq!(pagination.next, None);
        assert_eq!(pagination.prev, None);
        assert_eq!(pagination.first, None);
        assert_eq!(pagination.last, None);
    }

    #[test]
    fn test_parse_link_header_malformed() {
        let header = "not a valid link header";
        let pagination = parse_link_header(Some(header));

        // Should handle gracefully and return default (no links)
        assert_eq!(pagination.next, None);
        assert_eq!(pagination.prev, None);
    }

    #[test]
    fn test_parse_link_header_with_spaces() {
        // GitHub's actual format with spaces
        let header = r#"<https://api.github.com/resource?page=2>; rel="next" , <https://api.github.com/resource?page=5>; rel="last""#;
        let pagination = parse_link_header(Some(header));

        assert_eq!(
            pagination.next,
            Some("https://api.github.com/resource?page=2".to_string())
        );
        assert_eq!(
            pagination.last,
            Some("https://api.github.com/resource?page=5".to_string())
        );
    }
}

mod serialization {
    use super::*;
    use serde_json;

    #[test]
    fn test_pagination_serialize() {
        let mut pagination = Pagination::default();
        pagination.next = Some("https://api.github.com/resource?page=2".to_string());
        pagination.page = Some(1);
        pagination.per_page = Some(30);

        let json = serde_json::to_string(&pagination).expect("Failed to serialize");

        assert!(json.contains("\"next\""));
        assert!(json.contains("\"page\""));
        assert!(json.contains("\"per_page\""));
    }

    #[test]
    fn test_pagination_deserialize() {
        let json = r#"{
            "next": "https://api.github.com/resource?page=2",
            "prev": null,
            "first": "https://api.github.com/resource?page=1",
            "last": "https://api.github.com/resource?page=10",
            "page": 1,
            "per_page": 30
        }"#;

        let pagination: Pagination = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(
            pagination.next,
            Some("https://api.github.com/resource?page=2".to_string())
        );
        assert_eq!(pagination.prev, None);
        assert_eq!(
            pagination.first,
            Some("https://api.github.com/resource?page=1".to_string())
        );
        assert_eq!(
            pagination.last,
            Some("https://api.github.com/resource?page=10".to_string())
        );
        assert_eq!(pagination.page, Some(1));
        assert_eq!(pagination.per_page, Some(30));
    }

    #[test]
    fn test_paged_response_serialize() {
        let response = PagedResponse {
            items: vec![1, 2, 3],
            total_count: Some(100),
            pagination: Pagination::default(),
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize");

        assert!(json.contains("\"items\""));
        assert!(json.contains("\"total_count\""));
        assert!(json.contains("\"pagination\""));
    }

    #[test]
    fn test_paged_response_deserialize() {
        let json = r#"{
            "items": [1, 2, 3],
            "total_count": 100,
            "pagination": {
                "next": null,
                "prev": null,
                "first": null,
                "last": null,
                "page": 1,
                "per_page": 30
            }
        }"#;

        let response: PagedResponse<i32> =
            serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(response.items, vec![1, 2, 3]);
        assert_eq!(response.total_count, Some(100));
        assert_eq!(response.pagination.page, Some(1));
    }
}

mod extract_page_number {
    use super::*;

    #[test]
    fn test_extract_page_number_with_page_param() {
        let url = "https://api.github.com/repos/o/r/issues?page=3";
        let page = super::super::extract_page_number(url);

        assert_eq!(page, Some(3));
    }

    #[test]
    fn test_extract_page_number_with_multiple_params() {
        let url = "https://api.github.com/repos/o/r/issues?state=open&page=5&per_page=100";
        let page = super::super::extract_page_number(url);

        assert_eq!(page, Some(5));
    }

    #[test]
    fn test_extract_page_number_without_page_param() {
        let url = "https://api.github.com/repos/o/r/issues?state=open";
        let page = super::super::extract_page_number(url);

        assert_eq!(page, None);
    }

    #[test]
    fn test_extract_page_number_invalid_url() {
        let url = "not a valid url";
        let page = super::super::extract_page_number(url);

        assert_eq!(page, None);
    }

    #[test]
    fn test_extract_page_number_invalid_page_value() {
        let url = "https://api.github.com/repos/o/r/issues?page=notanumber";
        let page = super::super::extract_page_number(url);

        assert_eq!(page, None);
    }

    #[test]
    fn test_extract_page_number_page_zero() {
        let url = "https://api.github.com/repos/o/r/issues?page=0";
        let page = super::super::extract_page_number(url);

        // GitHub pages are 1-indexed, but we should parse 0 if present
        assert_eq!(page, Some(0));
    }
}
