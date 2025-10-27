# Rate Limiting and Retry Interface Specification

**Module**: `github-bot-sdk::client::retry`
**File**: `crates/github-bot-sdk/src/client/retry.rs`
**Dependencies**: `ApiError`, HTTP headers, `tokio::time`

## Overview

Retry logic and rate limit handling for GitHub API requests. Implements exponential backoff for transient failures and respects GitHub's rate limiting.

## Architectural Location

**Layer**: Infrastructure adapter (HTTP request middleware)
**Purpose**: Resilient API request handling
**Pattern**: Retry policy with backoff strategy

## Core Types

### RetryPolicy

Configuration for retry behavior.

```rust
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier for exponential backoff
    pub backoff_multiplier: f64,
}
```

### Default Retry Policy

```rust
impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(1000),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 2.0,
        }
    }
}
```

### RateLimitInfo

Parsed rate limit information from response headers.

```rust
#[derive(Debug, Clone, Copy)]
pub struct RateLimitInfo {
    /// Total request limit
    pub limit: u32,
    /// Remaining requests in current window
    pub remaining: u32,
    /// When rate limit resets (Unix timestamp)
    pub reset: i64,
}
```

### RetryDecision

Decision on whether to retry a failed request.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    /// Retry after specified duration
    Retry { after: Duration },
    /// Do not retry (permanent failure)
    DontRetry,
}
```

## Retry Functions

### Should Retry

```rust
/// Determine if a request should be retried based on error.
///
/// # Arguments
///
/// * `error` - The API error that occurred
/// * `attempt` - Current attempt number (0-indexed)
/// * `policy` - Retry policy configuration
///
/// # Returns
///
/// Returns `RetryDecision` indicating whether to retry and delay.
///
/// # Retry Conditions
///
/// Retries are attempted for:
/// - Network errors (connection refused, timeout)
/// - Rate limit errors (429, 403 with rate limit headers)
/// - Server errors (500, 502, 503, 504)
///
/// No retry for:
/// - Client errors (400, 404, 422)
/// - Permission denied (403 without rate limit)
/// - Attempt limit exceeded
///
/// # Examples
///
/// ```rust
/// let policy = RetryPolicy::default();
/// let decision = should_retry(&ApiError::Timeout, 0, &policy);
/// assert!(matches!(decision, RetryDecision::Retry { .. }));
/// ```
pub fn should_retry(
    error: &ApiError,
    attempt: u32,
    policy: &RetryPolicy,
) -> RetryDecision;
```

**Implementation Logic**:

1. Check if attempt count exceeds max_retries → `DontRetry`
2. Match on error type:
   - `ApiError::RateLimitExceeded` → Calculate delay from reset time
   - `ApiError::Timeout` → Exponential backoff
   - `ApiError::HttpClientError` → Exponential backoff
   - `ApiError::HttpError` with 5xx status → Exponential backoff
   - Other errors → `DontRetry`
3. Cap calculated delay at `policy.max_backoff`

### Calculate Backoff

```rust
/// Calculate backoff duration for retry attempt.
///
/// Uses exponential backoff: delay = initial * (multiplier ^ attempt)
///
/// # Arguments
///
/// * `attempt` - Current attempt number (0-indexed)
/// * `policy` - Retry policy configuration
///
/// # Returns
///
/// Returns `Duration` to wait before retry, capped at max_backoff.
///
/// # Examples
///
/// ```rust
/// let policy = RetryPolicy::default();
/// let delay = calculate_backoff(0, &policy);
/// assert_eq!(delay, Duration::from_millis(1000));
///
/// let delay = calculate_backoff(1, &policy);
/// assert_eq!(delay, Duration::from_millis(2000));
/// ```
pub fn calculate_backoff(attempt: u32, policy: &RetryPolicy) -> Duration;
```

**Implementation**:

```rust
let backoff_ms = policy.initial_backoff.as_millis() as f64
    * policy.backoff_multiplier.powi(attempt as i32);

let backoff = Duration::from_millis(backoff_ms as u64);

backoff.min(policy.max_backoff)
```

### Calculate Rate Limit Delay

```rust
/// Calculate delay until rate limit resets.
///
/// # Arguments
///
/// * `reset_timestamp` - Unix timestamp when rate limit resets
///
/// # Returns
///
/// Returns `Duration` to wait until rate limit reset.
/// Returns zero duration if reset time is in the past.
///
/// # Examples
///
/// ```rust
/// let reset = OffsetDateTime::now_utc().unix_timestamp() + 60;
/// let delay = calculate_rate_limit_delay(reset);
/// assert!(delay <= Duration::from_secs(61)); // Allow 1s buffer
/// ```
pub fn calculate_rate_limit_delay(reset_timestamp: i64) -> Duration;
```

## Rate Limit Parsing

### Parse Rate Limit Headers

```rust
/// Parse rate limit information from HTTP response headers.
///
/// # Arguments
///
/// * `headers` - HTTP response headers
///
/// # Returns
///
/// Returns `Some(RateLimitInfo)` if rate limit headers present.
/// Returns `None` if headers are missing or malformed.
///
/// # GitHub Rate Limit Headers
///
/// - `x-ratelimit-limit`: Total requests allowed
/// - `x-ratelimit-remaining`: Requests remaining
/// - `x-ratelimit-reset`: Reset timestamp (Unix epoch)
///
/// # Examples
///
/// ```rust
/// use reqwest::header::HeaderMap;
///
/// let mut headers = HeaderMap::new();
/// headers.insert("x-ratelimit-limit", "5000".parse().unwrap());
/// headers.insert("x-ratelimit-remaining", "4999".parse().unwrap());
/// headers.insert("x-ratelimit-reset", "1234567890".parse().unwrap());
///
/// let info = parse_rate_limit_headers(&headers).unwrap();
/// assert_eq!(info.limit, 5000);
/// assert_eq!(info.remaining, 4999);
/// ```
pub fn parse_rate_limit_headers(headers: &reqwest::header::HeaderMap) -> Option<RateLimitInfo>;
```

### Check Rate Limit

```rust
/// Check if request should be delayed due to approaching rate limit.
///
/// # Arguments
///
/// * `info` - Current rate limit information
/// * `threshold` - Minimum remaining requests before voluntary delay (default: 10)
///
/// # Returns
///
/// Returns `Some(Duration)` if delay recommended.
/// Returns `None` if sufficient requests remaining.
///
/// # Strategy
///
/// Voluntarily delay requests when remaining count drops below threshold
/// to avoid hitting hard rate limit.
pub fn check_rate_limit(info: &RateLimitInfo, threshold: u32) -> Option<Duration>;
```

## Retry Execution

### Execute with Retry

```rust
/// Execute an async operation with retry logic.
///
/// # Arguments
///
/// * `operation` - Async function to execute
/// * `policy` - Retry policy configuration
///
/// # Returns
///
/// Returns the operation result after retries exhausted or success.
///
/// # Behavior
///
/// 1. Execute operation
/// 2. On error, check if retryable
/// 3. If retry, sleep for backoff duration
/// 4. Repeat up to max_retries times
/// 5. Return last error if all attempts fail
///
/// # Examples
///
/// ```rust
/// let policy = RetryPolicy::default();
///
/// let result = execute_with_retry(
///     || async {
///         client.get("https://api.github.com/user").await
///     },
///     &policy,
/// ).await?;
/// ```
pub async fn execute_with_retry<F, Fut, T, E>(
    mut operation: F,
    policy: &RetryPolicy,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                let decision = should_retry(&error, attempt, policy);

                match decision {
                    RetryDecision::Retry { after } => {
                        log::warn!("Request failed (attempt {}), retrying after {:?}: {}",
                            attempt + 1, after, error);
                        tokio::time::sleep(after).await;
                        attempt += 1;
                    }
                    RetryDecision::DontRetry => {
                        return Err(error);
                    }
                }
            }
        }
    }
}
```

## Integration with InstallationClient

### Generic Request with Retry

Example integration:

```rust
impl InstallationClient {
    /// Internal method: GET request with retry logic.
    async fn get_with_retry(&self, path: &str) -> Result<reqwest::Response, ApiError> {
        let policy = RetryPolicy::default();

        execute_with_retry(
            || async {
                // Get installation token
                let token = self.get_installation_token().await?;

                // Build request
                let url = format!("{}/{}", self.client.base_url(), path);
                let request = self.client.http_client()
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", token.token))
                    .header("Accept", "application/vnd.github+json")
                    .header("User-Agent", self.client.user_agent());

                // Send request
                let response = request.send().await
                    .map_err(|e| ApiError::HttpClientError {
                        message: format!("Request failed: {}", e),
                    })?;

                // Check rate limit headers
                if let Some(info) = parse_rate_limit_headers(response.headers()) {
                    if info.remaining == 0 {
                        return Err(ApiError::RateLimitExceeded {
                            reset_at: OffsetDateTime::from_unix_timestamp(info.reset).ok(),
                        });
                    }
                }

                // Check status
                if response.status() == 429 {
                    let reset = response.headers()
                        .get("x-ratelimit-reset")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<i64>().ok());

                    return Err(ApiError::RateLimitExceeded {
                        reset_at: reset.and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok()),
                    });
                }

                Ok(response)
            },
            &policy,
        ).await
    }
}
```

## Usage Examples

### Custom Retry Policy

```rust
let policy = RetryPolicy {
    max_retries: 5,
    initial_backoff: Duration::from_millis(500),
    max_backoff: Duration::from_secs(30),
    backoff_multiplier: 1.5,
};
```

### Manual Retry Check

```rust
let error = ApiError::Timeout;
let decision = should_retry(&error, 0, &RetryPolicy::default());

match decision {
    RetryDecision::Retry { after } => {
        println!("Retrying after {:?}", after);
        tokio::time::sleep(after).await;
    }
    RetryDecision::DontRetry => {
        println!("Not retrying");
    }
}
```

## Error Classification

### Retryable Errors

Always retry (with backoff):

- `ApiError::Timeout`
- `ApiError::HttpClientError` (network issues)
- `ApiError::HttpError` with status 500, 502, 503, 504

### Conditional Retry

- `ApiError::RateLimitExceeded` → Retry after reset time
- `ApiError::HttpError` with status 429 → Retry after reset

### Non-Retryable Errors

Never retry:

- `ApiError::NotFound` (404)
- `ApiError::PermissionDenied` (403 without rate limit)
- `ApiError::ValidationError` (422)
- `ApiError::HttpError` with 4xx status (except 429)

## Testing Strategy

- Mock HTTP responses with retry-triggering errors
- Test exponential backoff calculation
- Test rate limit header parsing
- Verify retry count limits
- Test rate limit reset time calculations

## Performance Considerations

- Retries increase latency for failed requests
- Rate limit delays can be significant (up to 1 hour for primary limit)
- Consider user experience when setting max_retries
- Log retries for observability

## References

- GitHub API: [Rate Limits](https://docs.github.com/en/rest/overview/resources-in-the-rest-api#rate-limiting)
- GitHub API: [Secondary Rate Limits](https://docs.github.com/en/rest/overview/resources-in-the-rest-api#secondary-rate-limits)
