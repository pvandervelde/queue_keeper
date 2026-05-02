//! Tests for the queue-keeper-cli library module.

use super::*;

#[test]
fn test_cli_parsing() {
    // Test basic command parsing
    let cli = Cli::try_parse_from(["queue-keeper", "status", "--verbose"]);
    assert!(cli.is_ok());

    let cli = cli.unwrap();
    match cli.command {
        Commands::Status { verbose, .. } => assert!(verbose),
        _ => panic!("Expected Status command"),
    }
}

#[test]
fn test_config_defaults() {
    let config = CliConfig::default();
    assert_eq!(config.default_mode, ServiceMode::Combined);
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.logging.level, "info");
}

/// Verify that all command handlers return Err(CliError::CommandFailed) rather than panicking.
#[tokio::test]
async fn test_command_handlers_return_err_not_panic() {
    let config = CliConfig::default();

    let result = execute_start_command(
        ServiceMode::Server,
        8080,
        "127.0.0.1".into(),
        false,
        &config,
    )
    .await;
    assert!(
        matches!(result, Err(CliError::CommandFailed { .. })),
        "start: {result:?}"
    );

    let result = execute_stop_command(30, false).await;
    assert!(
        matches!(result, Err(CliError::CommandFailed { .. })),
        "stop: {result:?}"
    );

    let result = execute_status_command(false, OutputFormat::Text).await;
    assert!(
        matches!(result, Err(CliError::CommandFailed { .. })),
        "status: {result:?}"
    );

    let result = execute_config_command(None, false, ConfigFormat::Yaml).await;
    assert!(
        matches!(result, Err(CliError::CommandFailed { .. })),
        "config: {result:?}"
    );

    let result = execute_monitor_command(false, None, None, false, 10).await;
    assert!(
        matches!(result, Err(CliError::CommandFailed { .. })),
        "monitor: {result:?}"
    );

    let result = execute_events_command(EventCommands::List {
        limit: 10,
        event_type: None,
        repository: None,
        session: None,
        since: None,
        format: OutputFormat::Text,
    })
    .await;
    assert!(
        matches!(result, Err(CliError::CommandFailed { .. })),
        "events: {result:?}"
    );

    let result = execute_sessions_command(SessionCommands::List {
        repository: None,
        entity_type: None,
        pending_only: false,
        format: OutputFormat::Text,
    })
    .await;
    assert!(
        matches!(result, Err(CliError::CommandFailed { .. })),
        "sessions: {result:?}"
    );

    let result = execute_health_command(HealthCommands::Check {
        verbose: false,
        timeout: 10,
        format: OutputFormat::Text,
    })
    .await;
    assert!(
        matches!(result, Err(CliError::CommandFailed { .. })),
        "health: {result:?}"
    );

    let result = execute_completions_command(clap_complete::Shell::Bash).await;
    assert!(
        matches!(result, Err(CliError::CommandFailed { .. })),
        "completions: {result:?}"
    );
}

/// Verify load_configuration returns Ok with default config when no path is given.
///
/// The stub ignores the path argument. When a real implementation lands, add a
/// second case exercising `Some(path)` to cover config-file loading.
#[tokio::test]
async fn test_load_configuration_returns_default_config() {
    let result = load_configuration(None).await;
    assert!(
        result.is_ok(),
        "expected Ok from stub load_configuration: {result:?}"
    );
}

/// Verify initialize_logging returns Ok(()) (no-op stub).
#[test]
fn test_initialize_logging_returns_ok() {
    let cli = Cli::try_parse_from(["queue-keeper", "status"]).unwrap();
    let result = initialize_logging(&cli);
    assert!(result.is_ok(), "initialize_logging should be a no-op stub");
}
