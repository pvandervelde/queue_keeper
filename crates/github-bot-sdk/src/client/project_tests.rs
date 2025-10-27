//! Tests for project operations.

use super::*;

mod construction {
    use super::*;

    #[test]
    fn test_add_project_item_request() {
        todo!("Verify AddProjectV2ItemRequest with node ID")
    }
}

mod project_operations {
    use super::*;

    #[tokio::test]
    async fn test_list_organization_projects() {
        todo!("Mock: GET /orgs/:org/projects")
    }

    #[tokio::test]
    async fn test_list_user_projects() {
        todo!("Mock: GET /users/:username/projects")
    }

    #[tokio::test]
    async fn test_get_project_organization() {
        todo!("Mock: GET /orgs/:owner/projects/:number")
    }

    #[tokio::test]
    async fn test_get_project_user() {
        todo!("Mock: GET /users/:owner/projects/:number with fallback")
    }

    #[tokio::test]
    async fn test_get_project_not_found() {
        todo!("Mock: 404 response")
    }

    #[tokio::test]
    async fn test_add_item_to_project() {
        todo!("Mock: POST /projects/:id/items")
    }

    #[tokio::test]
    async fn test_add_item_already_in_project() {
        todo!("Mock: 422 validation error")
    }

    #[tokio::test]
    async fn test_remove_item_from_project() {
        todo!("Mock: DELETE /projects/:id/items/:item_id")
    }

    #[tokio::test]
    async fn test_remove_item_not_found() {
        todo!("Mock: 404 response")
    }
}

mod serialization {
    use super::*;

    #[test]
    fn test_project_v2_deserialize() {
        todo!("Verify ProjectV2 can be deserialized from GitHub API response")
    }

    #[test]
    fn test_project_owner_deserialize() {
        todo!("Verify ProjectOwner can be deserialized")
    }

    #[test]
    fn test_project_v2_item_deserialize() {
        todo!("Verify ProjectV2Item can be deserialized")
    }

    #[test]
    fn test_add_project_item_request_serialize() {
        todo!("Verify AddProjectV2ItemRequest serializes correctly")
    }
}

mod error_handling {
    use super::*;

    #[tokio::test]
    async fn test_project_not_found() {
        todo!("Mock: 404 response returns ApiError::NotFound")
    }

    #[tokio::test]
    async fn test_forbidden_access() {
        todo!("Mock: 403 response returns ApiError::Forbidden")
    }

    #[tokio::test]
    async fn test_organization_not_found() {
        todo!("Mock: 404 when org doesn't exist")
    }
}
