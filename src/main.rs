use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use clap::Parser;
use grov::GrovError;
use grov::backend::docker::DockerBackend;
use grov::backend::native::NativeBackend;
use grov::cli::Cli;
use grov::cli::Commands;
use grov::cli::commands::dispatch;
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

async fn run(command: Commands) -> Result<(), GrovError> {
    let grove_id = grove::resolve().map_err(|e| GrovError::Internal(e.to_string()))?;
    tracing::debug!(grove_id = %grove_id, "resolved grove ID");

    let state_manager = StateManager::new(&grove_id)?;

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_flag = shutdown.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            shutdown_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    });

    let backend_name = std::env::var("GROV_BACKEND").unwrap_or_else(|_| "docker".to_string());
    match backend_name.as_str() {
        "native" => {
            let orchestrator = Orchestrator::new(NativeBackend, state_manager, shutdown);
            dispatch(orchestrator, command).await
        }
        _ => {
            let backend = DockerBackend::new().await?;
            let orchestrator = Orchestrator::new(backend, state_manager, shutdown);
            dispatch(orchestrator, command).await
        }
    }
}

fn exit_code_for(err: &GrovError) -> u8 {
    match err {
        GrovError::UnknownService { .. } => 2,
        GrovError::Interrupted => 130,
        _ => 1,
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            let code = if e.use_stderr() { 2 } else { 0 };
            return ExitCode::from(code);
        }
    };
    init_tracing(cli.verbose);

    tracing::debug!("grov starting with verbosity level {}", cli.verbose);
    tracing::debug!("command: {:?}", cli.command);

    match run(cli.command).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(ref e @ GrovError::Interrupted) => ExitCode::from(exit_code_for(e)),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(exit_code_for(&e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grov::backend::BackendError;
    use grov::storage::StorageError;

    #[test]
    fn exit_code_for_unknown_service_is_2() {
        let err = GrovError::UnknownService {
            name: "foo".to_string(),
            available: String::new(),
        };
        assert_eq!(exit_code_for(&err), 2);
    }

    #[test]
    fn exit_code_for_backend_error_is_1() {
        let err = GrovError::Backend(BackendError::DockerUnavailable);
        assert_eq!(exit_code_for(&err), 1);
    }

    #[test]
    fn exit_code_for_storage_error_is_1() {
        let err = GrovError::Storage(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        )));
        assert_eq!(exit_code_for(&err), 1);
    }

    #[test]
    fn exit_code_for_internal_error_is_1() {
        let err = GrovError::Internal("boom".to_string());
        assert_eq!(exit_code_for(&err), 1);
    }

    #[test]
    fn exit_code_for_interrupted_is_130() {
        let err = GrovError::Interrupted;
        assert_eq!(exit_code_for(&err), 130);
    }
}
