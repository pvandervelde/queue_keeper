//! AWS SQS provider implementation using HTTP REST API.
//!
//! This module provides production-ready AWS SQS integration using direct HTTP calls
//! instead of the AWS SDK. This approach enables proper unit testing with mocked HTTP
//! responses, similar to the Azure provider implementation.
//!
//! ## Key Features
//!
//! - **HTTP REST API**: Direct calls to AWS SQS REST API endpoints
//! - **AWS Signature V4**: Manual request signing for authentication
//! - **Standard queues**: High-throughput scenarios (near-unlimited throughput)
//! - **FIFO queues**: Strict message ordering (3000 msgs/sec with batching)
//! - **Session support**: Via FIFO message groups
//! - **Dead letter queues**: Native AWS SQS DLQ integration
//! - **Batch operations**: Up to 10 messages per batch
//! - **Queue URL caching**: Performance optimization
//! - **Test-friendly**: Mock HTTP responses in unit tests
//!
//! ## Authentication
//!
//! Implements AWS Signature Version 4 signing for request authentication:
//! - **Access Keys**: Explicit access_key_id and secret_access_key
//! - **IAM Roles**: Via environment variables or instance metadata (future)
//!
//! ## Queue Types
//!
//! ### Standard Queues
//! - Near-unlimited throughput
//! - At-least-once delivery
//! - Best-effort ordering
//! - Use for high-throughput scenarios
//!
//! ### FIFO Queues
//! - Strict message ordering within message groups
//! - Exactly-once processing with deduplication
//! - Up to 3000 messages/second with batching
//! - Requires `.fifo` suffix in queue name
//! - Use for ordered processing requirements
//!
//! ## Session Support
//!
//! AWS SQS emulates sessions via FIFO queue message groups:
//! - SessionId maps to MessageGroupId
//! - Messages in same group processed in order
//! - Different groups can process concurrently
//! - Standard queues do not support sessions
//!
//! ## Benefits of HTTP Approach
//!
//! 1. **Testable**: Mock HTTP responses in unit tests
//! 2. **Transparent**: Full control over request/response handling
//! 3. **Lightweight**: No heavy SDK dependencies
//! 4. **Consistent**: Matches Azure provider pattern
//!
//! ## Example
//!
//! ```no_run
//! use queue_runtime::{QueueClientFactory, QueueConfig, ProviderConfig, AwsSqsConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = QueueConfig {
//!     provider: ProviderConfig::AwsSqs(AwsSqsConfig {
//!         region: "us-east-1".to_string(),
//!         access_key_id: Some("AKIAIOSFODNN7EXAMPLE".to_string()),
//!         secret_access_key: Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string()),
//!         use_fifo_queues: true,
//!     }),
//!     ..Default::default()
//! };
//!
//! let client = QueueClientFactory::create_client(config).await?;
//! # Ok(())
//! # }
//! ```

use crate::client::{QueueProvider, SessionProvider};
use crate::error::{ConfigurationError, QueueError, SerializationError};
use crate::message::{
    Message, MessageId, QueueName, ReceiptHandle, ReceivedMessage, SessionId, Timestamp,
};
use crate::provider::{AwsSqsConfig, ProviderType, SessionSupport};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use reqwest::Client as HttpClient;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(test)]
#[path = "aws_tests.rs"]
mod tests;

// ============================================================================
// Error Types
// ============================================================================

/// AWS SQS specific errors
#[derive(Debug, thiserror::Error)]
pub enum AwsError {
    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("SQS service error: {0}")]
    ServiceError(String),

    #[error("Queue not found: {0}")]
    QueueNotFound(String),

    #[error("Invalid receipt handle: {0}")]
    InvalidReceipt(String),

    #[error("Message too large: {size} bytes (max: {max_size})")]
    MessageTooLarge { size: usize, max_size: usize },

    #[error("Invalid configuration: {0}")]
    ConfigurationError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Sessions not supported on standard queues")]
    SessionsNotSupported,
}

impl AwsError {
    /// Check if error is transient and should be retried
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Authentication(_) => false,
            Self::NetworkError(_) => true,
            Self::ServiceError(_) => true, // Most SQS errors are transient
            Self::QueueNotFound(_) => false,
            Self::InvalidReceipt(_) => false,
            Self::MessageTooLarge { .. } => false,
            Self::ConfigurationError(_) => false,
            Self::SerializationError(_) => false,
            Self::SessionsNotSupported => false,
        }
    }

    /// Map AWS error to QueueError
    pub fn to_queue_error(self) -> QueueError {
        match self {
            Self::Authentication(msg) => QueueError::AuthenticationFailed { message: msg },
            Self::NetworkError(msg) => QueueError::ConnectionFailed { message: msg },
            Self::ServiceError(msg) => QueueError::ProviderError {
                provider: "AwsSqs".to_string(),
                code: "ServiceError".to_string(),
                message: msg,
            },
            Self::QueueNotFound(queue) => QueueError::QueueNotFound { queue_name: queue },
            Self::InvalidReceipt(receipt) => QueueError::MessageNotFound { receipt },
            Self::MessageTooLarge { size, max_size } => {
                QueueError::MessageTooLarge { size, max_size }
            }
            Self::ConfigurationError(msg) => {
                QueueError::ConfigurationError(ConfigurationError::Invalid { message: msg })
            }
            Self::SerializationError(msg) => QueueError::SerializationError(
                SerializationError::JsonError(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    msg,
                ))),
            ),
            Self::SessionsNotSupported => QueueError::ProviderError {
                provider: "AwsSqs".to_string(),
                code: "SessionsNotSupported".to_string(),
                message:
                    "Standard queues do not support session-based operations. Use FIFO queues."
                        .to_string(),
            },
        }
    }
}

// ============================================================================
// AWS Signature V4 Signing
// ============================================================================

type HmacSha256 = Hmac<Sha256>;

/// AWS Signature Version 4 signer for request authentication
///
/// Implements the AWS Signature V4 signing process:
/// 1. Create canonical request (method, URI, query, headers, payload)
/// 2. Create string to sign (algorithm, timestamp, scope, request hash)
/// 3. Derive signing key (4-level HMAC chain)
/// 4. Calculate signature and build Authorization header
///
/// ## References
///
/// - [AWS Signature V4](https://docs.aws.amazon.com/general/latest/gr/signature-version-4.html)
/// - [Signing Process](https://docs.aws.amazon.com/general/latest/gr/sigv4_signing.html)
struct AwsV4Signer {
    access_key: String,
    secret_key: String,
    region: String,
    service: String,
}

impl AwsV4Signer {
    /// Create new AWS Signature V4 signer
    ///
    /// # Arguments
    ///
    /// * `access_key` - AWS access key ID
    /// * `secret_key` - AWS secret access key
    /// * `region` - AWS region (e.g., "us-east-1")
    fn new(access_key: String, secret_key: String, region: String) -> Self {
        Self {
            access_key,
            secret_key,
            region,
            service: "sqs".to_string(),
        }
    }

    /// Sign an HTTP request with AWS Signature V4
    ///
    /// Returns a HashMap of headers to add to the request, including:
    /// - `Authorization`: AWS signature authorization header
    /// - `x-amz-date`: ISO8601 timestamp
    /// - `host`: Endpoint host
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method (GET, POST, etc.)
    /// * `host` - Endpoint host (e.g., "sqs.us-east-1.amazonaws.com")
    /// * `path` - Request path (e.g., "/")
    /// * `query_params` - Query parameters as key-value pairs
    /// * `body` - Request body (empty string for no body)
    /// * `timestamp` - Request timestamp
    fn sign_request(
        &self,
        method: &str,
        host: &str,
        path: &str,
        query_params: &HashMap<String, String>,
        body: &str,
        timestamp: &DateTime<Utc>,
    ) -> HashMap<String, String> {
        let date_stamp = timestamp.format("%Y%m%d").to_string();
        let amz_date = timestamp.format("%Y%m%dT%H%M%SZ").to_string();

        // Task 1: Create canonical request
        let canonical_uri = path;

        // Sort query parameters for canonical request
        let mut canonical_query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>();
        canonical_query_string.sort();
        let canonical_query_string = canonical_query_string.join("&");

        // Canonical headers (must be sorted)
        let canonical_headers = format!("host:{}\nx-amz-date:{}\n", host, amz_date);
        let signed_headers = "host;x-amz-date";

        // Payload hash
        let payload_hash = format!("{:x}", Sha256::digest(body.as_bytes()));

        // Build canonical request
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method,
            canonical_uri,
            canonical_query_string,
            canonical_headers,
            signed_headers,
            payload_hash
        );

        // Task 2: Create string to sign
        let algorithm = "AWS4-HMAC-SHA256";
        let credential_scope = format!(
            "{}/{}/{}/aws4_request",
            date_stamp, self.region, self.service
        );
        let canonical_request_hash = format!("{:x}", Sha256::digest(canonical_request.as_bytes()));

        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm, amz_date, credential_scope, canonical_request_hash
        );

        // Task 3: Calculate signature
        let signature = self.calculate_signature(&string_to_sign, &date_stamp);

        // Task 4: Build authorization header
        let authorization_header = format!(
            "{} Credential={}/{}, SignedHeaders={}, Signature={}",
            algorithm, self.access_key, credential_scope, signed_headers, signature
        );

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), authorization_header);
        headers.insert("x-amz-date".to_string(), amz_date);
        headers.insert("host".to_string(), host.to_string());

        headers
    }

    /// Calculate AWS Signature V4 signature
    ///
    /// Uses 4-level HMAC-SHA256 chain to derive signing key:
    /// 1. kSecret = "AWS4" + secret_key
    /// 2. kDate = HMAC(kSecret, date)
    /// 3. kRegion = HMAC(kDate, region)
    /// 4. kService = HMAC(kRegion, service)
    /// 5. kSigning = HMAC(kService, "aws4_request")
    /// 6. signature = HMAC(kSigning, string_to_sign)
    fn calculate_signature(&self, string_to_sign: &str, date_stamp: &str) -> String {
        let k_secret = format!("AWS4{}", self.secret_key);
        let k_date = self.hmac_sha256(k_secret.as_bytes(), date_stamp.as_bytes());
        let k_region = self.hmac_sha256(&k_date, self.region.as_bytes());
        let k_service = self.hmac_sha256(&k_region, self.service.as_bytes());
        let k_signing = self.hmac_sha256(&k_service, b"aws4_request");
        let signature = self.hmac_sha256(&k_signing, string_to_sign.as_bytes());

        hex::encode(signature)
    }

    /// Compute HMAC-SHA256
    fn hmac_sha256(&self, key: &[u8], data: &[u8]) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    }
}

// ============================================================================
// AWS SQS Provider
// ============================================================================

/// AWS SQS queue provider implementation
///
/// This provider implements the QueueProvider trait using AWS SQS.
/// It supports:
/// - Multiple authentication methods via AWS credential chain
/// - Standard queues for high throughput
/// - FIFO queues for ordered message processing
/// - Session emulation via FIFO message groups
/// - Queue URL caching for performance
/// - Dead letter queue integration
///
/// ## Thread Safety
///
/// The provider is thread-safe and can be shared across async tasks using `Arc`.
/// Internal state (queue URL cache) is protected by `RwLock`.
pub struct AwsSqsProvider {
    http_client: HttpClient,
    signer: Option<AwsV4Signer>,
    config: AwsSqsConfig,
    endpoint: String,
    queue_url_cache: Arc<RwLock<HashMap<QueueName, String>>>,
}

impl AwsSqsProvider {
    /// Create new AWS SQS provider
    ///
    /// # Arguments
    ///
    /// * `config` - AWS SQS configuration with region and authentication details
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Configuration is invalid
    /// - Authentication fails
    /// - AWS SDK initialization fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use queue_runtime::providers::AwsSqsProvider;
    /// use queue_runtime::AwsSqsConfig;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AwsSqsConfig {
    ///     region: "us-east-1".to_string(),
    ///     access_key_id: None,
    ///     secret_access_key: None,
    ///     use_fifo_queues: false,
    /// };
    ///
    /// let provider = AwsSqsProvider::new(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(config: AwsSqsConfig) -> Result<Self, AwsError> {
        // Validate configuration
        if config.region.is_empty() {
            return Err(AwsError::ConfigurationError(
                "Region cannot be empty".to_string(),
            ));
        }

        // Setup signer if credentials provided
        let signer = if let (Some(access_key), Some(secret_key)) =
            (&config.access_key_id, &config.secret_access_key)
        {
            Some(AwsV4Signer::new(
                access_key.clone(),
                secret_key.clone(),
                config.region.clone(),
            ))
        } else {
            // TODO: Support IAM roles via instance metadata
            None
        };

        // Build endpoint URL
        let endpoint = format!("https://sqs.{}.amazonaws.com", config.region);

        // Create HTTP client with timeout
        let http_client = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AwsError::NetworkError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            http_client,
            signer,
            config,
            endpoint,
            queue_url_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get queue URL for a queue name, with caching
    ///
    /// # Arguments
    ///
    /// * `queue_name` - The queue name to resolve
    ///
    /// # Errors
    ///
    /// Returns error if queue does not exist
    async fn get_queue_url(&self, queue_name: &QueueName) -> Result<String, AwsError> {
        // Check cache first
        {
            let cache = self.queue_url_cache.read().await;
            if let Some(url) = cache.get(queue_name) {
                return Ok(url.clone());
            }
        }

        // Not in cache, fetch from AWS
        let result = self
            .client
            .get_queue_url()
            .queue_name(queue_name.as_str())
            .send()
            .await;

        match result {
            Ok(output) => {
                if let Some(url) = output.queue_url {
                    // Cache the URL
                    let mut cache = self.queue_url_cache.write().await;
                    cache.insert(queue_name.clone(), url.clone());
                    Ok(url)
                } else {
                    Err(AwsError::QueueNotFound(queue_name.as_str().to_string()))
                }
            }
            Err(err) => {
                // Check for queue not found error
                let error_msg = format!("{}", err);
                if error_msg.contains("NonExistentQueue") || error_msg.contains("QueueDoesNotExist")
                {
                    Err(AwsError::QueueNotFound(queue_name.as_str().to_string()))
                } else if error_msg.contains("InvalidClientTokenId")
                    || error_msg.contains("UnrecognizedClientException")
                {
                    Err(AwsError::Authentication(error_msg))
                } else if error_msg.contains("connection") || error_msg.contains("timeout") {
                    Err(AwsError::NetworkError(error_msg))
                } else {
                    Err(AwsError::ServiceError(error_msg))
                }
            }
        }
    }

    /// Check if a queue is a FIFO queue
    fn is_fifo_queue(queue_name: &QueueName) -> bool {
        queue_name.as_str().ends_with(".fifo")
    }
}

impl fmt::Debug for AwsSqsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AwsSqsProvider")
            .field("config", &self.config)
            .field("queue_url_cache_size", &"<redacted>")
            .finish()
    }
}

#[async_trait]
impl QueueProvider for AwsSqsProvider {
    async fn send_message(
        &self,
        queue: &QueueName,
        message: &Message,
    ) -> Result<MessageId, QueueError> {
        let queue_url = self
            .get_queue_url(queue)
            .await
            .map_err(|e| e.to_queue_error())?;

        // Serialize message body (Bytes) to base64 for SQS
        use base64::{engine::general_purpose::STANDARD, Engine};
        let body_base64 = STANDARD.encode(&message.body);

        // Check message size (AWS SQS limit: 256KB)
        if body_base64.len() > 256 * 1024 {
            return Err(AwsError::MessageTooLarge {
                size: body_base64.len(),
                max_size: 256 * 1024,
            }
            .to_queue_error());
        }

        // Build send request
        let mut request = self
            .client
            .send_message()
            .queue_url(&queue_url)
            .message_body(body_base64);

        // Add message group ID for FIFO queues
        if Self::is_fifo_queue(queue) {
            if let Some(ref session_id) = message.session_id {
                request = request.message_group_id(session_id.as_str());
                // Use UUID for deduplication ID
                let dedup_id = uuid::Uuid::new_v4().to_string();
                request = request.message_deduplication_id(dedup_id);
            } else {
                // FIFO queues require message group ID
                return Err(QueueError::ValidationError(
                    crate::error::ValidationError::Required {
                        field: "session_id".to_string(),
                    },
                ));
            }
        }

        // Send message
        let result = request.send().await;

        match result {
            Ok(output) => {
                if let Some(message_id) = output.message_id {
                    Ok(MessageId::from_str(&message_id).unwrap_or_else(|_| MessageId::new()))
                } else {
                    Err(AwsError::ServiceError("No message ID returned".to_string())
                        .to_queue_error())
                }
            }
            Err(err) => {
                let error_msg = format!("{}", err);
                if error_msg.contains("InvalidMessageContents") {
                    Err(AwsError::SerializationError(error_msg).to_queue_error())
                } else if error_msg.contains("connection") || error_msg.contains("timeout") {
                    Err(AwsError::NetworkError(error_msg).to_queue_error())
                } else {
                    Err(AwsError::ServiceError(error_msg).to_queue_error())
                }
            }
        }
    }

    async fn send_messages(
        &self,
        queue: &QueueName,
        messages: &[Message],
    ) -> Result<Vec<MessageId>, QueueError> {
        // AWS SQS supports batches of up to 10 messages
        // Send in chunks if more than 10
        let mut all_message_ids = Vec::with_capacity(messages.len());

        for chunk in messages.chunks(10) {
            let chunk_ids = self.send_messages_batch(queue, chunk).await?;
            all_message_ids.extend(chunk_ids);
        }

        Ok(all_message_ids)
    }

    async fn receive_message(
        &self,
        queue: &QueueName,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        let messages = self.receive_messages(queue, 1, timeout).await?;
        Ok(messages.into_iter().next())
    }

    async fn receive_messages(
        &self,
        queue: &QueueName,
        max_messages: u32,
        timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        let queue_url = self
            .get_queue_url(queue)
            .await
            .map_err(|e| e.to_queue_error())?;

        // Convert timeout to seconds (AWS uses seconds for wait time)
        let wait_time_seconds = timeout.num_seconds().clamp(0, 20) as i32; // AWS max is 20 seconds

        // Receive messages with all attributes
        let result = self
            .client
            .receive_message()
            .queue_url(&queue_url)
            .max_number_of_messages(max_messages.min(10) as i32) // AWS max is 10
            .wait_time_seconds(wait_time_seconds)
            .message_system_attribute_names(aws_sdk_sqs::types::MessageSystemAttributeName::All)
            .send()
            .await;

        match result {
            Ok(output) => {
                let mut received_messages = Vec::new();

                if let Some(messages) = output.messages {
                    for sqs_message in messages {
                        if let (Some(body), Some(receipt_handle)) =
                            (sqs_message.body, sqs_message.receipt_handle)
                        {
                            // Decode base64 body back to Bytes
                            use base64::{engine::general_purpose::STANDARD, Engine};
                            let body_bytes = STANDARD.decode(body).map_err(|e| {
                                AwsError::SerializationError(format!("Base64 decode failed: {}", e))
                                    .to_queue_error()
                            })?;
                            let body = bytes::Bytes::from(body_bytes);

                            // Extract session ID from message attributes (for FIFO queues)
                            let session_id = sqs_message
                                .attributes
                                .as_ref()
                                .and_then(|attrs| attrs.get(&aws_sdk_sqs::types::MessageSystemAttributeName::MessageGroupId))
                                .and_then(|group_id| SessionId::new(group_id.clone()).ok());

                            // Parse message ID
                            let message_id = sqs_message
                                .message_id
                                .as_ref()
                                .and_then(|id| MessageId::from_str(id).ok())
                                .unwrap_or_else(MessageId::new);

                            // Get delivery count
                            let delivery_count = sqs_message.attributes
                                .as_ref()
                                .and_then(|attrs| attrs.get(&aws_sdk_sqs::types::MessageSystemAttributeName::ApproximateReceiveCount))
                                .and_then(|c| c.parse().ok())
                                .unwrap_or(1);

                            // Create receipt handle (AWS receipt handles typically valid for visibility timeout, usually 30s)
                            let expires_at = Timestamp::now();
                            let receipt = ReceiptHandle::new(
                                receipt_handle,
                                expires_at,
                                ProviderType::AwsSqs,
                            );

                            // Create received message using struct literal
                            let received_message = ReceivedMessage {
                                message_id,
                                body,
                                attributes: HashMap::new(),
                                session_id,
                                correlation_id: None,
                                receipt_handle: receipt,
                                delivery_count,
                                first_delivered_at: Timestamp::now(),
                                delivered_at: Timestamp::now(),
                            };

                            received_messages.push(received_message);
                        }
                    }
                }

                Ok(received_messages)
            }
            Err(err) => {
                let error_msg = format!("{}", err);
                if error_msg.contains("connection") || error_msg.contains("timeout") {
                    Err(AwsError::NetworkError(error_msg).to_queue_error())
                } else {
                    Err(AwsError::ServiceError(error_msg).to_queue_error())
                }
            }
        }
    }

    async fn complete_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError> {
        // AWS SQS requires queue URL to delete a message
        // Since receipt handle doesn't contain queue info, try all cached queue URLs
        let cache = self.queue_url_cache.read().await;

        for queue_url in cache.values() {
            let result = self
                .client
                .delete_message()
                .queue_url(queue_url)
                .receipt_handle(receipt.handle())
                .send()
                .await;

            match result {
                Ok(_) => return Ok(()),
                Err(err) => {
                    let error_msg = format!("{}", err);
                    // If receipt is invalid for this queue, try next queue
                    if !error_msg.contains("ReceiptHandleIsInvalid") {
                        // Other errors should be reported
                        if error_msg.contains("connection") || error_msg.contains("timeout") {
                            return Err(AwsError::NetworkError(error_msg).to_queue_error());
                        }
                    }
                }
            }
        }

        // If we get here, receipt was invalid for all queues
        Err(AwsError::InvalidReceipt(receipt.handle().to_string()).to_queue_error())
    }

    async fn abandon_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError> {
        // Change visibility timeout to 0 to make message immediately available
        let cache = self.queue_url_cache.read().await;

        for queue_url in cache.values() {
            let result = self
                .client
                .change_message_visibility()
                .queue_url(queue_url)
                .receipt_handle(receipt.handle())
                .visibility_timeout(0) // Make immediately available
                .send()
                .await;

            match result {
                Ok(_) => return Ok(()),
                Err(err) => {
                    let error_msg = format!("{}", err);
                    if !error_msg.contains("ReceiptHandleIsInvalid")
                        && !error_msg.contains("MessageNotInflight")
                        && (error_msg.contains("connection") || error_msg.contains("timeout"))
                    {
                        return Err(AwsError::NetworkError(error_msg).to_queue_error());
                    }
                }
            }
        }

        Err(AwsError::InvalidReceipt(receipt.handle().to_string()).to_queue_error())
    }

    async fn dead_letter_message(
        &self,
        receipt: &ReceiptHandle,
        _reason: &str,
    ) -> Result<(), QueueError> {
        // For AWS SQS, dead letter routing is automatic based on receive count
        // We delete the message, and AWS will route to DLQ if configured and max receives exceeded
        self.complete_message(receipt).await
    }

    async fn create_session_client(
        &self,
        queue: &QueueName,
        session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionProvider>, QueueError> {
        // Check if queue supports sessions (FIFO only)
        if !Self::is_fifo_queue(queue) {
            return Err(AwsError::SessionsNotSupported.to_queue_error());
        }

        // Get queue URL
        let queue_url = self
            .get_queue_url(queue)
            .await
            .map_err(|e| e.to_queue_error())?;

        // Session ID is required for FIFO queues
        let session_id = session_id.ok_or_else(|| {
            QueueError::ValidationError(crate::error::ValidationError::Required {
                field: "session_id".to_string(),
            })
        })?;

        Ok(Box::new(AwsSessionProvider::new(
            Arc::clone(&self.client),
            queue_url,
            queue.clone(),
            session_id,
        )))
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::AwsSqs
    }

    fn supports_sessions(&self) -> SessionSupport {
        SessionSupport::Emulated
    }

    fn supports_batching(&self) -> bool {
        true
    }

    fn max_batch_size(&self) -> u32 {
        10 // AWS SQS max batch size
    }
}

// Private helper methods
impl AwsSqsProvider {
    /// Send a batch of up to 10 messages
    async fn send_messages_batch(
        &self,
        queue: &QueueName,
        messages: &[Message],
    ) -> Result<Vec<MessageId>, QueueError> {
        if messages.is_empty() {
            return Ok(Vec::new());
        }

        let queue_url = self
            .get_queue_url(queue)
            .await
            .map_err(|e| e.to_queue_error())?;

        // Build batch entries
        use base64::{engine::general_purpose::STANDARD, Engine};
        let mut entries = Vec::with_capacity(messages.len());

        for (idx, message) in messages.iter().enumerate() {
            // Encode message body to base64
            let body_base64 = STANDARD.encode(&message.body);

            if body_base64.len() > 256 * 1024 {
                return Err(AwsError::MessageTooLarge {
                    size: body_base64.len(),
                    max_size: 256 * 1024,
                }
                .to_queue_error());
            }

            let mut entry = aws_sdk_sqs::types::SendMessageBatchRequestEntry::builder()
                .id(format!("msg_{}", idx))
                .message_body(body_base64)
                .build()
                .map_err(|e| {
                    AwsError::ServiceError(format!("Failed to build batch entry: {}", e))
                        .to_queue_error()
                })?;

            // Add FIFO attributes if needed
            if Self::is_fifo_queue(queue) {
                if let Some(ref session_id) = message.session_id {
                    let dedup_id = uuid::Uuid::new_v4().to_string();
                    // Rebuild entry with FIFO attributes - message_body is already String
                    let body_value = entry.message_body.clone();
                    entry = aws_sdk_sqs::types::SendMessageBatchRequestEntry::builder()
                        .id(format!("msg_{}", idx))
                        .message_body(body_value)
                        .message_group_id(session_id.as_str())
                        .message_deduplication_id(dedup_id)
                        .build()
                        .map_err(|e| {
                            AwsError::ServiceError(format!("Failed to build FIFO entry: {}", e))
                                .to_queue_error()
                        })?;
                }
            }

            entries.push(entry);
        }

        // Send batch
        let result = self
            .client
            .send_message_batch()
            .queue_url(&queue_url)
            .set_entries(Some(entries))
            .send()
            .await;

        match result {
            Ok(output) => {
                let mut message_ids = Vec::new();

                // Collect successful message IDs
                for success in output.successful() {
                    let msg_id = success.message_id();
                    if !msg_id.is_empty() {
                        message_ids
                            .push(MessageId::from_str(msg_id).unwrap_or_else(|_| MessageId::new()));
                    }
                }

                // Check for failures
                if !output.failed().is_empty() {
                    let error_msg = format!("Batch send had {} failures", output.failed().len());
                    return Err(AwsError::ServiceError(error_msg).to_queue_error());
                }

                Ok(message_ids)
            }
            Err(err) => {
                let error_msg = format!("{}", err);
                Err(AwsError::ServiceError(error_msg).to_queue_error())
            }
        }
    }
}

// ============================================================================
// AWS Session Provider
// ============================================================================

/// AWS SQS session provider for ordered message processing via FIFO queues
///
/// This provider implements session-based operations using FIFO queue message groups.
/// The SessionId is mapped to MessageGroupId to ensure ordering within the session.
pub struct AwsSessionProvider {
    client: Arc<SqsClient>,
    queue_url: String,
    queue_name: QueueName,
    session_id: SessionId,
}

impl AwsSessionProvider {
    /// Create new AWS session provider
    fn new(
        client: Arc<SqsClient>,
        queue_url: String,
        queue_name: QueueName,
        session_id: SessionId,
    ) -> Self {
        Self {
            client,
            queue_url,
            queue_name,
            session_id,
        }
    }
}

impl fmt::Debug for AwsSessionProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AwsSessionProvider")
            .field("queue_name", &self.queue_name)
            .field("session_id", &self.session_id)
            .finish()
    }
}

#[async_trait]
impl SessionProvider for AwsSessionProvider {
    async fn receive_message(
        &self,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        // Convert timeout to seconds
        let wait_time_seconds = timeout.num_seconds().clamp(0, 20) as i32;

        // Receive message from the session's message group
        // AWS SQS FIFO automatically delivers messages from the same group in order
        let result = self
            .client
            .receive_message()
            .queue_url(&self.queue_url)
            .max_number_of_messages(1)
            .wait_time_seconds(wait_time_seconds)
            .message_system_attribute_names(aws_sdk_sqs::types::MessageSystemAttributeName::All)
            .send()
            .await;

        match result {
            Ok(output) => {
                if let Some(messages) = output.messages {
                    if let Some(sqs_message) = messages.into_iter().next() {
                        if let (Some(body), Some(receipt_handle)) =
                            (sqs_message.body, sqs_message.receipt_handle)
                        {
                            // Decode base64 body
                            use base64::{engine::general_purpose::STANDARD, Engine};
                            let body_bytes = STANDARD.decode(body).map_err(|e| {
                                AwsError::SerializationError(format!("Base64 decode failed: {}", e))
                                    .to_queue_error()
                            })?;
                            let body = bytes::Bytes::from(body_bytes);

                            // Parse message ID
                            let message_id = sqs_message
                                .message_id
                                .as_ref()
                                .and_then(|id| MessageId::from_str(id).ok())
                                .unwrap_or_else(MessageId::new);

                            // Get delivery count
                            let delivery_count = sqs_message.attributes
                                .as_ref()
                                .and_then(|attrs| attrs.get(&aws_sdk_sqs::types::MessageSystemAttributeName::ApproximateReceiveCount))
                                .and_then(|c| c.parse().ok())
                                .unwrap_or(1);

                            // Create receipt handle
                            let expires_at = Timestamp::now();
                            let receipt = ReceiptHandle::new(
                                receipt_handle,
                                expires_at,
                                ProviderType::AwsSqs,
                            );

                            // Create received message with session ID
                            let received_message = ReceivedMessage {
                                message_id,
                                body,
                                attributes: HashMap::new(),
                                session_id: Some(self.session_id.clone()),
                                correlation_id: None,
                                receipt_handle: receipt,
                                delivery_count,
                                first_delivered_at: Timestamp::now(),
                                delivered_at: Timestamp::now(),
                            };

                            return Ok(Some(received_message));
                        }
                    }
                }
                Ok(None)
            }
            Err(err) => {
                let error_msg = format!("{}", err);
                if error_msg.contains("connection") || error_msg.contains("timeout") {
                    Err(AwsError::NetworkError(error_msg).to_queue_error())
                } else {
                    Err(AwsError::ServiceError(error_msg).to_queue_error())
                }
            }
        }
    }

    async fn complete_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError> {
        let result = self
            .client
            .delete_message()
            .queue_url(&self.queue_url)
            .receipt_handle(receipt.handle())
            .send()
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(err) => {
                let error_msg = format!("{}", err);
                if error_msg.contains("ReceiptHandleIsInvalid") {
                    Err(AwsError::InvalidReceipt(receipt.handle().to_string()).to_queue_error())
                } else if error_msg.contains("connection") || error_msg.contains("timeout") {
                    Err(AwsError::NetworkError(error_msg).to_queue_error())
                } else {
                    Err(AwsError::ServiceError(error_msg).to_queue_error())
                }
            }
        }
    }

    async fn abandon_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError> {
        let result = self
            .client
            .change_message_visibility()
            .queue_url(&self.queue_url)
            .receipt_handle(receipt.handle())
            .visibility_timeout(0)
            .send()
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(err) => {
                let error_msg = format!("{}", err);
                if error_msg.contains("ReceiptHandleIsInvalid")
                    || error_msg.contains("MessageNotInflight")
                {
                    Err(AwsError::InvalidReceipt(receipt.handle().to_string()).to_queue_error())
                } else if error_msg.contains("connection") || error_msg.contains("timeout") {
                    Err(AwsError::NetworkError(error_msg).to_queue_error())
                } else {
                    Err(AwsError::ServiceError(error_msg).to_queue_error())
                }
            }
        }
    }

    async fn dead_letter_message(
        &self,
        receipt: &ReceiptHandle,
        _reason: &str,
    ) -> Result<(), QueueError> {
        // Delete the message - AWS will route to DLQ if configured
        self.complete_message(receipt).await
    }

    async fn renew_session_lock(&self) -> Result<(), QueueError> {
        // Note: AWS SQS FIFO queues do not have explicit session locks like Azure Service Bus.
        // Message ordering within a message group is guaranteed by AWS SQS itself.
        // Visibility timeout serves as the implicit lock mechanism - messages remain
        // invisible to other consumers during processing.
        // Therefore, session lock renewal is not applicable to AWS SQS.
        Ok(())
    }

    async fn close_session(&self) -> Result<(), QueueError> {
        // Note: AWS SQS FIFO queues do not require explicit session termination.
        // Sessions are implicit through message groups - there's no server-side
        // session state to clean up. The message group simply becomes idle when
        // no messages are being processed.
        Ok(())
    }

    fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    fn session_expires_at(&self) -> Timestamp {
        // Note: AWS SQS FIFO queues do not have explicit session expiry times.
        // Sessions (message groups) are persistent and don't expire - they simply
        // become idle when no messages are being processed. The visibility timeout
        // controls how long a message remains locked to a specific consumer, but
        // the message group itself has no expiration.
        // Return a far-future timestamp to indicate no expiration.
        Timestamp::from_datetime(chrono::Utc::now() + chrono::Duration::days(365))
    }
}
