use crate::GrovError;
use crate::backend::Backend;
use crate::cli::Commands;
use crate::orchestration::Orchestrator;

pub async fn dispatch<B: Backend>(
    orchestrator: Orchestrator<B>,
    command: Commands,
) -> Result<(), GrovError> {
    match command {
        Commands::Install { services } => install(orchestrator, &services).await,
        Commands::Up { services } => up(orchestrator, &services).await,
        Commands::Down { services } => down(orchestrator, services.as_deref()).await,
        Commands::Env => env(orchestrator).await,
        Commands::Status => status(orchestrator).await,
        Commands::Clean => clean(orchestrator).await,
    }
}

async fn install<B: Backend>(
    orchestrator: Orchestrator<B>,
    services: &[String],
) -> Result<(), GrovError> {
    orchestrator.install(services).await?;
    Ok(())
}

async fn up<B: Backend>(
    orchestrator: Orchestrator<B>,
    services: &[String],
) -> Result<(), GrovError> {
    orchestrator.up(services).await?;
    Ok(())
}

async fn down<B: Backend>(
    orchestrator: Orchestrator<B>,
    services: Option<&[String]>,
) -> Result<(), GrovError> {
    orchestrator.down(services).await?;
    Ok(())
}

async fn env<B: Backend>(orchestrator: Orchestrator<B>) -> Result<(), GrovError> {
    let entries = orchestrator.env().await?;
    for entry in &entries {
        println!("{}={}", entry.key, entry.value);
    }
    Ok(())
}

async fn clean<B: Backend>(orchestrator: Orchestrator<B>) -> Result<(), GrovError> {
    orchestrator.clean().await?;
    Ok(())
}

async fn status<B: Backend>(orchestrator: Orchestrator<B>) -> Result<(), GrovError> {
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
    Ok(())
}
