pub mod backend;
pub mod cli;
pub mod orchestration;
pub mod storage;

use backend::BackendError;
use backend::health::HealthCheckError;
use storage::StorageError;

#[derive(Debug, thiserror::Error)]
pub enum GrovError {
    #[error("backend error: {0}")]
    Backend(#[from] BackendError),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("health check failed: {0}")]
    HealthCheck(#[from] HealthCheckError),
    #[error("unknown service: {name}. Available services: {available}")]
    UnknownService { name: String, available: String },
    #[error("service {name} is already running on port {port}")]
    AlreadyRunning { name: String, port: u16 },
    #[error("internal error: {0}")]
    Internal(String),
    #[error("interrupted")]
    Interrupted,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn backend_error_converts_to_grov_error() {
        let backend_err = BackendError::DockerUnavailable;
        let grov_err: GrovError = backend_err.into();
        assert!(matches!(
            grov_err,
            GrovError::Backend(BackendError::DockerUnavailable)
        ));
    }

    #[test]
    fn storage_error_converts_to_grov_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let storage_err = StorageError::Io(io_err);
        let grov_err: GrovError = storage_err.into();
        assert!(matches!(grov_err, GrovError::Storage(StorageError::Io(_))));
    }

    #[test]
    fn health_check_error_converts_to_grov_error() {
        let health_err = HealthCheckError::Timeout {
            service: "postgres".to_string(),
            port: 5432,
            elapsed: Duration::from_secs(60),
        };
        let grov_err: GrovError = health_err.into();
        assert!(matches!(grov_err, GrovError::HealthCheck(_)));
    }

    #[test]
    fn unknown_service_display_message() {
        let err = GrovError::UnknownService {
            name: "postgre".to_string(),
            available: "minio, postgres".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "unknown service: postgre. Available services: minio, postgres"
        );
    }

    #[test]
    fn already_running_display_message() {
        let err = GrovError::AlreadyRunning {
            name: "postgres".to_string(),
            port: 54321,
        };
        assert_eq!(
            err.to_string(),
            "service postgres is already running on port 54321"
        );
    }

    #[test]
    fn docker_unavailable_display_message() {
        let err = GrovError::Backend(BackendError::DockerUnavailable);
        assert_eq!(
            err.to_string(),
            "backend error: Docker daemon is not running. Start Docker and try again."
        );
    }

    #[test]
    fn internal_error_display_message() {
        let err = GrovError::Internal("something went wrong".to_string());
        assert_eq!(err.to_string(), "internal error: something went wrong");
    }

    #[test]
    fn interrupted_display_message() {
        let err = GrovError::Interrupted;
        assert_eq!(err.to_string(), "interrupted");
    }

    #[test]
    fn health_check_timeout_display_message() {
        let err = GrovError::HealthCheck(HealthCheckError::Timeout {
            service: "postgres".to_string(),
            port: 5432,
            elapsed: Duration::from_secs(60),
        });
        assert_eq!(
            err.to_string(),
            "health check failed: postgres failed to become healthy within 60 seconds"
        );
    }
}
