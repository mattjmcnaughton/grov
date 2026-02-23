pub mod docker;
pub mod health;
pub mod native;

use crate::orchestration::service::ResolvedService;
use crate::orchestration::services::Service;

#[derive(Debug, Clone)]
pub enum ServiceHandle {
    Docker { container_id: String },
    Native { pid: u32 },
}

pub trait Backend: Send + Sync {
    fn install(
        &self,
        service: &dyn Service,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn start(
        &self,
        service: &dyn Service,
        resolved: &ResolvedService,
    ) -> impl std::future::Future<Output = Result<ServiceHandle, BackendError>> + Send;

    fn stop(
        &self,
        handle: &ServiceHandle,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn is_running(
        &self,
        handle: &ServiceHandle,
    ) -> impl std::future::Future<Output = Result<bool, BackendError>> + Send;

    fn backend_type(&self) -> &'static str;
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("Docker daemon is not running. Start Docker and try again.")]
    DockerUnavailable,
    #[error("port {port} is unavailable")]
    PortUnavailable { port: u16 },
    #[error("failed to start {service}: {reason}")]
    StartFailed { service: String, reason: String },
    #[error("failed to stop {service}: {reason}")]
    StopFailed { service: String, reason: String },
    #[error("Docker error: {0}")]
    Docker(String),
    #[error("{binary} not found. Install it and ensure it is in PATH.")]
    BinaryNotFound { binary: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_unavailable_message() {
        let err = BackendError::DockerUnavailable;
        assert_eq!(
            err.to_string(),
            "Docker daemon is not running. Start Docker and try again."
        );
    }

    #[test]
    fn port_unavailable_message() {
        let err = BackendError::PortUnavailable { port: 54321 };
        assert_eq!(err.to_string(), "port 54321 is unavailable");
    }

    #[test]
    fn start_failed_message() {
        let err = BackendError::StartFailed {
            service: "postgres".to_string(),
            reason: "container exited".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to start postgres: container exited"
        );
    }

    #[test]
    fn stop_failed_message() {
        let err = BackendError::StopFailed {
            service: "minio".to_string(),
            reason: "timeout".to_string(),
        };
        assert_eq!(err.to_string(), "failed to stop minio: timeout");
    }

    #[test]
    fn docker_error_message() {
        let err = BackendError::Docker("connection refused".to_string());
        assert_eq!(err.to_string(), "Docker error: connection refused");
    }

    #[test]
    fn binary_not_found_message() {
        let err = BackendError::BinaryNotFound {
            binary: "pg_ctl".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "pg_ctl not found. Install it and ensure it is in PATH."
        );
    }
}
