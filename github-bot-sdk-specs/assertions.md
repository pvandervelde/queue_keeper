# GitHub Bot SDK Behavioral Assertions

## Overview

This document defines testable behavioral assertions for the GitHub Bot SDK. These assertions verify that authentication, API operations, and event processing work correctly and securely according to GitHub's requirements and best practices.

## Authentication Assertions

### Assertion 1: JWT Token Generation

**Given**: A valid GitHub App ID and private key
**When**: `generate_jwt_token()` is called
**Then**: Operation returns `Ok(JwtToken)` with valid JWT
**And**: Token expires within 10 minutes (GitHub requirement)
**And**: Token contains correct `iss` claim matching App ID

**Test Criteria**:

- JWT structure is valid (header.payload.signature)
- `iss` claim matches provided App ID
- `iat` claim is current timestamp (±5 seconds)
- `exp` claim is `iat + duration` (max 10 minutes)
- Token validates against provided private key

### Assertion 2: App-Level API Operations

**Given**: A valid JWT token
**When**: App-level operations are called (e.g., `list_installations()`, `get_app()`)
**Then**: Operations use JWT directly in Authorization header
**And**: GitHub API accepts the JWT authentication
**And**: Operations return app-level data (not installation-scoped)

**Test Criteria**:

- Authorization header contains `Bearer <JWT>`
- Requests go to app-level endpoints (`/app`, `/app/installations`)
- Responses contain app-level information
- No installation token is used or cached

### Assertion 3: JWT Token with Invalid Private Key

**Given**: A GitHub App ID and malformed private key
**When**: `generate_jwt_token()` is called
**Then**: Operation returns `Err(AuthenticationError::InvalidPrivateKey)`
**And**: No token is generated
**And**: Error message does not expose private key content

**Test Criteria**:

- Error type is specifically `InvalidPrivateKey`
- Private key content never appears in error message
- No partial token generation occurs

### Assertion 4: Installation Token Retrieval

**Given**: A valid JWT token and installation ID
**When**: `get_installation_token()` is called
**Then**: Operation returns `Ok(InstallationToken)` with GitHub API response
**And**: Token has valid expiry time from GitHub
**And**: Token is cached for subsequent requests

**Test Criteria**:

- JWT is used to authenticate the token exchange request
- Installation token format matches GitHub specification
- Expiry time is parsed correctly from GitHub response (1 hour from GitHub)
- Subsequent calls within cache period return cached token
- Cache respects token expiry times

### Assertion 5: Installation-Level API Operations

**Given**: A valid installation token for installation ID 12345
**When**: Installation-level operations are called (e.g., create issue, list PRs)
**Then**: Operations use installation token in Authorization header
**And**: GitHub API accepts the installation token authentication
**And**: Operations are scoped to repositories within the installation

**Test Criteria**:

- Authorization header contains `Bearer <installation_token>` or `token <installation_token>`
- Requests go to installation-scoped endpoints (`/repos/{owner}/{repo}/*`)
- Operations succeed only for repositories within the installation
- Operations fail with 404 for repositories outside the installation scope

### Assertion 6: Installation Token for Non-Existent Installation

**Given**: A valid JWT token and non-existent installation ID
**When**: `get_installation_token()` is called
**Then**: Operation returns `Err(GitHubError::InstallationNotFound)`
**And**: No token is cached
**And**: Error includes installation ID for debugging

**Test Criteria**:

- Error type is specifically `InstallationNotFound`
- Installation ID included in error context
- No cache pollution occurs

### Assertion 7: Token Cache Expiry Handling

**Given**: A cached installation token that has expired
**When**: Operations requiring authentication are performed
**Then**: New token is automatically requested from GitHub
**And**: Operations succeed with fresh token
**And**: Cache is updated with new token

**Test Criteria**:

- Expired tokens trigger automatic refresh
- Refresh happens transparently to caller
- New token replaces expired token in cache
- Cache timestamps updated correctly

## API Operations Assertions

### Assertion 8: Repository Information Retrieval

**Given**: Valid authentication and existing repository
**When**: `get_repository()` is called with repository ID
**Then**: Operation returns `Ok(Repository)` with complete metadata
**And**: Response includes owner, name, permissions, and settings

**Test Criteria**:

- Repository data matches GitHub API response format
- All required fields are populated
- Permissions reflect actual GitHub App installation permissions
- Data types match specification

### Assertion 9: Repository Access Without Permission

**Given**: Valid authentication but repository not in installation scope
**When**: `get_repository()` is called
**Then**: Operation returns `Err(GitHubError::PermissionDenied)`
**And**: Error clearly indicates permission issue
**And**: No partial data is returned

**Test Criteria**:

- Error type is specifically `PermissionDenied`
- Error message is actionable for troubleshooting
- Security context preserved (no data leaks)

### Assertion 10: Pull Request Creation

**Given**: Valid authentication and repository with write permissions
**When**: `create_pull_request()` is called with valid PR data
**Then**: Operation returns `Ok(PullRequest)` with GitHub-assigned ID
**And**: PR is visible in GitHub repository
**And**: All specified metadata is set correctly

**Test Criteria**:

- PR ID is valid GitHub-assigned identifier
- Title, body, base, and head match request
- Labels, assignees, and reviewers applied correctly
- PR state is "open" initially

### Assertion 11: Pull Request Creation Without Write Permission

**Given**: Valid authentication but repository with read-only permissions
**When**: `create_pull_request()` is called
**Then**: Operation returns `Err(GitHubError::PermissionDenied)`
**And**: No PR is created in repository
**And**: Error indicates specific permission needed

**Test Criteria**:

- Error type is specifically `PermissionDenied`
- Error indicates "write" permission requirement
- No partial PR creation occurs

### Assertion 12: Issue Management Operations

**Given**: Valid authentication and repository with appropriate permissions
**When**: Issue operations are performed (create, update, close)
**Then**: Operations succeed and modify GitHub repository state
**And**: Issue state changes are immediately visible
**And**: All metadata updates are preserved

**Test Criteria**:

- Issue creation returns valid issue ID
- Updates modify exact fields specified
- State transitions (open/closed) work correctly
- Comments, labels, and assignments persist

## Rate Limiting Assertions

### Assertion 13: Rate Limit Respect

**Given**: GitHub API client approaching rate limits
**When**: Multiple API requests are made rapidly
**Then**: Client automatically throttles requests
**And**: Rate limit headers are monitored and respected
**And**: Operations eventually succeed without rate limit violations

**Test Criteria**:

- No HTTP 429 (rate limited) responses from GitHub
- Request delays increase as rate limit approaches
- Rate limit headers parsed correctly
- Operations resume after rate limit reset

### Assertion 14: Rate Limit Exceeded Handling

**Given**: GitHub API returning HTTP 429 rate limit exceeded
**When**: Additional API requests are attempted
**Then**: Client implements exponential backoff with jitter
**And**: Requests are retried after appropriate delay
**And**: Operations eventually succeed when rate limit resets

**Test Criteria**:

- Exponential backoff with jitter implemented
- Retry delays respect `Retry-After` header when present
- Maximum retry attempts honored
- Success after rate limit window expires

### Assertion 15: Secondary Rate Limit Handling

**Given**: GitHub API returning secondary rate limit (abuse detection)
**When**: Client receives HTTP 403 with rate limit message
**Then**: Client implements longer backoff period
**And**: Aggressive retry patterns are avoided
**And**: Operations resume after cooldown period

**Test Criteria**:

- Secondary rate limits detected correctly
- Longer backoff periods applied (60+ seconds)
- No aggressive retry behavior
- Graceful degradation during limits

## Webhook Processing Assertions

### Assertion 16: Webhook Signature Validation

**Given**: Valid webhook payload and matching signature
**When**: `validate_webhook()` is called
**Then**: Operation returns `Ok(true)` indicating valid signature
**And**: Payload content is verified as authentic
**And**: Validation uses constant-time comparison

**Test Criteria**:

- HMAC-SHA256 signature computed correctly
- Signature comparison is constant-time (timing attack resistant)
- Payload integrity confirmed
- No timing side channels in validation

### Assertion 17: Webhook Signature Validation Failure

**Given**: Webhook payload with invalid or tampered signature
**When**: `validate_webhook()` is called
**Then**: Operation returns `Ok(false)` indicating invalid signature
**And**: Payload is rejected as potentially malicious
**And**: Validation timing is consistent regardless of failure type

**Test Criteria**:

- Invalid signatures correctly rejected
- Tampered payloads detected
- Consistent timing prevents timing attacks
- No information leakage about signature validation

### Assertion 18: Webhook Event Processing

**Given**: Valid authenticated webhook with supported event type
**When**: `process_webhook_event()` is called
**Then**: Event is routed to appropriate handler
**And**: Event processing is idempotent
**And**: Handler receives correctly parsed event data

**Test Criteria**:

- Event type routing works correctly
- Duplicate events (same ID) handled idempotently
- Event data parsed according to GitHub schema
- Handler context includes authentication information

### Assertion 19: Webhook Event Deduplication

**Given**: Multiple webhook deliveries with same event ID
**When**: Events are processed sequentially
**Then**: Only first event is processed completely
**And**: Subsequent events are recognized as duplicates
**And**: No duplicate processing side effects occur

**Test Criteria**:

- Event ID tracking prevents duplicates
- First processing completes normally
- Duplicate detection is immediate
- No resource waste on duplicate processing

## Error Handling and Recovery Assertions

### Assertion 20: Network Connectivity Failure

**Given**: GitHub API client when network connectivity is lost
**When**: API operations are attempted
**Then**: Operations return `Err(GitHubError::NetworkError)`
**And**: Client can recover when connectivity is restored
**And**: Retry logic handles transient failures

**Test Criteria**:

- Network errors classified correctly
- Automatic retry with exponential backoff
- Recovery after connectivity restoration
- Circuit breaker prevents cascading failures

### Assertion 21: GitHub API Server Errors

**Given**: GitHub API returning HTTP 5xx server errors
**When**: API operations are attempted
**Then**: Operations are retried with backoff
**And**: Eventually succeed when GitHub recovers
**And**: Circuit breaker protects against sustained failures

**Test Criteria**:

- Server errors trigger retry logic
- Exponential backoff between retries
- Circuit breaker opens after sustained failures
- Operations succeed after GitHub recovery

### Assertion 22: Authentication Token Expiry During Operations

**Given**: Long-running operations with expiring tokens
**When**: Token expires during operation sequence
**Then**: New tokens are automatically obtained
**And**: Operations continue seamlessly
**And**: No authentication errors surface to caller

**Test Criteria**:

- Token expiry detected automatically
- Refresh happens transparently
- Operations resume with new token
- No user-visible authentication interruptions

## Security Assertions

### Assertion 23: Private Key Security

**Given**: GitHub App private key loaded into memory
**When**: Private key is used for JWT generation
**Then**: Private key never appears in logs or error messages
**And**: Memory is securely cleared after use
**And**: Key material is not accessible through debugging

**Test Criteria**:

- No private key content in any logs
- Memory zeroed after cryptographic operations
- Debug output excludes sensitive data
- Error messages don't leak key information

### Assertion 24: Token Security in Transit

**Given**: Authentication tokens being used for API requests
**When**: HTTP requests are made to GitHub API
**Then**: Tokens are transmitted only over HTTPS
**And**: TLS certificate validation is enforced
**And**: Tokens are included only in Authorization headers

**Test Criteria**:

- All requests use HTTPS protocol
- Certificate validation is strict
- No tokens in URLs or query parameters
- Authorization headers properly formatted

### Assertion 25: Sensitive Data Logging Prevention

**Given**: SDK operations involving authentication or API data
**When**: Logging occurs at any level
**Then**: No sensitive data appears in log output
**And**: Structured logging maintains security boundaries
**And**: Debug information excludes secrets

**Test Criteria**:

- No tokens, keys, or passwords in logs
- API request/response bodies sanitized
- Personal data (emails, names) redacted appropriately
- Debug tracing excludes sensitive context

## Performance and Scalability Assertions

### Assertion 26: Concurrent API Operations

**Given**: Multiple concurrent GitHub API requests
**When**: Operations execute simultaneously
**Then**: All operations complete successfully
**And**: Connection pooling is utilized efficiently
**And**: No race conditions in token management

**Test Criteria**:

- Thread safety across all operations
- HTTP connection reuse working
- Token cache handles concurrent access
- Performance scales with concurrency

### Assertion 27: Memory Usage Under Load

**Given**: High-volume API operations over extended time
**When**: Continuous operations are performed
**Then**: Memory usage remains bounded
**And**: No memory leaks in token or connection management
**And**: Garbage collection is efficient

**Test Criteria**:

- Memory usage stabilizes under constant load
- No unbounded cache growth
- Connection pools respect size limits
- Long-running operations don't accumulate memory
