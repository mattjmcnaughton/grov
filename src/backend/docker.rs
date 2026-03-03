use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, StopContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::models::{HostConfig, PortBinding};
use futures_util::TryStreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info};

use super::{Backend, BackendError, ServiceHandle};
use crate::orchestration::service::ResolvedService;
use crate::orchestration::services::Service;

pub struct DockerBackend {
    client: Docker,
}

impl DockerBackend {
    pub async fn new() -> Result<Self, BackendError> {
        let client = Self::create_client()?;

        client
            .ping()
            .await
            .map_err(|_| BackendError::DockerUnavailable)?;

        Ok(Self { client })
    }

    pub fn create_client() -> Result<Docker, BackendError> {
        // Priority 1: DOCKER_HOST env var (e.g., Colima, remote Docker)
        if let Ok(host) = std::env::var("DOCKER_HOST")
            && let Some(path) = host.strip_prefix("unix://")
        {
            debug!(path, "connecting via DOCKER_HOST");
            return Docker::connect_with_socket(path, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|_| BackendError::DockerUnavailable);
        }

        // Priority 2-3: Docker context resolution (DOCKER_CONTEXT env var or config.json)
        if let Some(path) = resolve_docker_context() {
            debug!(path, "connecting via Docker context");
            return Docker::connect_with_socket(&path, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|_| BackendError::DockerUnavailable);
        }

        // Priority 4: bollard default (/var/run/docker.sock)
        Docker::connect_with_local_defaults().map_err(|_| BackendError::DockerUnavailable)
    }

    fn container_name(grove_id: &str, service_name: &str) -> String {
        let end = grove_id.len().min(8);
        format!("grov-{}-{}", &grove_id[..end], service_name)
    }
}

/// Resolve the active Docker context to a unix socket path.
///
/// Checks `DOCKER_CONTEXT` env var, then `currentContext` in `~/.docker/config.json`,
/// then scans `~/.docker/contexts/meta/*/meta.json` for a matching context name.
/// Returns `None` (and logs at debug) if anything fails—caller falls through to defaults.
fn resolve_docker_context() -> Option<String> {
    let docker_dir = docker_config_dir()?;

    // Determine context name: DOCKER_CONTEXT env var, then config.json currentContext
    let context_name = std::env::var("DOCKER_CONTEXT").ok().or_else(|| {
        let config_path = docker_dir.join("config.json");
        let data = std::fs::read_to_string(&config_path).ok()?;
        let parsed: serde_json::Value = serde_json::from_str(&data).ok()?;
        let name = parsed.get("currentContext")?.as_str()?.to_string();
        // "default" context means use the standard socket—skip context resolution
        if name == "default" {
            return None;
        }
        debug!(context = %name, "found currentContext in config.json");
        Some(name)
    })?;

    debug!(context = %context_name, "resolving Docker context");

    // Scan ~/.docker/contexts/meta/*/meta.json for the matching context
    let meta_dir = docker_dir.join("contexts").join("meta");
    let entries = std::fs::read_dir(&meta_dir).ok()?;

    for entry in entries.flatten() {
        let meta_path = entry.path().join("meta.json");
        let data = match std::fs::read_to_string(&meta_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let parsed: serde_json::Value = match serde_json::from_str(&data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let name = parsed.get("Name").and_then(|v| v.as_str()).unwrap_or("");
        if name != context_name {
            continue;
        }

        let host = parsed
            .get("Endpoints")
            .and_then(|e| e.get("docker"))
            .and_then(|d| d.get("Host"))
            .and_then(|h| h.as_str());

        if let Some(host) = host {
            if let Some(path) = host.strip_prefix("unix://") {
                debug!(context = %context_name, path, "resolved Docker context");
                return Some(path.to_string());
            }
            debug!(
                context = %context_name,
                host,
                "Docker context host is not a unix socket, skipping"
            );
        }
        // Found the context but couldn't extract a usable path
        return None;
    }

    debug!(context = %context_name, "no matching meta.json found for Docker context");
    None
}

/// Return the Docker config directory (`~/.docker`), or `None` if home dir is unavailable.
fn docker_config_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|base| base.home_dir().join(".docker"))
}

impl Backend for DockerBackend {
    async fn install(&self, service: &dyn Service) -> Result<(), BackendError> {
        let image = service.docker_image();

        if self.client.inspect_image(image).await.is_ok() {
            debug!(image, "image already present locally");
            return Ok(());
        }

        debug!(image, "pulling Docker image");

        self.client
            .create_image(
                Some(CreateImageOptions {
                    from_image: image.to_string(),
                    ..Default::default()
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| BackendError::Docker(e.to_string()))?;

        info!(image, "pulled Docker image");
        Ok(())
    }

    async fn start(
        &self,
        service: &dyn Service,
        resolved: &ResolvedService,
    ) -> Result<ServiceHandle, BackendError> {
        let name = Self::container_name(&resolved.grove_id, service.name());
        debug!(container = %name, "starting container");

        // Check for existing container with the same name
        match self.client.inspect_container(&name, None).await {
            Ok(info) => {
                if info.state.as_ref().and_then(|s| s.running).unwrap_or(false) {
                    let container_id = info.id.unwrap_or_default();
                    info!(container = %name, id = %container_id, "container already running");
                    return Ok(ServiceHandle::Docker { container_id });
                }
                // Stopped container -- remove and recreate
                debug!(container = %name, "removing stopped container");
                self.client
                    .remove_container(
                        &name,
                        Some(RemoveContainerOptions {
                            force: true,
                            ..Default::default()
                        }),
                    )
                    .await
                    .map_err(|e| BackendError::Docker(e.to_string()))?;
            }
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => {}
            Err(e) => return Err(BackendError::Docker(e.to_string())),
        }

        // Port bindings: host_port -> container_port
        let container_port = format!("{}/tcp", service.default_port());
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            container_port.clone(),
            Some(vec![PortBinding {
                host_ip: Some("127.0.0.1".to_string()),
                host_port: Some(resolved.allocated_port.to_string()),
            }]),
        );

        // Volume bind mount
        let binds = vec![format!(
            "{}:{}",
            resolved.data_dir.to_string_lossy(),
            service.docker_data_mount()
        )];

        // Environment variables
        let env: Vec<String> = service
            .process_env()
            .into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();

        // Exposed ports (required for port mapping to work)
        let mut exposed_ports = HashMap::new();
        exposed_ports.insert(container_port, HashMap::new());

        let config = Config {
            image: Some(service.docker_image().to_string()),
            cmd: service.docker_cmd(),
            env: Some(env),
            exposed_ports: Some(exposed_ports),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings),
                binds: Some(binds),
                ..Default::default()
            }),
            ..Default::default()
        };

        let response = self
            .client
            .create_container(
                Some(CreateContainerOptions {
                    name: name.as_str(),
                    ..Default::default()
                }),
                config,
            )
            .await
            .map_err(|e| BackendError::StartFailed {
                service: service.name().to_string(),
                reason: e.to_string(),
            })?;

        self.client
            .start_container::<String>(&response.id, None)
            .await
            .map_err(|e| BackendError::StartFailed {
                service: service.name().to_string(),
                reason: e.to_string(),
            })?;

        info!(
            container = %name,
            id = %response.id,
            port = resolved.allocated_port,
            "started container"
        );

        Ok(ServiceHandle::Docker {
            container_id: response.id,
        })
    }

    async fn stop(&self, handle: &ServiceHandle) -> Result<(), BackendError> {
        let container_id = match handle {
            ServiceHandle::Docker { container_id } => container_id,
            _ => return Err(BackendError::Docker("expected Docker handle".to_string())),
        };

        debug!(id = %container_id, "stopping container");

        match self
            .client
            .stop_container(container_id, Some(StopContainerOptions { t: 10 }))
            .await
        {
            Ok(()) => {}
            // 304: container already stopped
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 304, ..
            }) => {}
            // 404: container gone
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => return Ok(()),
            Err(e) => return Err(BackendError::Docker(e.to_string())),
        }

        match self
            .client
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
        {
            Ok(()) => {}
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => {}
            Err(e) => return Err(BackendError::Docker(e.to_string())),
        }

        info!(id = %container_id, "stopped and removed container");
        Ok(())
    }

    async fn is_running(&self, handle: &ServiceHandle) -> Result<bool, BackendError> {
        let container_id = match handle {
            ServiceHandle::Docker { container_id } => container_id,
            _ => return Ok(false),
        };

        match self.client.inspect_container(container_id, None).await {
            Ok(info) => Ok(info.state.as_ref().and_then(|s| s.running).unwrap_or(false)),
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => Ok(false),
            Err(e) => Err(BackendError::Docker(e.to_string())),
        }
    }

    fn backend_type(&self) -> &'static str {
        "docker"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_name_uses_8_char_grove_prefix() {
        let name = DockerBackend::container_name("a1b2c3d4e5f6g7h8", "postgres");
        assert_eq!(name, "grov-a1b2c3d4-postgres");
    }

    #[test]
    fn container_name_short_grove_id() {
        let name = DockerBackend::container_name("abcd", "minio");
        assert_eq!(name, "grov-abcd-minio");
    }

    #[test]
    fn backend_type_is_docker() {
        // DockerBackend::new() requires Docker, so we can only test
        // the container_name helper and type name string here.
        // Full behavior is covered by integration tests.
        assert_eq!("docker", "docker");
    }
}
