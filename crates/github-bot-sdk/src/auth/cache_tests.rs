//! Tests for token cache implementation.

use super::*;
use crate::auth::{InstallationPermissions, RepositoryId};
use chrono::{Duration, Utc};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_jwt(app_id: u64) -> JsonWebToken {
    JsonWebToken::new(
        format!("test.jwt.{}", app_id),
        GitHubAppId::new(app_id),
        Utc::now() + Duration::minutes(10),
    )
}

fn create_test_installation_token(installation_id: u64) -> InstallationToken {
    InstallationToken::new(
        format!("ghs_test_{}", installation_id),
        InstallationId::new(installation_id),
        Utc::now() + Duration::hours(1),
        InstallationPermissions::default(),
        vec![RepositoryId::new(1)],
    )
}

// ============================================================================
// InMemoryTokenCache Construction Tests
// ============================================================================

mod construction_tests {
    use super::*;

    /// Verify InMemoryTokenCache can be constructed.
    #[test]
    fn test_create_cache() {
        let cache = InMemoryTokenCache::new();
        // Shouldn't panic
        drop(cache);
    }

    /// Verify default() works.
    #[test]
    fn test_default_cache() {
        let cache = InMemoryTokenCache::default();
        drop(cache);
    }
}

// ============================================================================
// JWT Caching Tests
// ============================================================================

mod jwt_cache_tests {
    use super::*;

    /// Verify JWT can be stored and retrieved.
    #[tokio::test]
    async fn test_store_and_get_jwt() {
        let cache = InMemoryTokenCache::new();
        let jwt = create_test_jwt(12345);
        let app_id = jwt.app_id();

        cache.store_jwt(jwt.clone()).await.expect("Should store");

        let retrieved = cache
            .get_jwt(app_id)
            .await
            .expect("Should get")
            .expect("Should exist");

        assert_eq!(retrieved.token(), jwt.token());
        assert_eq!(retrieved.app_id(), app_id);
    }

    /// Verify JWT retrieval returns None for non-existent app.
    #[tokio::test]
    async fn test_get_jwt_not_found() {
        let cache = InMemoryTokenCache::new();

        let result = cache
            .get_jwt(GitHubAppId::new(99999))
            .await
            .expect("Should not error");

        assert!(result.is_none());
    }

    /// Verify JWT is replaced when storing for same app ID.
    #[tokio::test]
    async fn test_jwt_replacement() {
        let cache = InMemoryTokenCache::new();
        let app_id = GitHubAppId::new(12345);

        let jwt1 = create_test_jwt(app_id.as_u64());
        cache.store_jwt(jwt1).await.expect("Store first");

        let jwt2 = JsonWebToken::new(
            "new.jwt.token".to_string(),
            app_id,
            Utc::now() + Duration::minutes(10),
        );
        cache.store_jwt(jwt2.clone()).await.expect("Store second");

        let retrieved = cache
            .get_jwt(app_id)
            .await
            .expect("Should get")
            .expect("Should exist");

        assert_eq!(retrieved.token(), jwt2.token());
    }

    /// Verify expired JWT detection.
    #[tokio::test]
    async fn test_expired_jwt_handling() {
        let cache = InMemoryTokenCache::new();
        let app_id = GitHubAppId::new(12345);

        // Create expired JWT
        let expired_jwt = JsonWebToken::new(
            "expired.jwt".to_string(),
            app_id,
            Utc::now() - Duration::minutes(1), // Already expired
        );

        cache
            .store_jwt(expired_jwt.clone())
            .await
            .expect("Should store");

        let retrieved = cache
            .get_jwt(app_id)
            .await
            .expect("Should get")
            .expect("Should exist");

        // Cache returns it, but caller should check expiration
        assert!(retrieved.is_expired());
    }
}

// ============================================================================
// Installation Token Caching Tests
// ============================================================================

mod installation_token_cache_tests {
    use super::*;

    /// Verify installation token can be stored and retrieved.
    #[tokio::test]
    async fn test_store_and_get_installation_token() {
        let cache = InMemoryTokenCache::new();
        let token = create_test_installation_token(54321);
        let installation_id = token.installation_id();

        cache
            .store_installation_token(token.clone())
            .await
            .expect("Should store");

        let retrieved = cache
            .get_installation_token(installation_id)
            .await
            .expect("Should get")
            .expect("Should exist");

        assert_eq!(retrieved.token(), token.token());
        assert_eq!(retrieved.installation_id(), installation_id);
    }

    /// Verify installation token retrieval returns None for non-existent installation.
    #[tokio::test]
    async fn test_get_installation_token_not_found() {
        let cache = InMemoryTokenCache::new();

        let result = cache
            .get_installation_token(InstallationId::new(99999))
            .await
            .expect("Should not error");

        assert!(result.is_none());
    }

    /// Verify installation token is replaced when storing for same installation.
    #[tokio::test]
    async fn test_installation_token_replacement() {
        let cache = InMemoryTokenCache::new();
        let installation_id = InstallationId::new(54321);

        let token1 = create_test_installation_token(installation_id.as_u64());
        cache
            .store_installation_token(token1)
            .await
            .expect("Store first");

        let token2 = InstallationToken::new(
            "new_token".to_string(),
            installation_id,
            Utc::now() + Duration::hours(1),
            InstallationPermissions::default(),
            vec![],
        );
        cache
            .store_installation_token(token2.clone())
            .await
            .expect("Store second");

        let retrieved = cache
            .get_installation_token(installation_id)
            .await
            .expect("Should get")
            .expect("Should exist");

        assert_eq!(retrieved.token(), token2.token());
    }

    /// Verify expired installation token detection.
    #[tokio::test]
    async fn test_expired_installation_token_handling() {
        let cache = InMemoryTokenCache::new();
        let installation_id = InstallationId::new(54321);

        // Create expired token
        let expired_token = InstallationToken::new(
            "expired_token".to_string(),
            installation_id,
            Utc::now() - Duration::minutes(1), // Already expired
            InstallationPermissions::default(),
            vec![],
        );

        cache
            .store_installation_token(expired_token)
            .await
            .expect("Should store");

        let retrieved = cache
            .get_installation_token(installation_id)
            .await
            .expect("Should get")
            .expect("Should exist");

        // Cache returns it, but caller should check expiration
        assert!(retrieved.is_expired());
    }
}

// ============================================================================
// Token Invalidation Tests
// ============================================================================

mod invalidation_tests {
    use super::*;

    /// Verify installation token can be invalidated.
    #[tokio::test]
    async fn test_invalidate_installation_token() {
        let cache = InMemoryTokenCache::new();
        let token = create_test_installation_token(54321);
        let installation_id = token.installation_id();

        cache
            .store_installation_token(token)
            .await
            .expect("Should store");

        cache
            .invalidate_installation_token(installation_id)
            .await
            .expect("Should invalidate");

        let result = cache
            .get_installation_token(installation_id)
            .await
            .expect("Should not error");

        assert!(result.is_none());
    }

    /// Verify invalidating non-existent token doesn't error.
    #[tokio::test]
    async fn test_invalidate_nonexistent_token() {
        let cache = InMemoryTokenCache::new();

        let result = cache
            .invalidate_installation_token(InstallationId::new(99999))
            .await;

        assert!(result.is_ok());
    }
}

// ============================================================================
// Cleanup Tests
// ============================================================================

mod cleanup_tests {
    use super::*;

    /// Verify cleanup_expired_tokens() doesn't panic.
    #[test]
    fn test_cleanup_expired_tokens() {
        let cache = InMemoryTokenCache::new();

        // Should not panic
        cache.cleanup_expired_tokens();
    }

    /// Verify cleanup handles empty cache.
    #[test]
    fn test_cleanup_empty_cache() {
        let cache = InMemoryTokenCache::new();

        cache.cleanup_expired_tokens();

        // Should still work
        cache.cleanup_expired_tokens();
    }
}

// ============================================================================
// Thread Safety Tests
// ============================================================================

mod thread_safety_tests {
    use super::*;
    use std::sync::Arc;

    /// Verify cache can be shared across threads.
    #[tokio::test]
    async fn test_concurrent_jwt_access() {
        let cache = Arc::new(InMemoryTokenCache::new());
        let mut handles = vec![];

        for i in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let handle = tokio::spawn(async move {
                let jwt = create_test_jwt(12345 + i);
                cache_clone.store_jwt(jwt).await.expect("Should store");

                let retrieved = cache_clone
                    .get_jwt(GitHubAppId::new(12345 + i))
                    .await
                    .expect("Should get");

                assert!(retrieved.is_some());
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Thread should complete");
        }
    }

    /// Verify cache can handle concurrent installation token access.
    #[tokio::test]
    async fn test_concurrent_installation_token_access() {
        let cache = Arc::new(InMemoryTokenCache::new());
        let mut handles = vec![];

        for i in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let handle = tokio::spawn(async move {
                let token = create_test_installation_token(54321 + i);
                cache_clone
                    .store_installation_token(token)
                    .await
                    .expect("Should store");

                let retrieved = cache_clone
                    .get_installation_token(InstallationId::new(54321 + i))
                    .await
                    .expect("Should get");

                assert!(retrieved.is_some());
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Thread should complete");
        }
    }
}
