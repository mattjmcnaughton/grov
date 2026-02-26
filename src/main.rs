use std::process::ExitCode;

use clap::Parser;
use grov::backend::Backend;
use grov::backend::docker::DockerBackend;
use grov::backend::native::NativeBackend;
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

async fn dispatch<B: Backend>(
    orchestrator: Orchestrator<B>,
    command: Commands,
) -> Result<(), Box<dyn std::error::Error>> {
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
            let entries = orchestrator.env().await?;
            for entry in &entries {
                println!("{}={}", entry.key, entry.value);
            }
        }
        Commands::Status => {
            let statuses = orchestrator.status().await?;
            if statuses.is_empty() {
                println!("No running services.");
            } else {
                let name_w = statuses.iter().map(|s| s.name.len()).max().unwrap().max(7);
                let backend_w = statuses
                    .iter()
                    .map(|s| s.backend.len())
                    .max()
                    .unwrap()
                    .max(7);
                let status_w = statuses
                    .iter()
                    .map(|s| s.status.to_string().len())
                    .max()
                    .unwrap()
                    .max(6);
                println!(
                    "{:<name_w$}  {:<backend_w$}  {:<status_w$}  PORT",
                    "SERVICE", "BACKEND", "STATUS"
                );
                for s in &statuses {
                    println!(
                        "{:<name_w$}  {:<backend_w$}  {:<status_w$}  {}",
                        s.name, s.backend, s.status, s.port
                    );
                }
            }
        }
    }
    Ok(())
}

async fn run(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    let grove_id = grove::resolve()?;
    tracing::debug!(grove_id = %grove_id, "resolved grove ID");

    let state_manager = StateManager::new(&grove_id)?;

    let backend_name = std::env::var("GROV_BACKEND").unwrap_or_else(|_| "docker".to_string());
    match backend_name.as_str() {
        "native" => {
            let orchestrator = Orchestrator::new(NativeBackend, state_manager);
            dispatch(orchestrator, command).await
        }
        _ => {
            let backend = DockerBackend::new().await?;
            let orchestrator = Orchestrator::new(backend, state_manager);
            dispatch(orchestrator, command).await
        }
    }
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
