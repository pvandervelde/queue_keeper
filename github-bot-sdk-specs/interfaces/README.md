# GitHub Bot SDK Interface Specifications

This directory contains detailed interface specifications for the GitHub Bot SDK installation-level client operations. Each specification defines type signatures, function contracts, error conditions, and usage examples.

## Overview

The installation-level client provides authenticated access to GitHub API operations scoped to a specific app installation. All operations use installation tokens (not JWTs) and respect GitHub's rate limiting and permissions.

## Interface Documents

### Core Client

- **[installation-client.md](./installation-client.md)** - InstallationClient foundation
  - Client struct definition
  - Factory methods
  - Generic request helpers (GET, POST, PUT, DELETE, PATCH)
  - Token exchange integration

### Repository Operations

- **[repository-operations.md](./repository-operations.md)** - Repository and branch management
  - Repository information retrieval
  - Branch operations (list, get, create, delete)
  - Git reference management (tags, branches)
  - Repository metadata updates

### Issue and PR Operations

- **[issue-operations.md](./issue-operations.md)** - GitHub issue management
  - Issue CRUD operations
  - Issue comments
  - Label management (create, update, delete)
  - Issue state transitions

- **[pull-request-operations.md](./pull-request-operations.md)** - Pull request operations
  - PR CRUD operations
  - Review management
  - PR comments and discussions
  - Merge operations

### Cross-Domain Operations

- **[milestone-operations.md](./milestone-operations.md)** - Milestone management
  - Milestone CRUD operations
  - Milestone association with issues/PRs

- **[workflow-operations.md](./workflow-operations.md)** - GitHub Actions workflows
  - Workflow listing and retrieval
  - Workflow run management
  - Workflow status tracking

- **[release-operations.md](./release-operations.md)** - Release management
  - Release CRUD operations
  - Release asset management
  - Tag association

### Infrastructure

- **[pagination.md](./pagination.md)** - Pagination support
  - PagedResponse generic type
  - Link header parsing
  - Page navigation helpers

- **[rate-limiting-retry.md](./rate-limiting-retry.md)** - Rate limiting and retry logic
  - Installation-level rate tracking
  - Exponential backoff with jitter
  - 429 and 403 handling
  - Retry policy configuration

## Dependency Graph

```
installation-client (foundation)
    ↓
├── repository-operations
├── issue-operations
├── pull-request-operations
├── milestone-operations
├── workflow-operations
└── release-operations
    ↓
pagination (enhances list operations)
    ↓
rate-limiting-retry (integrates with all operations)
```

## Architectural Location

All installation client interfaces are part of the **GitHub Bot SDK** library:

**Layer**: Infrastructure adapters
**Crate**: `github-bot-sdk`
**Module**: `client`
**Dependencies**: `auth` module (for tokens), `error` module

## Type Organization

Types are organized by domain in separate files:

- `installation.rs` - InstallationClient core
- `repository.rs` - Repository, Branch, GitRef types and operations
- `issue.rs` - Issue, Comment, Label types and operations
- `pull_request.rs` - PullRequest, Review types and operations
- `milestone.rs` - Milestone types and operations
- `workflow.rs` - Workflow, WorkflowRun types and operations
- `release.rs` - Release types and operations
- `pagination.rs` - PagedResponse generic type
- `retry.rs` - RetryPolicy and backoff strategies

## Naming Conventions

- **Types**: PascalCase (e.g., `InstallationClient`, `Repository`, `PagedResponse`)
- **Functions**: snake_case (e.g., `get_repository`, `list_branches`)
- **Enums**: PascalCase with PascalCase variants (e.g., `IssueState::Open`)
- **Error types**: Reuse `ApiError` from `error.rs`

## Common Patterns

### Result Types

All operations return `Result<T, ApiError>`:

```rust
pub async fn operation(&self) -> Result<SuccessType, ApiError>
```

### Permission Errors

- `ApiError::PermissionDenied` - 403 responses (insufficient permissions)
- `ApiError::NotFound` - 404 responses (resource not found OR no permission)
- `ApiError::AuthorizationFailed` - authentication token issues

### Request Helpers

All operations use generic request methods:

```rust
self.get(path).await
self.post(path, body).await
self.put(path, body).await
self.delete(path).await
self.patch(path, body).await
```

### Serde Patterns

All API types derive:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
```

Optional fields use `Option<T>`, with `#[serde(default)]` where appropriate.

## Implementation Guidelines

1. **Read the specification** - Each spec document defines exact signatures
2. **Check shared registry** - Verify types aren't already defined elsewhere
3. **Follow patterns** - Use existing `GitHubClient` and `App` as examples
4. **Test thoroughly** - Mock GitHub API responses with `wiremock`
5. **Document examples** - All public APIs need rustdoc with examples

## Security Considerations

- Never log installation tokens
- All requests use HTTPS (enforced by `reqwest`)
- Validate all external input
- Handle rate limiting to avoid abuse detection
- Respect GitHub's API terms of service

## References

- [GitHub REST API Documentation](https://docs.github.com/en/rest)
- [GitHub App Authentication](https://docs.github.com/en/developers/apps/building-github-apps/authenticating-with-github-apps)
- [Rate Limiting](https://docs.github.com/en/rest/overview/resources-in-the-rest-api#rate-limiting)
