# Authentication Module

The authentication module provides comprehensive GitHub App authentication capabilities, including JWT signing, installation token management, and secure credential handling.

## Overview

The authentication system provides GitHub App authentication capabilities for both repository-specific and installation-wide operations. It supports authentication as a GitHub App for general operations, and as specific installations for user or organization-scoped actions.

## Core Types

### GitHubAppAuth

The primary authentication manager that handles GitHub App credentials and token lifecycle.

**GitHubAppAuth Interface Requirements**:

- Support GitHub App ID and private key configuration
- Maintain token cache with configurable TTL and refresh margins
- Generate GitHub App JWTs for API authentication
- Exchange JWTs for installation tokens (both by installation ID and repository)
- Automatic token refresh with expiration margin handling
- HTTP client integration with appropriate user agent and retry logic

```

### InstallationToken

Represents a GitHub App installation token with metadata and automatic expiration handling.

**InstallationToken Requirements**:

- Secure token storage with automatic redaction in logs
- Expiration tracking with configurable refresh margins
- Installation-specific permission and repository scope tracking
- Authorization header formatting for GitHub API requests
- Thread-safe access for concurrent bot operations

### AuthConfig

**Authentication Configuration Requirements**:

- JWT expiration duration (default: 10 minutes, GitHub limit)
- Token refresh margin for proactive renewal (default: 5 minutes)
- Cache TTL slightly less than token expiration (default: 55 minutes)
- Retry configuration for transient failures (default: 3 retries)
- Configurable GitHub API endpoint for GitHub Enterprise support
- User agent customization for request identification

## Builder Pattern

**Builder Pattern Requirements**:
    private_key: Option<PrivateKey>,
    config: AuthConfig,
}

impl GitHubAppAuthBuilder {
    pub fn app_id(mut self, app_id: u64) -> Self { ... }

    pub fn private_key(mut self, key: PrivateKey) -> Self { ... }

    pub fn private_key_from_pem(mut self, pem: &str) -> Result<Self, AuthError> { ... }

    pub fn private_key_from_env(mut self, env_var: &str) -> Result<Self, AuthError> { ... }

    pub fn private_key_from_file(mut self, path: &Path) -> Result<Self, AuthError> { ... }

    pub fn github_api_url(mut self, url: String) -> Self { ... }

    pub fn user_agent(mut self, agent: String) -> Self { ... }

- Flexible configuration of JWT and token parameters
- Multiple private key input methods (PEM string, file, environment)
- GitHub API endpoint customization for enterprise deployments
- User agent configuration for request identification
- Validation of required parameters before construction

## Token Caching

**Intelligent Caching Requirements**:

### TokenCache Interface

**Cache Contract Requirements**:

- Installation ID-based token storage and retrieval
- Asynchronous cache operations for non-blocking performance
- Thread-safe concurrent access for multi-bot scenarios
- Automatic cleanup of expired tokens
- Installation-specific token removal for security

**Cache Implementation Options**:

- In-memory cache for single-instance deployments
- Redis-backed cache for distributed bot architectures
- Configurable cache backends for deployment flexibility

### Caching Strategy

**Cache Management Requirements**:

1. **Cache Key Strategy**: Installation ID as primary cache key
2. **TTL Management**: 55-minute cache duration (5 minutes before GitHub expiration)
3. **Proactive Refresh**: Background token renewal before expiration
4. **Cleanup Operations**: Automatic removal of expired tokens
5. **Fallback Handling**: Fresh token acquisition on cache misses

## Security Considerations

### Private Key Handling

**Secure Key Management Requirements**:

- Private key encapsulation preventing accidental exposure
- Multiple key loading methods (PEM string, file path, environment variable)
- Automatic key format validation and parsing
- Debug output redaction for security logging
- Memory-safe key storage and handling

### Token Security

**Secret Management Requirements**:

- Secure string wrapper preventing accidental token logging
- Controlled access patterns for sensitive token data
- Debug output redaction for all secret values
- Thread-safe secret handling for concurrent access
- Clear separation between public and sensitive data

## Error Handling

**AuthError Classification Requirements**:

- **MissingConfig**: Required configuration parameters not provided
- **InvalidPrivateKey**: Private key format or parsing errors
- **JwtGeneration**: JWT creation or signing failures
- **GitHubApi**: GitHub API request failures with status codes
- **InstallationNotFound**: Invalid installation ID references
- **TokenExpired**: Expired token usage attempts
- **Network**: Network connectivity and request failures
- **Cache**: Token cache operation failures

**Error Context Requirements**:

- Structured error information with relevant context fields
- Error source chain preservation for debugging
- User-friendly error messages for configuration issues
- Detailed error information for troubleshooting

## Usage Patterns

### Basic Setup Workflow

**Authentication Setup Requirements**:

- GitHub App ID configuration from environment or direct assignment
- Private key loading from multiple sources (environment, file, direct)
- User agent configuration for request identification
- Validation of required configuration before use

**Token Acquisition Requirements**:

- Installation ID-based token acquisition for known installations
- Repository-based token lookup for repository-specific operations
- Proactive token refresh checking based on expiration margins
- Automatic token renewal before expiration deadlines

**Custom Configuration Requirements**:

- GitHub Enterprise Server API endpoint support
- Configurable JWT expiration durations within GitHub limits
- Custom token refresh margins for different deployment scenarios
- Flexible private key loading from multiple sources

## Integration Requirements

**SDK Component Integration**:

- **Client Module**: Automatic token refresh and injection for API requests
- **Events Module**: App credential-based webhook signature validation
- **Tracing Module**: Authentication operation tracing with correlation IDs
- **Testing Module**: Mock authentication implementations for unit testing

**Cross-Module Dependencies**:

- Consistent error handling patterns across all SDK modules
- Shared configuration patterns for deployment flexibility
- Common logging and tracing integration points
- Unified async/await patterns for all operations

## Testing Support

**Mock Implementation Requirements**:

- Configurable mock authentication provider for testing scenarios
- Pre-configured token responses for predictable test behavior
- Installation-specific token simulation for integration tests

    #[async_trait]
    impl AuthProvider for MockGitHubAppAuth {
        async fn installation_token(&self, installation_id: u64) -> Result<InstallationToken, AuthError> { ... }
    }
}
```

## Performance Characteristics

- **JWT Generation**: ~1ms on modern hardware
- **Token Cache Hit**: ~0.1ms lookup time
- **Token Refresh**: ~100-200ms GitHub API round-trip
- **Memory Usage**: ~1KB per cached token
- **Concurrent Safety**: All operations are thread-safe and async-compatible
