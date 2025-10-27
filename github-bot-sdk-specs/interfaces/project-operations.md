# GitHub Projects v2 Operations

**Architectural Layer**: Installation-Level Operations
**Module Path**: `src/client/project.rs`
**Dependencies**:

- Types: `InstallationClient` (installation-client.md)
- Shared: `ApiError`, `Result` (shared-types.md)

## Overview

GitHub Projects v2 is a completely redesigned project management system with a flexible data model. Unlike Projects Classic (v1), Projects v2:

- Uses GraphQL API for most operations
- Supports custom fields with various types
- Has organization-level and user-level projects
- Provides more flexible item management

**Important**: This SDK provides REST API operations where available. For advanced Projects v2 features (custom fields, views, workflows), users should use GitHub's GraphQL API directly.

## Type Definitions

### ProjectV2

Represents a GitHub Projects v2 project.

```rust
/// GitHub Projects v2 project.
///
/// Projects v2 provide flexible project management with custom fields,
/// multiple views, and automation capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectV2 {
    /// Unique project identifier
    pub id: u64,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Project number (unique within owner)
    pub number: u64,

    /// Project title
    pub title: String,

    /// Project description
    pub description: Option<String>,

    /// Project owner (organization or user)
    pub owner: ProjectOwner,

    /// Project visibility
    pub public: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Project URL
    pub url: String,
}
```

### ProjectOwner

Owner of a project (organization or user).

```rust
/// Project owner (organization or user).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOwner {
    /// Owner login name
    pub login: String,

    /// Owner type
    #[serde(rename = "type")]
    pub owner_type: String, // "Organization" or "User"

    /// Owner ID
    pub id: u64,

    /// Owner node ID
    pub node_id: String,
}
```

### ProjectV2Item

Represents an item (issue or pull request) added to a project.

```rust
/// Item in a GitHub Projects v2 project.
///
/// Items are issues or pull requests added to the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectV2Item {
    /// Unique item identifier (project-specific)
    pub id: String,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Content type
    pub content_type: String, // "Issue" or "PullRequest"

    /// Content node ID (issue or PR node ID)
    pub content_node_id: String,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}
```

### AddProjectV2ItemRequest

Request to add an item to a project.

```rust
/// Request to add an item to a GitHub Projects v2 project.
#[derive(Debug, Clone, Serialize)]
pub struct AddProjectV2ItemRequest {
    /// Node ID of the content to add (issue or pull request)
    pub content_node_id: String,
}
```

## Operations

### List Organization Projects

List all Projects v2 for an organization.

**Signature**:

```rust
pub async fn list_organization_projects(
    &self,
    org: &str,
) -> Result<Vec<ProjectV2>, ApiError>
```

**Arguments**:

- `org` - Organization login name

**Returns**:

- `Ok(Vec<ProjectV2>)` - List of projects
- `Err(ApiError::NotFound)` - Organization not found
- `Err(ApiError::Forbidden)` - Insufficient permissions
- `Err(ApiError)` - Other errors

**Behavior**:

1. Make GET request to `/orgs/{org}/projects`
2. Parse response into `Vec<ProjectV2>`
3. Return projects

**Example**:

```rust
let projects = client.list_organization_projects("my-org").await?;
for project in projects {
    println!("Project: {} ({})", project.title, project.number);
}
```

---

### List User Projects

List all Projects v2 for a user.

**Signature**:

```rust
pub async fn list_user_projects(
    &self,
    username: &str,
) -> Result<Vec<ProjectV2>, ApiError>
```

**Arguments**:

- `username` - User login name

**Returns**:

- `Ok(Vec<ProjectV2>)` - List of projects
- `Err(ApiError::NotFound)` - User not found
- `Err(ApiError::Forbidden)` - Insufficient permissions (private projects)
- `Err(ApiError)` - Other errors

**Behavior**:

1. Make GET request to `/users/{username}/projects`
2. Parse response into `Vec<ProjectV2>`
3. Return projects

**Example**:

```rust
let projects = client.list_user_projects("octocat").await?;
```

---

### Get Project

Get details about a specific project.

**Signature**:

```rust
pub async fn get_project(
    &self,
    owner: &str,
    project_number: u64,
) -> Result<ProjectV2, ApiError>
```

**Arguments**:

- `owner` - Organization or user login name
- `project_number` - Project number (unique within owner)

**Returns**:

- `Ok(ProjectV2)` - Project details
- `Err(ApiError::NotFound)` - Project not found
- `Err(ApiError::Forbidden)` - Insufficient permissions
- `Err(ApiError)` - Other errors

**Behavior**:

1. Make GET request to `/users/{owner}/projects/{project_number}` or `/orgs/{owner}/projects/{project_number}`
2. Parse response into `ProjectV2`
3. Return project

**Note**: The API endpoint varies based on whether owner is an organization or user. This implementation tries organization first, then falls back to user endpoint.

**Example**:

```rust
let project = client.get_project("my-org", 1).await?;
println!("Project: {}", project.title);
```

---

### Add Item to Project

Add an issue or pull request to a project.

**Signature**:

```rust
pub async fn add_item_to_project(
    &self,
    owner: &str,
    project_number: u64,
    content_node_id: &str,
) -> Result<ProjectV2Item, ApiError>
```

**Arguments**:

- `owner` - Organization or user login name
- `project_number` - Project number
- `content_node_id` - Node ID of the issue or pull request to add

**Returns**:

- `Ok(ProjectV2Item)` - Created project item
- `Err(ApiError::NotFound)` - Project or content not found
- `Err(ApiError::Forbidden)` - Insufficient permissions
- `Err(ApiError::ValidationFailed)` - Invalid content type or already added
- `Err(ApiError)` - Other errors

**Behavior**:

1. Create `AddProjectV2ItemRequest` with content node ID
2. Make POST request to `/projects/{project_id}/items`
3. Parse response into `ProjectV2Item`
4. Return item

**Example**:

```rust
// Get issue node ID from issue object
let issue = client.get_issue("owner", "repo", 123).await?;
let item = client.add_item_to_project("my-org", 1, &issue.node_id).await?;
println!("Added item: {}", item.id);
```

---

### Remove Item from Project

Remove an item from a project.

**Signature**:

```rust
pub async fn remove_item_from_project(
    &self,
    owner: &str,
    project_number: u64,
    item_id: &str,
) -> Result<(), ApiError>
```

**Arguments**:

- `owner` - Organization or user login name
- `project_number` - Project number
- `item_id` - Project item ID (not the issue/PR ID)

**Returns**:

- `Ok(())` - Item removed successfully
- `Err(ApiError::NotFound)` - Project or item not found
- `Err(ApiError::Forbidden)` - Insufficient permissions
- `Err(ApiError)` - Other errors

**Behavior**:

1. Make DELETE request to `/projects/{project_id}/items/{item_id}`
2. Verify successful response
3. Return success

**Example**:

```rust
client.remove_item_from_project("my-org", 1, "item-id").await?;
```

---

## GraphQL API Note

For advanced Projects v2 operations not available via REST API, users should use GitHub's GraphQL API:

- **Custom Fields**: Set, update, and read custom field values
- **Views**: Create and manage project views
- **Workflows**: Configure project automation
- **Field Definitions**: List and create custom fields

The REST API provides basic project and item management. For full Projects v2 functionality, GraphQL is required.

## Error Handling

All operations return `Result<T, ApiError>` with these common errors:

- `ApiError::NotFound` - Project, organization, or content not found
- `ApiError::Forbidden` - Insufficient permissions to access or modify project
- `ApiError::ValidationFailed` - Invalid request (e.g., content already in project)
- `ApiError::RateLimited` - API rate limit exceeded
- `ApiError::NetworkError` - HTTP request failed

## Permissions

Projects v2 operations require:

- **Read**: Read access to the organization/user and project
- **Write**: Write access to project (for add/remove items)
- **Admin**: Admin access (for create/update/delete project)

Installation must have the `organization_projects: read` or `organization_projects: write` permission.

## Implementation Notes

1. Projects v2 REST API is limited compared to GraphQL API
2. Node IDs are required for adding items (use GraphQL or get from issue/PR objects)
3. Custom field management requires GraphQL API
4. Project creation/update/deletion may require GraphQL API (check GitHub API docs)
5. Some endpoints may still be in beta (check API documentation)
