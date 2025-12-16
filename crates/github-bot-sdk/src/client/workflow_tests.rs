//! Tests for workflow operations.

mod construction {

    #[test]
    #[ignore = "TODO: Verify TriggerWorkflowRequest with only ref"]
    fn test_trigger_workflow_request_minimal() {
        todo!("Verify TriggerWorkflowRequest with only ref")
    }

    #[test]
    #[ignore = "TODO: Verify TriggerWorkflowRequest with inputs map"]
    fn test_trigger_workflow_request_with_inputs() {
        todo!("Verify TriggerWorkflowRequest with inputs map")
    }
}

mod workflow_operations {

    #[tokio::test]
    #[ignore = "TODO: Mock: GET /repos/:owner/:repo/actions/workflows"]
    async fn test_list_workflows() {
        todo!("Mock: GET /repos/:owner/:repo/actions/workflows")
    }

    #[tokio::test]
    #[ignore = "TODO: Mock: GET /repos/:owner/:repo/actions/workflows/:id"]
    async fn test_get_workflow() {
        todo!("Mock: GET /repos/:owner/:repo/actions/workflows/:id")
    }

    #[tokio::test]
    #[ignore = "TODO: Mock: 404 response"]
    async fn test_get_workflow_not_found() {
        todo!("Mock: 404 response")
    }

    #[tokio::test]
    #[ignore = "TODO: Mock: POST /repos/:owner/:repo/actions/workflows/:id/dispatches"]
    async fn test_trigger_workflow() {
        todo!("Mock: POST /repos/:owner/:repo/actions/workflows/:id/dispatches")
    }

    #[tokio::test]
    #[ignore = "TODO: Mock: POST with inputs in request body"]
    async fn test_trigger_workflow_with_inputs() {
        todo!("Mock: POST with inputs in request body")
    }
}

mod workflow_run_operations {

    #[tokio::test]
    #[ignore = "TODO: Mock: GET /repos/:owner/:repo/actions/workflows/:id/runs"]
    async fn test_list_workflow_runs() {
        todo!("Mock: GET /repos/:owner/:repo/actions/workflows/:id/runs")
    }

    #[tokio::test]
    #[ignore = "TODO: Mock: GET /repos/:owner/:repo/actions/runs/:id"]
    async fn test_get_workflow_run() {
        todo!("Mock: GET /repos/:owner/:repo/actions/runs/:id")
    }

    #[tokio::test]
    #[ignore = "TODO: Mock: POST /repos/:owner/:repo/actions/runs/:id/cancel"]
    async fn test_cancel_workflow_run() {
        todo!("Mock: POST /repos/:owner/:repo/actions/runs/:id/cancel")
    }

    #[tokio::test]
    #[ignore = "TODO: Mock: POST /repos/:owner/:repo/actions/runs/:id/rerun"]
    async fn test_rerun_workflow_run() {
        todo!("Mock: POST /repos/:owner/:repo/actions/runs/:id/rerun")
    }
}

mod serialization {

    #[test]
    #[ignore = "TODO: Verify Workflow can be deserialized from GitHub API response"]
    fn test_workflow_deserialize() {
        todo!("Verify Workflow can be deserialized from GitHub API response")
    }

    #[test]
    #[ignore = "TODO: Verify WorkflowRun can be deserialized from GitHub API response"]
    fn test_workflow_run_deserialize() {
        todo!("Verify WorkflowRun can be deserialized from GitHub API response")
    }

    #[test]
    #[ignore = "TODO: Verify TriggerWorkflowRequest serializes correctly"]
    fn test_trigger_workflow_request_serialize() {
        todo!("Verify TriggerWorkflowRequest serializes correctly")
    }

    #[test]
    #[ignore = "TODO: Verify inputs are included in serialization"]
    fn test_trigger_workflow_request_serialize_with_inputs() {
        todo!("Verify inputs are included in serialization")
    }
}

mod error_handling {

    #[tokio::test]
    #[ignore = "TODO: Mock: 404 response returns ApiError::NotFound"]
    async fn test_workflow_not_found() {
        todo!("Mock: 404 response returns ApiError::NotFound")
    }

    #[tokio::test]
    #[ignore = "TODO: Mock: 403 response returns ApiError::Forbidden"]
    async fn test_forbidden_access() {
        todo!("Mock: 403 response returns ApiError::Forbidden")
    }

    #[tokio::test]
    #[ignore = "TODO: Mock: Error when workflow is disabled"]
    async fn test_workflow_disabled() {
        todo!("Mock: Error when workflow is disabled")
    }
}
