use queue_keeper_cli::run_cli;
use tracing::error;

#[tokio::main]
async fn main() {
    // Run CLI and handle errors
    if let Err(e) = run_cli().await {
        error!("CLI error: {}", e);

        // Exit with appropriate code based on error type
        let exit_code = match e {
            queue_keeper_cli::CliError::Configuration(_) => 1,
            queue_keeper_cli::CliError::Service(_) => 2,
            queue_keeper_cli::CliError::CommandFailed { .. } => 3,
            queue_keeper_cli::CliError::InvalidArgument { .. } => 4,
            queue_keeper_cli::CliError::Io(_) => 5,
            queue_keeper_cli::CliError::QueueKeeper(_) => 6,
        };

        std::process::exit(exit_code);
    }
}
