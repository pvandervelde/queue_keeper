//! Tests for the queue-keeper-cli library module.

use super::*;

#[test]
fn test_cli_parsing() {
    // Test basic command parsing
    let cli = Cli::try_parse_from(&["queue-keeper", "status", "--verbose"]);
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
