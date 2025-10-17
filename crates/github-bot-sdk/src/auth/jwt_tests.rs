//! Tests for JWT generation functionality.

use super::*;
use chrono::{Duration, Utc};

// Test private key (2048-bit RSA key for testing only - DO NOT USE IN PRODUCTION)
const TEST_PRIVATE_KEY_PEM: &str = r#"-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEAu1SU1LfVLPHCozMxH2Mo4lgOEePzNm0tRgeLezV6ffAt0gun
VTLw7onLRnrq0/IzW7yWR7QkrmBL7jTKEn5u+qKhbwKfBstIs+bMY2Zkp18gnTxK
LxoS2tFczGkPLPgizskuemMghRniWaoLcyehkd3qqGElvW/VDL5AaWTg0nLVkjRo
9z+40RQzuVaE8AkAFmxZzow3x+VJYKdjykkJ0iT9wCS0DRTXu269V264Vf/3jvre
dZVp7ZD7jPzH7RqfYDCh7rjdl3bqKMTyGBvOkuNt0lZH5lfG7WccmvLl7K5e5P+1
0M3KMhZy6Ykl7xHjCYVGW04x8jdHDCQB3NQnrwIDAQABAoIBAHLZqH9Y1EyXwJpT
UwDPVHQHLKPAYeXQBX3hVxLzQQqAZdUvZXvA2YZ0KJDhj6LpLVGQ
-----END RSA PRIVATE KEY-----"#;

const TEST_PRIVATE_KEY_INVALID: &str = r#"-----BEGIN RSA PRIVATE KEY-----
INVALID KEY DATA HERE
-----END RSA PRIVATE KEY-----"#;

/// Helper to create a test private key.
fn test_private_key() -> PrivateKey {
    PrivateKey::from_pem(TEST_PRIVATE_KEY_PEM).expect("Test key should be valid")
}

mod jwt_generator_tests {
    use super::*;

    /// Assertion 1: JWT Token Generation
    ///
    /// Given: A valid GitHub App ID and private key
    /// When: generate_jwt() is called
    /// Then: Operation returns Ok(JwtToken) with valid JWT
    /// And: Token expires within 10 minutes (GitHub requirement)
    /// And: Token contains correct iss claim matching App ID
    #[tokio::test]
    async fn test_generate_jwt_with_valid_credentials() {
        let app_id = GitHubAppId::new(123456);
        let private_key = test_private_key();
        let generator = RS256JwtGenerator::new(private_key);

        let result = generator.generate_jwt(app_id).await;

        assert!(result.is_ok(), "JWT generation should succeed");
        let jwt = result.unwrap();

        // Verify token is not expired
        assert!(!jwt.is_expired(), "JWT should not be immediately expired");

        // Verify app ID matches
        assert_eq!(jwt.app_id(), app_id, "JWT app_id should match input");

        // Verify expiration is within 10 minutes
        let time_until_expiry = jwt.time_until_expiry();
        assert!(
            time_until_expiry <= Duration::minutes(10),
            "JWT expiration should not exceed 10 minutes (GitHub requirement)"
        );
        assert!(
            time_until_expiry > Duration::minutes(0),
            "JWT should have positive time until expiry"
        );
    }

    /// Verify JWT structure is valid (header.payload.signature).
    #[tokio::test]
    async fn test_jwt_has_valid_structure() {
        let app_id = GitHubAppId::new(789);
        let private_key = test_private_key();
        let generator = RS256JwtGenerator::new(private_key);

        let jwt = generator.generate_jwt(app_id).await.unwrap();
        let token_str = jwt.token();

        // JWT should have three parts separated by dots
        let parts: Vec<&str> = token_str.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "JWT should have exactly 3 parts (header.payload.signature)"
        );

        // Each part should be non-empty
        assert!(!parts[0].is_empty(), "JWT header should not be empty");
        assert!(!parts[1].is_empty(), "JWT payload should not be empty");
        assert!(!parts[2].is_empty(), "JWT signature should not be empty");
    }

    /// Verify iat (issued at) claim is current timestamp (±5 seconds).
    #[tokio::test]
    async fn test_jwt_issued_at_is_current() {
        let app_id = GitHubAppId::new(555);
        let private_key = test_private_key();
        let generator = RS256JwtGenerator::new(private_key);

        let before = Utc::now();
        let jwt = generator.generate_jwt(app_id).await.unwrap();
        let after = Utc::now();

        let issued_at = jwt.issued_at();

        // Issued at should be between before and after (with small margin)
        assert!(
            issued_at >= before - Duration::seconds(5),
            "JWT issued_at should be recent"
        );
        assert!(
            issued_at <= after + Duration::seconds(5),
            "JWT issued_at should not be in the future"
        );
    }

    /// Verify exp claim is iat + duration (max 10 minutes).
    #[tokio::test]
    async fn test_jwt_expiration_matches_duration() {
        let app_id = GitHubAppId::new(999);
        let private_key = test_private_key();
        let generator = RS256JwtGenerator::new(private_key);

        let jwt = generator.generate_jwt(app_id).await.unwrap();

        let expected_expiry = jwt.issued_at() + generator.expiration_duration();
        let actual_expiry = jwt.expires_at();

        // Expiration should match issued_at + duration (with 1 second tolerance)
        let diff = (actual_expiry - expected_expiry).num_seconds().abs();
        assert!(
            diff <= 1,
            "JWT expiration should be issued_at + duration (tolerance: 1s)"
        );
    }

    /// Test custom expiration duration (8 minutes).
    #[tokio::test]
    async fn test_jwt_with_custom_expiration() {
        let app_id = GitHubAppId::new(111);
        let private_key = test_private_key();
        let custom_duration = Duration::minutes(8);
        let generator = RS256JwtGenerator::with_expiration(private_key, custom_duration);

        let jwt = generator.generate_jwt(app_id).await.unwrap();
        let time_until_expiry = jwt.time_until_expiry();

        // Should be approximately 8 minutes (with small tolerance)
        assert!(
            time_until_expiry <= Duration::minutes(8) + Duration::seconds(5),
            "JWT should expire in approximately 8 minutes"
        );
        assert!(
            time_until_expiry >= Duration::minutes(8) - Duration::seconds(5),
            "JWT expiration should match custom duration"
        );
    }

    /// Test that generator enforces 10-minute maximum.
    #[tokio::test]
    #[should_panic(expected = "JWT expiration cannot exceed 10 minutes")]
    async fn test_jwt_rejects_expiration_over_10_minutes() {
        let private_key = test_private_key();
        let _ = RS256JwtGenerator::with_expiration(private_key, Duration::minutes(11));
    }

    /// Test that multiple JWT generations produce different tokens.
    #[tokio::test]
    async fn test_multiple_jwt_generations_are_unique() {
        let app_id = GitHubAppId::new(222);
        let private_key = test_private_key();
        let generator = RS256JwtGenerator::new(private_key);

        let jwt1 = generator.generate_jwt(app_id).await.unwrap();

        // Small delay to ensure different iat timestamp
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let jwt2 = generator.generate_jwt(app_id).await.unwrap();

        // Tokens should be different (different iat timestamps)
        assert_ne!(
            jwt1.token(),
            jwt2.token(),
            "Successive JWT generations should produce different tokens"
        );
    }

    /// Test JWT generation with different app IDs.
    #[tokio::test]
    async fn test_jwt_generation_with_different_app_ids() {
        let private_key = test_private_key();
        let generator = RS256JwtGenerator::new(private_key);

        let app_id_1 = GitHubAppId::new(111);
        let app_id_2 = GitHubAppId::new(222);

        let jwt1 = generator.generate_jwt(app_id_1).await.unwrap();
        let jwt2 = generator.generate_jwt(app_id_2).await.unwrap();

        // Verify correct app IDs
        assert_eq!(jwt1.app_id(), app_id_1);
        assert_eq!(jwt2.app_id(), app_id_2);

        // Tokens should be different
        assert_ne!(jwt1.token(), jwt2.token());
    }
}

mod private_key_tests {
    use super::*;

    /// Assertion 2: JWT Token with Invalid Private Key
    ///
    /// Given: A GitHub App ID and malformed private key
    /// When: JWT generation is attempted
    /// Then: Operation returns Err(AuthenticationError::InvalidPrivateKey)
    /// And: No token is generated
    /// And: Error message does not expose private key content
    #[tokio::test]
    async fn test_generate_jwt_with_invalid_private_key() {
        let result = PrivateKey::from_pem(TEST_PRIVATE_KEY_INVALID);

        assert!(
            result.is_err(),
            "Invalid private key should be rejected during parsing"
        );

        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);

        // Error message should not contain key material
        assert!(
            !err_msg.contains("INVALID KEY DATA"),
            "Error message should not expose key content"
        );
    }

    /// Test valid PEM key loading.
    #[test]
    fn test_private_key_from_valid_pem() {
        let result = PrivateKey::from_pem(TEST_PRIVATE_KEY_PEM);

        assert!(result.is_ok(), "Valid PEM key should load successfully");
    }

    /// Test PEM key with wrong format.
    #[test]
    fn test_private_key_from_malformed_pem() {
        let malformed_pem = "NOT A VALID PEM KEY";
        let result = PrivateKey::from_pem(malformed_pem);

        assert!(result.is_err(), "Malformed PEM should be rejected");
    }

    /// Test empty PEM key.
    #[test]
    fn test_private_key_from_empty_pem() {
        let empty_pem = "";
        let result = PrivateKey::from_pem(empty_pem);

        assert!(result.is_err(), "Empty PEM should be rejected");
    }

    /// Test PEM key with extra whitespace.
    #[test]
    fn test_private_key_from_pem_with_whitespace() {
        let pem_with_whitespace = format!("\n\n{}\n\n", TEST_PRIVATE_KEY_PEM);
        let result = PrivateKey::from_pem(&pem_with_whitespace);

        // Should handle extra whitespace gracefully
        assert!(result.is_ok(), "PEM parser should handle extra whitespace");
    }

    /// Verify that PrivateKey doesn't expose key data in Debug output.
    #[test]
    fn test_private_key_debug_redaction() {
        let key = test_private_key();
        let debug_output = format!("{:?}", key);

        // Debug output should not contain actual key data
        assert!(
            !debug_output.contains("MIIEpAIBAAKCAQEA"),
            "Debug output should not expose key material"
        );
        assert!(
            debug_output.contains("<REDACTED>") || debug_output.contains("PrivateKey"),
            "Debug output should indicate redaction or type"
        );
    }
}

mod jwt_claims_tests {
    use super::*;

    /// Test JWT claims structure.
    #[test]
    fn test_jwt_claims_construction() {
        let app_id = GitHubAppId::new(12345);
        let now = Utc::now();
        let iat = now.timestamp();
        let exp = (now + Duration::minutes(10)).timestamp();

        let claims = JwtClaims {
            iss: app_id,
            iat,
            exp,
        };

        assert_eq!(claims.iss, app_id);
        assert_eq!(claims.iat, iat);
        assert_eq!(claims.exp, exp);
    }

    /// Test JWT claims serialization.
    #[test]
    fn test_jwt_claims_serialization() {
        let app_id = GitHubAppId::new(67890);
        let now = Utc::now();
        let claims = JwtClaims {
            iss: app_id,
            iat: now.timestamp(),
            exp: (now + Duration::minutes(10)).timestamp(),
        };

        let json = serde_json::to_string(&claims);
        assert!(json.is_ok(), "JWT claims should serialize to JSON");

        let json_str = json.unwrap();
        assert!(json_str.contains("\"iss\""));
        assert!(json_str.contains("\"iat\""));
        assert!(json_str.contains("\"exp\""));
    }
}

mod expiration_tests {
    use super::*;

    /// Test JWT expires_soon detection.
    #[tokio::test]
    async fn test_jwt_expires_soon_detection() {
        let app_id = GitHubAppId::new(333);
        let private_key = test_private_key();

        // Create generator with short expiration for testing
        let generator = RS256JwtGenerator::with_expiration(private_key, Duration::seconds(30));

        let jwt = generator.generate_jwt(app_id).await.unwrap();

        // Should not expire soon with 1 minute margin
        assert!(
            !jwt.expires_soon(Duration::minutes(1)),
            "JWT should not expire soon with large margin"
        );

        // Should expire soon with 1 second margin
        assert!(
            jwt.expires_soon(Duration::seconds(1)),
            "JWT should expire soon with small margin"
        );
    }

    /// Test time_until_expiry is accurate.
    #[tokio::test]
    async fn test_jwt_time_until_expiry() {
        let app_id = GitHubAppId::new(444);
        let private_key = test_private_key();
        let duration = Duration::minutes(5);
        let generator = RS256JwtGenerator::with_expiration(private_key, duration);

        let jwt = generator.generate_jwt(app_id).await.unwrap();
        let time_remaining = jwt.time_until_expiry();

        // Should be approximately 5 minutes (with tolerance for execution time)
        assert!(
            time_remaining <= Duration::minutes(5),
            "Time until expiry should not exceed configured duration"
        );
        assert!(
            time_remaining >= Duration::minutes(4) + Duration::seconds(55),
            "Time until expiry should be close to configured duration"
        );
    }
}

mod trait_implementation_tests {
    use super::*;

    /// Test that JwtGenerator trait is object-safe (can be used as dyn trait).
    #[tokio::test]
    async fn test_jwt_generator_trait_object() {
        let app_id = GitHubAppId::new(555);
        let private_key = test_private_key();
        let generator = RS256JwtGenerator::new(private_key);

        // Use as trait object
        let trait_obj: &dyn JwtGenerator = &generator;
        let jwt = trait_obj.generate_jwt(app_id).await.unwrap();

        assert_eq!(jwt.app_id(), app_id);
    }

    /// Test expiration_duration accessor.
    #[test]
    fn test_jwt_generator_expiration_duration() {
        let private_key = test_private_key();
        let generator = RS256JwtGenerator::new(private_key);

        assert_eq!(
            generator.expiration_duration(),
            Duration::minutes(10),
            "Default expiration should be 10 minutes"
        );
    }

    /// Test custom expiration_duration accessor.
    #[test]
    fn test_jwt_generator_custom_expiration_duration() {
        let private_key = test_private_key();
        let custom_duration = Duration::minutes(7);
        let generator = RS256JwtGenerator::with_expiration(private_key, custom_duration);

        assert_eq!(
            generator.expiration_duration(),
            custom_duration,
            "Custom expiration should be accessible"
        );
    }
}
