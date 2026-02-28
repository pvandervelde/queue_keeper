//! Tests for the bot configuration module.

use super::*;
use crate::{webhook::WrappedEvent, Repository, RepositoryId, User, UserId, UserType};

// ============================================================================
// Basic Type Tests
// ============================================================================

#[test]
fn test_bot_specific_config_new() {
    let config = BotSpecificConfig::new();
    assert!(config.is_empty());
}

#[test]
fn test_bot_specific_config_with_setting() {
    let config = BotSpecificConfig::new().with_setting(
        "key".to_string(),
        serde_json::Value::String("value".to_string()),
    );

    assert!(!config.is_empty());
    assert_eq!(
        config.get("key"),
        Some(&serde_json::Value::String("value".to_string()))
    );
}

#[test]
fn test_bot_configuration_settings_default() {
    let settings = BotConfigurationSettings::default();
    assert_eq!(settings.max_bots, 50);
    assert_eq!(settings.default_message_ttl, 24 * 60 * 60);
    assert!(settings.validate_on_startup);
    assert!(settings.log_configuration);
}

#[test]
fn test_queue_destination() {
    let bot_name = BotName::new("test-bot").unwrap();
    let queue_name = QueueName::new("queue-keeper-test-bot").unwrap();
    let config = BotSpecificConfig::new();

    let destination = QueueDestination::new(bot_name, queue_name, true, config);
    assert!(destination.requires_ordering());
}

#[test]
fn test_routing_decision() {
    let event_id = EventId::new();
    let decision = RoutingDecision::new(event_id, vec![]);

    assert!(!decision.has_destinations());
    assert_eq!(decision.get_ordered_destinations().len(), 0);
    assert_eq!(decision.get_parallel_destinations().len(), 0);
}

#[test]
fn test_routing_decision_with_destinations() {
    let event_id = EventId::new();
    let bot_name = BotName::new("test-bot").unwrap();
    let queue_name = QueueName::new("queue-keeper-test-bot").unwrap();
    let config = BotSpecificConfig::new();

    let ordered_dest =
        QueueDestination::new(bot_name.clone(), queue_name.clone(), true, config.clone());
    let parallel_dest = QueueDestination::new(bot_name, queue_name, false, config);

    let decision = RoutingDecision::new(event_id, vec![ordered_dest, parallel_dest]);

    assert!(decision.has_destinations());
    assert_eq!(decision.get_ordered_destinations().len(), 1);
    assert_eq!(decision.get_parallel_destinations().len(), 1);
}

#[test]
fn test_bot_config_error_transient() {
    let error = BotConfigError::SourceUnavailable("test".to_string());
    assert!(error.is_transient());

    let error = BotConfigError::ValidationError {
        errors: vec!["test".to_string()],
    };
    assert!(!error.is_transient());
}

// ============================================================================
// EventTypePattern Tests
// ============================================================================

mod event_type_pattern_tests {
    use super::*;

    #[test]
    fn test_exact_pattern_parsing() {
        let pattern = EventTypePattern::from_str("issues.opened").unwrap();
        match pattern {
            EventTypePattern::Exact(s) => assert_eq!(s, "issues.opened"),
            _ => panic!("Expected Exact pattern"),
        }
    }

    #[test]
    fn test_wildcard_pattern_parsing() {
        let pattern = EventTypePattern::from_str("issues.*").unwrap();
        match pattern {
            EventTypePattern::Wildcard(s) => assert_eq!(s, "issues.*"),
            _ => panic!("Expected Wildcard pattern"),
        }
    }

    #[test]
    fn test_entity_all_pattern_parsing() {
        let pattern = EventTypePattern::from_str("pull_request").unwrap();
        match pattern {
            EventTypePattern::EntityAll(s) => assert_eq!(s, "pull_request"),
            _ => panic!("Expected EntityAll pattern"),
        }
    }

    #[test]
    fn test_exclude_pattern_parsing() {
        let pattern = EventTypePattern::from_str("!push").unwrap();
        match pattern {
            EventTypePattern::Exclude(s) => assert_eq!(s, "push"),
            _ => panic!("Expected Exclude pattern"),
        }
    }

    #[test]
    fn test_exact_pattern_matches_exact_event() {
        let pattern = EventTypePattern::Exact("issues.opened".to_string());
        assert!(pattern.matches("issues.opened"));
        assert!(!pattern.matches("issues.closed"));
        assert!(!pattern.matches("pull_request.opened"));
    }

    #[test]
    fn test_wildcard_pattern_matches_prefix() {
        let pattern = EventTypePattern::Wildcard("issues.*".to_string());
        assert!(pattern.matches("issues.opened"));
        assert!(pattern.matches("issues.closed"));
        assert!(pattern.matches("issues.labeled"));
        assert!(!pattern.matches("pull_request.opened"));
    }

    #[test]
    fn test_wildcard_pattern_matches_suffix() {
        let pattern = EventTypePattern::Wildcard("*.opened".to_string());
        assert!(pattern.matches("issues.opened"));
        assert!(pattern.matches("pull_request.opened"));
        assert!(!pattern.matches("issues.closed"));
    }

    #[test]
    fn test_entity_all_pattern_matches() {
        let pattern = EventTypePattern::EntityAll("pull_request".to_string());
        assert!(pattern.matches("pull_request.opened"));
        assert!(pattern.matches("pull_request.closed"));
        assert!(pattern.matches("pull_request.synchronize"));
        assert!(pattern.matches("pull_request"));
        assert!(!pattern.matches("issues.opened"));
    }

    #[test]
    fn test_pattern_get_entity_type() {
        let pattern = EventTypePattern::EntityAll("pull_request".to_string());
        assert_eq!(pattern.get_entity_type(), Some("pull_request"));

        let pattern = EventTypePattern::Wildcard("issues.*".to_string());
        assert_eq!(pattern.get_entity_type(), Some("issues"));

        let pattern = EventTypePattern::Exact("push".to_string());
        assert_eq!(pattern.get_entity_type(), None);
    }
}

// ============================================================================
// RepositoryFilter Tests
// ============================================================================

mod repository_filter_tests {
    use super::*;

    fn create_test_repository(owner: &str, name: &str) -> Repository {
        Repository::new(
            RepositoryId::new(12345),
            name.to_string(),
            format!("{}/{}", owner, name),
            User {
                id: UserId::new(1),
                login: owner.to_string(),
                user_type: UserType::User,
            },
            false,
        )
    }

    #[test]
    fn test_exact_filter_matches_specific_repo() {
        let filter = RepositoryFilter::Exact {
            owner: "owner".to_string(),
            name: "repo".to_string(),
        };

        let repo = create_test_repository("owner", "repo");
        assert!(filter.matches(&repo));

        let other_repo = create_test_repository("owner", "other");
        assert!(!filter.matches(&other_repo));

        let other_owner = create_test_repository("other", "repo");
        assert!(!filter.matches(&other_owner));
    }

    #[test]
    fn test_owner_filter_matches_owner_repos() {
        let filter = RepositoryFilter::Owner("owner".to_string());

        let repo1 = create_test_repository("owner", "repo1");
        let repo2 = create_test_repository("owner", "repo2");
        let other = create_test_repository("other", "repo");

        assert!(filter.matches(&repo1));
        assert!(filter.matches(&repo2));
        assert!(!filter.matches(&other));
    }

    #[test]
    fn test_name_pattern_filter_with_regex() {
        let filter = RepositoryFilter::NamePattern(".*-specs$".to_string());
        assert!(filter.validate().is_ok());

        let specs_repo = create_test_repository("owner", "project-specs");
        let regular_repo = create_test_repository("owner", "project");

        assert!(filter.matches(&specs_repo));
        assert!(!filter.matches(&regular_repo));
    }

    #[test]
    fn test_any_of_filter_or_logic() {
        let filter = RepositoryFilter::AnyOf(vec![
            RepositoryFilter::Owner("owner1".to_string()),
            RepositoryFilter::Exact {
                owner: "owner2".to_string(),
                name: "special".to_string(),
            },
        ]);

        let repo1 = create_test_repository("owner1", "any-repo");
        let repo2 = create_test_repository("owner2", "special");
        let repo3 = create_test_repository("owner2", "other");

        assert!(filter.matches(&repo1));
        assert!(filter.matches(&repo2));
        assert!(!filter.matches(&repo3));
    }

    #[test]
    fn test_all_of_filter_and_logic() {
        let filter = RepositoryFilter::AllOf(vec![
            RepositoryFilter::Owner("owner".to_string()),
            RepositoryFilter::NamePattern(".*-test$".to_string()),
        ]);

        let matching = create_test_repository("owner", "project-test");
        let wrong_owner = create_test_repository("other", "project-test");
        let wrong_name = create_test_repository("owner", "project");

        assert!(filter.matches(&matching));
        assert!(!filter.matches(&wrong_owner));
        assert!(!filter.matches(&wrong_name));
    }

    #[test]
    fn test_filter_validation_invalid_regex() {
        let filter = RepositoryFilter::NamePattern("[invalid".to_string());
        assert!(filter.validate().is_err());
    }

    #[test]
    fn test_filter_validation_valid_regex() {
        let filter = RepositoryFilter::NamePattern("^test-.*$".to_string());
        assert!(filter.validate().is_ok());
    }
}

// ============================================================================
// BotConfiguration Tests
// ============================================================================

mod bot_configuration_tests {
    use super::*;

    fn create_test_configuration() -> BotConfiguration {
        BotConfiguration {
            bots: vec![
                BotSubscription {
                    name: BotName::new("bot1").unwrap(),
                    queue: QueueName::new("queue-keeper-bot1").unwrap(),
                    events: vec![EventTypePattern::Exact("issues.opened".to_string())],
                    ordered: true,
                    repository_filter: None,
                    config: BotSpecificConfig::new(),
                },
                BotSubscription {
                    name: BotName::new("bot2").unwrap(),
                    queue: QueueName::new("queue-keeper-bot2").unwrap(),
                    events: vec![EventTypePattern::Wildcard("pull_request.*".to_string())],
                    ordered: false,
                    repository_filter: Some(RepositoryFilter::Owner("test-org".to_string())),
                    config: BotSpecificConfig::new(),
                },
            ],
            settings: BotConfigurationSettings::default(),
        }
    }

    fn create_test_event(event_type: &str, owner: &str, repo: &str) -> WrappedEvent {
        WrappedEvent::new(
            "github".to_string(),
            event_type.to_string(),
            Some("opened".to_string()),
            None,
            serde_json::json!({
                "action": "opened",
                "repository": {
                    "id": 12345,
                    "name": repo,
                    "full_name": format!("{}/{}", owner, repo),
                    "private": false,
                    "owner": {"id": 1, "login": owner, "type": "User"}
                }
            }),
        )
    }

    #[test]
    fn test_validation_duplicate_bot_names() {
        let mut config = create_test_configuration();
        config.bots.push(BotSubscription {
            name: BotName::new("bot1").unwrap(), // Duplicate
            queue: QueueName::new("queue-keeper-bot3").unwrap(),
            events: vec![EventTypePattern::Exact("push".to_string())],
            ordered: false,
            repository_filter: None,
            config: BotSpecificConfig::new(),
        });

        let result = config.validate();
        assert!(result.is_err());
        match result {
            Err(BotConfigError::ValidationError { errors }) => {
                assert!(errors
                    .iter()
                    .any(|e| e.contains("duplicate") || e.contains("Duplicate")));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    fn test_validation_max_bots_exceeded() {
        let mut config = create_test_configuration();
        config.settings.max_bots = 1;

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_success() {
        let config = create_test_configuration();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_get_target_bots_single_match() {
        let config = create_test_configuration();
        let event = create_test_event("issues.opened", "any-org", "any-repo");

        let targets = config.get_target_bots(&event);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name.as_str(), "bot1");
    }

    #[test]
    fn test_get_target_bots_multiple_match() {
        let mut config = create_test_configuration();
        config.bots.push(BotSubscription {
            name: BotName::new("bot3").unwrap(),
            queue: QueueName::new("queue-keeper-bot3").unwrap(),
            events: vec![EventTypePattern::Wildcard("issues.*".to_string())],
            ordered: false,
            repository_filter: None,
            config: BotSpecificConfig::new(),
        });

        let event = create_test_event("issues.opened", "any-org", "any-repo");
        let targets = config.get_target_bots(&event);
        assert_eq!(targets.len(), 2);
    }

    #[test]
    fn test_get_target_bots_no_match() {
        let config = create_test_configuration();
        let event = create_test_event("push", "any-org", "any-repo");

        let targets = config.get_target_bots(&event);
        assert_eq!(targets.len(), 0);
    }

    #[test]
    fn test_get_target_bots_with_repository_filter() {
        let config = create_test_configuration();
        let event = create_test_event("pull_request.opened", "test-org", "repo");

        let targets = config.get_target_bots(&event);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name.as_str(), "bot2");
    }

    #[test]
    fn test_get_target_bots_repository_filter_mismatch() {
        let config = create_test_configuration();
        let event = create_test_event("pull_request.opened", "other-org", "repo");

        let targets = config.get_target_bots(&event);
        assert_eq!(targets.len(), 0);
    }
}

// ============================================================================
// BotSubscription Tests
// ============================================================================

mod bot_subscription_tests {
    use super::*;

    fn create_test_event(event_type: &str, owner: &str, repo: &str) -> WrappedEvent {
        WrappedEvent::new(
            "github".to_string(),
            event_type.to_string(),
            Some("opened".to_string()),
            None,
            serde_json::json!({
                "action": "opened",
                "repository": {
                    "id": 12345,
                    "name": repo,
                    "full_name": format!("{}/{}", owner, repo),
                    "private": false,
                    "owner": {"id": 1, "login": owner, "type": "User"}
                }
            }),
        )
    }

    #[test]
    fn test_subscription_matches_event_type() {
        let subscription = BotSubscription {
            name: BotName::new("test-bot").unwrap(),
            queue: QueueName::new("queue-keeper-test-bot").unwrap(),
            events: vec![EventTypePattern::Exact("issues.opened".to_string())],
            ordered: true,
            repository_filter: None,
            config: BotSpecificConfig::new(),
        };

        let event = create_test_event("issues.opened", "owner", "repo");
        assert!(subscription.matches_event(&event));
    }

    #[test]
    fn test_subscription_rejects_event_type() {
        let subscription = BotSubscription {
            name: BotName::new("test-bot").unwrap(),
            queue: QueueName::new("queue-keeper-test-bot").unwrap(),
            events: vec![EventTypePattern::Exact("issues.opened".to_string())],
            ordered: true,
            repository_filter: None,
            config: BotSpecificConfig::new(),
        };

        let event = create_test_event("issues.closed", "owner", "repo");
        assert!(!subscription.matches_event(&event));
    }

    #[test]
    fn test_subscription_with_repository_filter() {
        let subscription = BotSubscription {
            name: BotName::new("test-bot").unwrap(),
            queue: QueueName::new("queue-keeper-test-bot").unwrap(),
            events: vec![EventTypePattern::Wildcard("issues.*".to_string())],
            ordered: true,
            repository_filter: Some(RepositoryFilter::Owner("specific-owner".to_string())),
            config: BotSpecificConfig::new(),
        };

        let matching = create_test_event("issues.opened", "specific-owner", "repo");
        assert!(subscription.matches_event(&matching));

        let non_matching = create_test_event("issues.opened", "other-owner", "repo");
        assert!(!subscription.matches_event(&non_matching));
    }

    #[test]
    fn test_subscription_ordering_requirements() {
        let ordered = BotSubscription {
            name: BotName::new("ordered-bot").unwrap(),
            queue: QueueName::new("queue-keeper-ordered-bot").unwrap(),
            events: vec![],
            ordered: true,
            repository_filter: None,
            config: BotSpecificConfig::new(),
        };

        let parallel = BotSubscription {
            name: BotName::new("parallel-bot").unwrap(),
            queue: QueueName::new("queue-keeper-parallel-bot").unwrap(),
            events: vec![],
            ordered: false,
            repository_filter: None,
            config: BotSpecificConfig::new(),
        };

        assert!(ordered.requires_ordering());
        assert!(!parallel.requires_ordering());
    }
}

// ============================================================================
// EventMatcher Tests
// ============================================================================

mod event_matcher_tests {
    use super::*;

    fn create_test_event(event_type: &str, owner: &str, repo: &str) -> WrappedEvent {
        WrappedEvent::new(
            "github".to_string(),
            event_type.to_string(),
            Some("opened".to_string()),
            None,
            serde_json::json!({
                "action": "opened",
                "repository": {
                    "id": 12345,
                    "name": repo,
                    "full_name": format!("{}/{}", owner, repo),
                    "private": false,
                    "owner": {"id": 1, "login": owner, "type": "User"}
                }
            }),
        )
    }

    #[test]
    fn test_matches_pattern_exact() {
        let matcher = DefaultEventMatcher;
        let pattern = EventTypePattern::Exact("issues.opened".to_string());

        assert!(matcher.matches_pattern("issues.opened", &pattern));
        assert!(!matcher.matches_pattern("issues.closed", &pattern));
    }

    #[test]
    fn test_matches_pattern_wildcard() {
        let matcher = DefaultEventMatcher;
        let pattern = EventTypePattern::Wildcard("issues.*".to_string());

        assert!(matcher.matches_pattern("issues.opened", &pattern));
        assert!(matcher.matches_pattern("issues.closed", &pattern));
        assert!(!matcher.matches_pattern("pull_request.opened", &pattern));
    }

    #[test]
    fn test_matches_repository() {
        let matcher = DefaultEventMatcher;
        let filter = RepositoryFilter::Owner("test-owner".to_string());

        let repo = Repository::new(
            RepositoryId::new(12345),
            "repo".to_string(),
            "test-owner/repo".to_string(),
            User {
                id: UserId::new(1),
                login: "test-owner".to_string(),
                user_type: UserType::User,
            },
            false,
        );

        assert!(matcher.matches_repository(&repo, &filter));
    }

    #[test]
    fn test_matches_subscription_complete() {
        let matcher = DefaultEventMatcher;

        let subscription = BotSubscription {
            name: BotName::new("test-bot").unwrap(),
            queue: QueueName::new("queue-keeper-test-bot").unwrap(),
            events: vec![EventTypePattern::Wildcard("issues.*".to_string())],
            ordered: true,
            repository_filter: Some(RepositoryFilter::Owner("test-owner".to_string())),
            config: BotSpecificConfig::new(),
        };

        let matching_event = create_test_event("issues.opened", "test-owner", "repo");
        assert!(matcher.matches_subscription(&matching_event, &subscription));

        let wrong_event = create_test_event("push", "test-owner", "repo");
        assert!(!matcher.matches_subscription(&wrong_event, &subscription));

        let wrong_owner = create_test_event("issues.opened", "other-owner", "repo");
        assert!(!matcher.matches_subscription(&wrong_owner, &subscription));
    }
}

// ============================================================================
// Configuration Loading Tests (will be implemented with file I/O)
// ============================================================================

mod configuration_loading_tests {
    use super::*;

    #[test]
    fn test_load_from_env_missing() {
        // Clear environment variable to ensure it's not set
        std::env::remove_var("BOT_CONFIGURATION");

        let result = BotConfiguration::load_from_env();
        assert!(result.is_err());
        match result {
            Err(BotConfigError::SourceUnavailable(_)) => {}
            _ => panic!("Expected SourceUnavailable error"),
        }
    }
}
