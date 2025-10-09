# GitHub Bot SDK Domain Vocabulary

This document defines the core concepts and terminology used throughout the github-bot-sdk to ensure consistent understanding across all bot implementations and GitHub integrations.

## GitHub Integration Concepts

### GitHub App

A type of application that can be installed on GitHub organizations and repositories to provide automated functionality.

- **App ID**: Unique numeric identifier assigned by GitHub
- **Private Key**: RSA private key used for signing JWT tokens
- **Permissions**: Specific GitHub API scopes granted to the app
- **Installation**: Specific deployment of the app to an organization or repository
- **Authentication**: Two-step process: JWT for app identity, installation token for API access

### Installation

A specific deployment of a GitHub App to an organization or repository.

- **Installation ID**: Unique numeric identifier for the specific installation
- **Target**: Organization or repository where the app is installed
- **Permissions**: Subset of app permissions granted to this installation
- **Token**: Short-lived access token for API operations within installation scope
- **Events**: Webhook events that the installation should receive

### Installation Token

A short-lived access token that provides API access within a specific installation scope.

- **Scope**: Limited to repositories and permissions of the installation
- **Expiration**: Valid for 1 hour from issuance
- **Refresh**: Must be renewed before expiration using JWT
- **Authentication**: Used in Authorization header for GitHub API requests
- **Caching**: Should be cached until near expiration for efficiency

### JWT (JSON Web Token)

A signed token that authenticates the GitHub App identity for installation token requests.

- **Header**: Specifies RS256 algorithm and token type
- **Payload**: Contains app ID, issued time, and expiration time
- **Signature**: RSA signature using the app's private key
- **Expiration**: Valid for maximum 10 minutes as per GitHub requirements
- **Purpose**: Exchanges for installation tokens via GitHub API

## Event Processing Concepts

### Event Envelope

A standardized container that wraps GitHub webhook events with additional metadata.

- **Event ID**: Unique identifier for deduplication and tracking
- **Event Type**: GitHub event classification (pull_request, issues, etc.)
- **Repository**: Source repository information
- **Entity**: Primary GitHub object affected by the event
- **Session ID**: Grouping identifier for ordered processing
- **Payload**: Original GitHub webhook data
- **Metadata**: Processing timestamps, correlation IDs, routing information

### GitHub Event

A specific type of activity that occurs on GitHub and triggers webhook notifications.

- **Event Types**: pull_request, issues, push, release, check_run, etc.
- **Actions**: Specific activities within event types (opened, closed, synchronized)
- **Payload Structure**: GitHub-defined JSON schema for each event type
- **Headers**: HTTP headers including event type and delivery ID
- **Signature**: HMAC-SHA256 signature for authenticity verification

### Event Parsing

The process of converting raw GitHub webhook payloads into strongly-typed structures.

- **Type Safety**: Rust structures that match GitHub's event schemas
- **Validation**: Ensures required fields are present and correctly typed
- **Error Handling**: Graceful handling of unknown or malformed events
- **Extensibility**: Support for new GitHub event types and fields

### Session Processing

Ordered processing of related events based on GitHub entities.

- **Session Correlation**: Events for same PR/issue share session identifier
- **Sequential Processing**: Events within session processed in chronological order
- **Parallelism**: Different sessions can be processed concurrently
- **State Management**: Tracking entity state across multiple events

## Authentication Concepts

### GitHub App Authentication

The primary authentication mechanism for GitHub API access in bot applications.

- **App-Level**: JWT-based authentication proving app identity
- **Installation-Level**: Token-based authentication for specific installations
- **Two-Phase**: First authenticate as app, then obtain installation token
- **Security**: Private key never transmitted, only signatures

### Private Key

The RSA private key used to sign JWT tokens for GitHub App authentication.

- **Format**: PEM-encoded RSA private key
- **Security**: Must be stored securely and never logged or exposed
- **Usage**: Signs JWT tokens for GitHub API authentication
- **Rotation**: Can be rotated through GitHub App settings
- **Storage**: Environment variables, files, or secure key management services

### Token Caching

The strategy for storing and reusing installation tokens to improve performance.

- **TTL**: Tokens cached until 5 minutes before expiration
- **Keying**: Cached by installation ID for multi-installation bots
- **Invalidation**: Automatic removal of expired tokens
- **Concurrency**: Thread-safe access for concurrent bot operations
- **Fallback**: Fresh token acquisition on cache misses

### Secret Management

Secure handling of sensitive authentication data and configuration.

- **Principles**: Never log secrets, use secure storage, rotate regularly
- **Debug Output**: All secret types redact values in debug output
- **Environment**: Prefer environment variables over hardcoded values
- **Key Vault**: Integration with cloud secret management services
- **Scoping**: Minimum required permissions for each operation

## API Client Concepts

### GitHub API Client

A wrapper around GitHub's REST API that handles authentication and common operations.

- **Authentication**: Automatic injection of installation tokens
- **Rate Limiting**: Built-in respect for GitHub's rate limits
- **Retry Logic**: Automatic retry for transient failures
- **User Agent**: Proper identification for GitHub's request tracking
- **Endpoints**: Strongly-typed methods for common GitHub operations

### Rate Limiting

GitHub's mechanism for controlling API usage and ensuring service stability.

- **Limits**: Different limits for different endpoint categories
- **Headers**: Rate limit information returned in response headers
- **Backoff**: Exponential backoff when approaching limits
- **Reset**: Rate limits reset hourly based on UTC time
- **Monitoring**: Tracking usage to avoid limit exhaustion

### API Operation

A specific interaction with GitHub's API, such as creating comments or updating status.

- **HTTP Method**: GET, POST, PATCH, DELETE for different operations
- **Endpoint**: Specific GitHub API URL path and parameters
- **Request Body**: JSON payload for operations that modify data
- **Response**: GitHub's JSON response with operation results
- **Error Handling**: Proper interpretation of GitHub error responses

### Pagination

GitHub's mechanism for returning large result sets across multiple requests.

- **Page-Based**: Results divided into pages with next/previous links
- **Cursor-Based**: Some endpoints use cursor-based pagination
- **Helper Methods**: SDK utilities for iterating through all pages
- **Limits**: Per-page limits and total result set limits

## Error Handling Concepts

### GitHub API Error

Errors returned by GitHub's API that indicate various failure conditions.

- **Status Codes**: HTTP status codes indicating error categories
- **Error Messages**: Human-readable descriptions of failures
- **Documentation URLs**: Links to relevant GitHub documentation
- **Rate Limit**: Special handling for rate limit exceeded errors
- **Validation**: Field-specific validation errors for malformed requests

### Authentication Error

Failures related to GitHub App authentication and authorization.

- **Invalid Credentials**: Wrong app ID or corrupted private key
- **Expired Token**: Installation token past its expiration time
- **Insufficient Permissions**: App lacks required permissions for operation
- **Installation Issues**: App not installed or installation suspended
- **Network Issues**: Connectivity problems during authentication

### Transient Error

Temporary failure conditions that may succeed if retried.

- **Network Timeouts**: Request timed out due to network issues
- **Server Errors**: GitHub API returning 5xx status codes
- **Rate Limiting**: Temporary throttling that resets over time
- **Service Unavailability**: GitHub maintenance or capacity issues
- **Retry Strategy**: Exponential backoff with maximum attempt limits

### Permanent Error

Failure conditions that will not succeed regardless of retries.

- **Authorization**: Insufficient permissions for the requested operation
- **Not Found**: Requested resource does not exist
- **Validation**: Malformed request data that will always fail
- **Conflict**: Operation conflicts with current resource state
- **No Retry**: Immediate failure without retry attempts

## Configuration Concepts

### Bot Configuration

Settings that control bot behavior and integration with GitHub services.

- **App Credentials**: GitHub App ID and private key location
- **User Agent**: Identification string for GitHub API requests
- **Timeout Settings**: Network and operation timeout configurations
- **Rate Limit**: Custom rate limiting settings beyond GitHub defaults
- **Environment**: Environment-specific settings and feature flags

### Environment Configuration

Settings that vary between development, staging, and production environments.

- **GitHub API URL**: GitHub.com vs GitHub Enterprise Server endpoints
- **Credential Sources**: Environment variables, files, or secret services
- **Logging Levels**: Different verbosity for different environments
- **Feature Flags**: Enable/disable features per environment
- **Monitoring**: Environment-specific telemetry and monitoring settings

## Testing Concepts

### Mock Authentication

Test doubles that simulate GitHub App authentication without real API calls.

- **Predetermined Responses**: Configurable token responses for tests
- **Error Simulation**: Ability to simulate authentication failures
- **No Network**: Pure in-memory implementation for fast tests
- **Deterministic**: Consistent behavior for reproducible tests

### Test Events

Sample GitHub webhook events used for testing bot behavior.

- **Representative**: Cover common GitHub event types and actions
- **Edge Cases**: Include malformed or unusual event structures
- **Complete**: Full event payloads with all relevant fields
- **Versioned**: Match current GitHub webhook schema versions

### Integration Testing

Testing that verifies bot behavior against real or realistic GitHub services.

- **Test Repositories**: Dedicated repositories for integration testing
- **Test Apps**: GitHub Apps specifically for testing purposes
- **Sandbox**: Isolated environment that doesn't affect production
- **Cleanup**: Automatic cleanup of test data and resources

## Observability Concepts

### Distributed Tracing

Tracking requests and operations across multiple services and systems.

- **Trace Context**: W3C standard headers for trace propagation
- **Spans**: Individual operations within a distributed request
- **Correlation**: Links SDK operations with upstream and downstream processing
- **Sampling**: Configurable sampling rates to control overhead

### Structured Logging

Logging that uses consistent, machine-readable formats for better analysis.

- **JSON Format**: Structured log entries with standard fields
- **Context**: Correlation IDs and operation metadata in log entries
- **Levels**: ERROR, WARN, INFO, DEBUG for different severity levels
- **Filtering**: Ability to filter and search logs by structured fields

### Metrics

Quantitative measurements of SDK operations and performance.

- **Counters**: API requests, authentication attempts, errors
- **Gauges**: Active connections, cached tokens, queue depth
- **Histograms**: Request latency, token refresh time, payload size
- **Labels**: GitHub organization, repository, operation type for filtering

This vocabulary establishes the shared language for github-bot-sdk architecture and implementation, ensuring consistent terminology across all bot implementations and GitHub integrations.
