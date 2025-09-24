# Security Model

The `github-bot-sdk` implements a comprehensive security model that protects GitHub App credentials, validates webhook authenticity, and ensures secure API communications.

## Security Principles

1. **Defense in Depth**: Multiple layers of security controls
2. **Least Privilege**: Minimal required permissions and access
3. **Credential Isolation**: Secure storage and handling of secrets
4. **Transport Security**: All communications encrypted in transit
5. **Audit Trail**: Comprehensive logging of security events

## Threat Model

### Identified Threats

| Threat | Impact | Mitigation |
|--------|--------|------------|
| Private key compromise | Complete app takeover | Key rotation, secure storage, no logging |
| Installation token theft | Repository access | Short-lived tokens, secure transmission |
| Webhook spoofing | Malicious event processing | HMAC signature validation |
| Man-in-the-middle attacks | Data interception | TLS enforcement, certificate pinning |
| Replay attacks | Duplicate event processing | Event deduplication, timestamp validation |
| Rate limit abuse | Service degradation | Client-side rate limiting, circuit breakers |

## Credential Management

### Private Key Security

```rust
pub struct PrivateKey {
    inner: jsonwebtoken::EncodingKey,
}

impl PrivateKey {
    /// Load private key from PEM format with validation
    pub fn from_pem(pem: &str) -> Result<Self, SecurityError> {
        // Validate key format and strength
        let key = jsonwebtoken::EncodingKey::from_rsa_pem(pem.as_bytes())
            .map_err(|e| SecurityError::InvalidPrivateKey { source: e })?;

        // Ensure minimum key size (2048 bits)
        Self::validate_key_strength(&key)?;

        Ok(Self { inner: key })
    }

    fn validate_key_strength(key: &jsonwebtoken::EncodingKey) -> Result<(), SecurityError> {
        // Implementation validates RSA key is at least 2048 bits
        // ...
    }
}

// Prevent key exposure in logs or debug output
impl Debug for PrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivateKey")
            .field("inner", &"[REDACTED]")
            .finish()
    }
}
```

### Secret Storage Integration

```rust
#[async_trait]
pub trait SecretProvider: Send + Sync {
    async fn get_private_key(&self, app_id: u64) -> Result<String, SecretError>;
    async fn get_webhook_secret(&self, repository: &Repository) -> Result<String, SecretError>;
}

// Azure Key Vault implementation
pub struct AzureKeyVaultProvider {
    client: azure_security_keyvault::KeyvaultClient,
    vault_url: String,
}

impl AzureKeyVaultProvider {
    pub fn new(vault_url: String, credential: Arc<dyn TokenCredential>) -> Self {
        let client = KeyvaultClient::new(&vault_url, credential);
        Self { client, vault_url }
    }
}

#[async_trait]
impl SecretProvider for AzureKeyVaultProvider {
    async fn get_private_key(&self, app_id: u64) -> Result<String, SecretError> {
        let secret_name = format!("github-app-{}-private-key", app_id);

        let secret = self.client
            .get_secret(&secret_name)
            .await
            .map_err(|e| SecretError::RetrievalFailed { source: e })?;

        Ok(secret.value)
    }

    async fn get_webhook_secret(&self, repository: &Repository) -> Result<String, SecretError> {
        let secret_name = format!("webhook-secret-{}", repository.full_name.replace('/', '-'));

        let secret = self.client
            .get_secret(&secret_name)
            .await
            .map_err(|e| SecretError::RetrievalFailed { source: e })?;

        Ok(secret.value)
    }
}
```

### Token Security

```rust
/// Secure wrapper for sensitive strings that prevents logging
#[derive(Clone)]
pub struct SecretString {
    inner: String,
}

impl SecretString {
    pub fn new(value: String) -> Self {
        Self { inner: value }
    }

    /// Expose the secret value - use carefully
    pub fn expose(&self) -> &str {
        &self.inner
    }

    /// Create authorization header safely
    pub fn authorization_header(&self) -> String {
        format!("Bearer {}", self.inner)
    }
}

impl Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        // Zero out memory on drop
        unsafe {
            std::ptr::write_volatile(
                self.inner.as_mut_ptr(),
                0u8,
            );
        }
    }
}
```

## Authentication Security

### JWT Security

```rust
impl GitHubAppAuth {
    pub fn generate_jwt(&self) -> Result<String, AuthError> {
        let now = Utc::now();
        let expiration = now + self.config.jwt_expiration;

        // Ensure JWT doesn't exceed GitHub's 10-minute maximum
        if self.config.jwt_expiration > Duration::from_secs(600) {
            return Err(AuthError::InvalidJwtExpiration);
        }

        let claims = JwtClaims {
            iss: self.app_id.to_string(),
            iat: now.timestamp(),
            exp: expiration.timestamp(),
        };

        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);

        jsonwebtoken::encode(&header, &claims, &self.private_key.inner)
            .map_err(|e| AuthError::JwtGeneration { source: e })
    }
}

#[derive(Serialize)]
struct JwtClaims {
    iss: String,  // Issuer (App ID)
    iat: i64,     // Issued at
    exp: i64,     // Expiration
}
```

### Installation Token Management

```rust
pub struct TokenSecurity {
    min_remaining_time: Duration,
    max_age: Duration,
}

impl InstallationToken {
    pub fn is_secure(&self, security: &TokenSecurity) -> bool {
        let now = Utc::now();

        // Check if token expires soon
        if self.expires_at - now < security.min_remaining_time {
            return false;
        }

        // Check if token is too old (potential replay)
        let age = now - (self.expires_at - Duration::from_secs(3600));
        if age > security.max_age {
            return false;
        }

        true
    }

    pub fn validate_permissions(&self, required: &Permissions) -> Result<(), SecurityError> {
        // Ensure token has required permissions
        if !self.permissions.contains(required) {
            return Err(SecurityError::InsufficientPermissions {
                required: required.clone(),
                available: self.permissions.clone(),
            });
        }

        Ok(())
    }
}
```

## Webhook Security

### Signature Validation

```rust
pub struct SignatureValidator {
    secrets: Arc<dyn SecretProvider>,
    timing_safe_compare: bool,
}

impl SignatureValidator {
    pub async fn validate(
        &self,
        payload: &[u8],
        signature: &str,
        repository: &Repository,
    ) -> Result<(), ValidationError> {
        // Parse signature header
        let signature_bytes = self.parse_signature(signature)?;

        // Get webhook secret for repository
        let secret = self.secrets
            .get_webhook_secret(repository)
            .await
            .map_err(|e| ValidationError::SecretRetrieval { source: e })?;

        // Compute expected HMAC
        let expected = self.compute_hmac(payload, &secret)?;

        // Timing-safe comparison to prevent timing attacks
        if self.timing_safe_compare {
            self.constant_time_compare(&signature_bytes, &expected)?;
        } else {
            if signature_bytes != expected {
                return Err(ValidationError::HmacMismatch);
            }
        }

        Ok(())
    }

    fn parse_signature(&self, signature: &str) -> Result<Vec<u8>, ValidationError> {
        if !signature.starts_with("sha256=") {
            return Err(ValidationError::InvalidSignatureFormat);
        }

        let hex_signature = &signature[7..];
        hex::decode(hex_signature)
            .map_err(|_| ValidationError::InvalidSignatureFormat)
    }

    fn compute_hmac(&self, payload: &[u8], secret: &str) -> Result<Vec<u8>, ValidationError> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|_| ValidationError::InvalidSecret)?;

        mac.update(payload);
        Ok(mac.finalize().into_bytes().to_vec())
    }

    fn constant_time_compare(&self, a: &[u8], b: &[u8]) -> Result<(), ValidationError> {
        use subtle::ConstantTimeEq;

        if a.len() != b.len() {
            return Err(ValidationError::HmacMismatch);
        }

        if a.ct_eq(b).into() {
            Ok(())
        } else {
            Err(ValidationError::HmacMismatch)
        }
    }
}
```

### Payload Validation

```rust
pub struct PayloadValidator {
    max_size: usize,
    required_headers: Vec<String>,
}

impl PayloadValidator {
    pub fn validate_request(
        &self,
        headers: &HeaderMap,
        payload: &[u8],
    ) -> Result<(), ValidationError> {
        // Check payload size
        if payload.len() > self.max_size {
            return Err(ValidationError::PayloadTooLarge {
                size: payload.len(),
                max_size: self.max_size,
            });
        }

        // Validate required headers
        for header_name in &self.required_headers {
            if !headers.contains_key(header_name) {
                return Err(ValidationError::MissingHeader {
                    header: header_name.clone(),
                });
            }
        }

        // Validate content type
        if let Some(content_type) = headers.get("content-type") {
            if content_type != "application/json" {
                return Err(ValidationError::InvalidContentType);
            }
        }

        Ok(())
    }
}
```

## Transport Security

### TLS Configuration

```rust
pub struct TlsConfig {
    pub min_version: TlsVersion,
    pub cipher_suites: Vec<CipherSuite>,
    pub verify_hostname: bool,
    pub certificate_pinning: Option<Vec<String>>,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            min_version: TlsVersion::V1_2,
            cipher_suites: vec![
                CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
                CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
            ],
            verify_hostname: true,
            certificate_pinning: None,
        }
    }
}

pub fn create_secure_client(config: &TlsConfig) -> Result<reqwest::Client, SecurityError> {
    let client_builder = reqwest::Client::builder()
        .min_tls_version(config.min_version.into())
        .tls_built_in_root_certs(true);

    // Configure certificate pinning if specified
    if let Some(pins) = &config.certificate_pinning {
        // Implementation would add certificate pinning
        // This requires custom TLS configuration
    }

    client_builder
        .build()
        .map_err(|e| SecurityError::TlsConfiguration { source: e })
}
```

## Security Monitoring

### Audit Logging

```rust
#[derive(Debug, Serialize)]
pub struct SecurityEvent {
    pub event_type: SecurityEventType,
    pub timestamp: DateTime<Utc>,
    pub correlation_id: String,
    pub app_id: Option<u64>,
    pub installation_id: Option<u64>,
    pub repository: Option<String>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub outcome: SecurityOutcome,
    pub details: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub enum SecurityEventType {
    AuthenticationAttempt,
    TokenRefresh,
    WebhookValidation,
    PermissionCheck,
    RateLimitExceeded,
    SuspiciousActivity,
}

#[derive(Debug, Serialize)]
pub enum SecurityOutcome {
    Success,
    Failure,
    Blocked,
}

impl SecurityEvent {
    pub fn authentication_success(app_id: u64, installation_id: u64) -> Self {
        Self {
            event_type: SecurityEventType::AuthenticationAttempt,
            timestamp: Utc::now(),
            correlation_id: uuid::Uuid::new_v4().to_string(),
            app_id: Some(app_id),
            installation_id: Some(installation_id),
            repository: None,
            user_agent: None,
            ip_address: None,
            outcome: SecurityOutcome::Success,
            details: json!({}),
        }
    }

    pub fn webhook_validation_failed(repository: &str, reason: &str) -> Self {
        Self {
            event_type: SecurityEventType::WebhookValidation,
            timestamp: Utc::now(),
            correlation_id: uuid::Uuid::new_v4().to_string(),
            app_id: None,
            installation_id: None,
            repository: Some(repository.to_string()),
            user_agent: None,
            ip_address: None,
            outcome: SecurityOutcome::Failure,
            details: json!({ "reason": reason }),
        }
    }
}
```

### Threat Detection

```rust
pub struct ThreatDetector {
    patterns: Vec<ThreatPattern>,
    rate_limits: HashMap<String, RateLimit>,
}

#[derive(Debug)]
pub struct ThreatPattern {
    pub name: String,
    pub condition: Box<dyn Fn(&SecurityEvent) -> bool + Send + Sync>,
    pub severity: ThreatSeverity,
    pub action: ThreatAction,
}

#[derive(Debug)]
pub enum ThreatSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug)]
pub enum ThreatAction {
    Log,
    Alert,
    Block,
    Quarantine,
}

impl ThreatDetector {
    pub fn analyze_event(&self, event: &SecurityEvent) -> Vec<ThreatAlert> {
        let mut alerts = Vec::new();

        for pattern in &self.patterns {
            if (pattern.condition)(event) {
                alerts.push(ThreatAlert {
                    pattern_name: pattern.name.clone(),
                    severity: pattern.severity,
                    event: event.clone(),
                    recommended_action: pattern.action,
                });
            }
        }

        alerts
    }
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("Invalid private key: {source}")]
    InvalidPrivateKey { source: Box<dyn std::error::Error + Send + Sync> },

    #[error("JWT expiration exceeds maximum allowed duration")]
    InvalidJwtExpiration,

    #[error("Insufficient permissions: required {required:?}, available {available:?}")]
    InsufficientPermissions { required: Permissions, available: Permissions },

    #[error("Token validation failed: {reason}")]
    TokenValidation { reason: String },

    #[error("TLS configuration error: {source}")]
    TlsConfiguration { source: reqwest::Error },

    #[error("Secret retrieval failed: {source}")]
    SecretRetrieval { source: Box<dyn std::error::Error + Send + Sync> },
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid signature format")]
    InvalidSignatureFormat,

    #[error("HMAC validation failed")]
    HmacMismatch,

    #[error("Payload too large: {size} bytes (max: {max_size})")]
    PayloadTooLarge { size: usize, max_size: usize },

    #[error("Missing required header: {header}")]
    MissingHeader { header: String },

    #[error("Invalid content type")]
    InvalidContentType,

    #[error("Secret retrieval error: {source}")]
    SecretRetrieval { source: Box<dyn std::error::Error + Send + Sync> },
}
```

## Best Practices

### Development Guidelines

1. **Never log secrets**: Use redacted debug implementations
2. **Validate all inputs**: Check sizes, formats, and contents
3. **Use timing-safe comparisons**: Prevent timing attacks on secrets
4. **Implement proper error handling**: Don't leak sensitive information
5. **Regular key rotation**: Establish processes for credential updates
6. **Monitor security events**: Implement comprehensive audit logging

### Deployment Security

1. **Environment isolation**: Separate dev/staging/prod credentials
2. **Secret management**: Use dedicated secret storage services
3. **Network security**: Implement proper firewall and access controls
4. **Regular updates**: Keep dependencies updated for security patches
5. **Security scanning**: Regular vulnerability assessments
6. **Incident response**: Established procedures for security incidents

### Configuration Examples

```rust
// Production security configuration
let security_config = SecurityConfig {
    private_key_source: PrivateKeySource::KeyVault {
        vault_url: "https://mybot-vault.vault.azure.net/".to_string(),
        secret_name: "github-app-private-key".to_string(),
    },
    webhook_validation: WebhookValidation {
        enforce_signatures: true,
        timing_safe_compare: true,
        max_payload_size: 1024 * 1024, // 1MB
        required_headers: vec![
            "X-GitHub-Event".to_string(),
            "X-Hub-Signature-256".to_string(),
        ],
    },
    token_security: TokenSecurity {
        min_remaining_time: Duration::from_secs(300), // 5 minutes
        max_age: Duration::from_secs(3300), // 55 minutes
    },
    tls_config: TlsConfig {
        min_version: TlsVersion::V1_2,
        verify_hostname: true,
        certificate_pinning: Some(vec![
            "sha256/k2v657xBsOVe1PQRwOsHsw3bsGT2VzIqz5K+59sNQws=".to_string(),
        ]),
        ..Default::default()
    },
    audit_logging: AuditConfig {
        log_all_events: true,
        log_payloads: false, // Never log sensitive payloads
        retention_days: 90,
    },
};
```
