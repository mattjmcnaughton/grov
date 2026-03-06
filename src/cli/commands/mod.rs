use std::path::Path;

use tracing::{info, warn};

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
        Commands::Clean {
            all,
            orphans,
            dry_run,
        } => {
            if all {
                clean_all(orchestrator, dry_run).await
            } else if orphans {
                clean_orphans(orchestrator, dry_run).await
            } else {
                clean(orchestrator, dry_run).await
            }
        }
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

async fn clean<B: Backend>(orchestrator: Orchestrator<B>, dry_run: bool) -> Result<(), GrovError> {
    if dry_run {
        eprintln!(
            "would clean grove at {}",
            orchestrator.store_path().display()
        );
        return Ok(());
    }
    orchestrator.clean().await?;
    Ok(())
}

async fn clean_all<B: Backend>(
    orchestrator: Orchestrator<B>,
    dry_run: bool,
) -> Result<(), GrovError> {
    let groves = crate::storage::list_all_groves()?;
    if groves.is_empty() {
        info!("no groves found");
        return Ok(());
    }
    for grove in &groves {
        let grove_id = &grove.state.grove_id;
        let worktree = &grove.state.worktree_path;
        if dry_run {
            eprintln!(
                "would clean grove {grove_id} (worktree: {worktree}, path: {})",
                grove.store_path.display()
            );
            continue;
        }
        info!(grove_id = grove_id.as_str(), "cleaning grove");
        crate::orchestration::stop_grove_services(orchestrator.backend(), &grove.state).await;
        if let Err(e) = std::fs::remove_dir_all(&grove.store_path) {
            warn!(grove_id = grove_id.as_str(), error = %e, "failed to remove grove directory");
        }
    }
    Ok(())
}

async fn clean_orphans<B: Backend>(
    orchestrator: Orchestrator<B>,
    dry_run: bool,
) -> Result<(), GrovError> {
    let groves = crate::storage::list_all_groves()?;
    if groves.is_empty() {
        info!("no groves found");
        return Ok(());
    }
    let mut cleaned = 0;
    for grove in &groves {
        let grove_id = &grove.state.grove_id;
        let worktree = &grove.state.worktree_path;
        let is_orphan = worktree.is_empty() || !Path::new(worktree).exists();
        if !is_orphan {
            continue;
        }
        if dry_run {
            eprintln!(
                "would clean orphaned grove {grove_id} (worktree: {worktree}, path: {})",
                grove.store_path.display()
            );
            cleaned += 1;
            continue;
        }
        info!(
            grove_id = grove_id.as_str(),
            worktree = worktree.as_str(),
            "cleaning orphaned grove"
        );
        crate::orchestration::stop_grove_services(orchestrator.backend(), &grove.state).await;
        if let Err(e) = std::fs::remove_dir_all(&grove.store_path) {
            warn!(grove_id = grove_id.as_str(), error = %e, "failed to remove grove directory");
        }
        cleaned += 1;
    }
    if dry_run {
        eprintln!("would clean {cleaned} orphaned grove(s)");
    } else {
        info!(count = cleaned, "cleaned orphaned groves");
    }
    Ok(())
}

async fn status<B: Backend>(orchestrator: Orchestrator<B>) -> Result<(), GrovError> {
    println!("Grove: {}", orchestrator.store_path().display());
    println!();
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
