use std::process::ExitCode;

use clap::Parser;
use grov::backend::docker::DockerBackend;
use grov::cli::{Cli, Commands};
use grov::orchestration::Orchestrator;
use grov::orchestration::grove;
use grov::storage::StateManager;
use tracing_subscriber::EnvFilter;

fn init_tracing(verbosity: u8) {
    let default_level = match verbosity {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();
}

async fn run(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    let grove_id = grove::resolve()?;
    tracing::debug!(grove_id = %grove_id, "resolved grove ID");

    let state_manager = StateManager::new(&grove_id)?;
    let backend = DockerBackend::new().await?;
    let orchestrator = Orchestrator::new(backend, state_manager);

    match command {
        Commands::Install { services } => {
            orchestrator.install(&services).await?;
        }
        Commands::Up { services } => {
            orchestrator.up(&services).await?;
        }
        Commands::Down { services } => {
            orchestrator.down(services.as_deref()).await?;
        }
        Commands::Env => {
            eprintln!("env command not yet implemented");
        }
        Commands::Status => {
            eprintln!("status command not yet implemented");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    tracing::debug!("grov starting with verbosity level {}", cli.verbose);
    tracing::debug!("command: {:?}", cli.command);

    match run(cli.command).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
