//! Tests for the bot configuration module.

use super::*;

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
fn test_bot_config_error_transient() {
    let error = BotConfigError::SourceUnavailable("test".to_string());
    assert!(error.is_transient());

    let error = BotConfigError::ValidationError {
        errors: vec!["test".to_string()],
    };
    assert!(!error.is_transient());
}
