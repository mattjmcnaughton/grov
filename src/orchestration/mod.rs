pub mod env_template;
pub mod grove;
pub mod port;
pub mod service;
pub mod services;

use std::collections::HashMap;

use chrono::{SecondsFormat, Utc};
use tracing::{debug, info, warn};

use crate::GrovError;
use crate::backend::{Backend, BackendError, ServiceHandle};
use crate::orchestration::service::ResolvedService;
use crate::orchestration::services::Service;
use crate::storage::StateManager;
use crate::storage::state::{ServiceHandleState, ServiceState};

// --- From impls for ServiceHandle <-> ServiceHandleState ---

impl From<ServiceHandle> for ServiceHandleState {
    fn from(handle: ServiceHandle) -> Self {
        match handle {
            ServiceHandle::Docker { container_id } => ServiceHandleState::Docker { container_id },
            ServiceHandle::Native { pid } => ServiceHandleState::Native { pid },
        }
    }
}

impl From<ServiceHandleState> for ServiceHandle {
    fn from(state: ServiceHandleState) -> Self {
        match state {
            ServiceHandleState::Docker { container_id } => ServiceHandle::Docker { container_id },
            ServiceHandleState::Native { pid } => ServiceHandle::Native { pid },
        }
    }
}

// --- Helper functions ---

fn build_template_values(service: &dyn Service, port: u16) -> HashMap<String, String> {
    let mut values = service.defaults();
    values.insert("port".to_string(), port.to_string());
    values
}

fn render_service_env(service: &dyn Service, port: u16) -> anyhow::Result<HashMap<String, String>> {
    let values = build_template_values(service, port);
    let mut rendered = HashMap::new();
    for (key, template) in service.env_template() {
        let value = env_template::render(&template, &values)?;
        rendered.insert(key, value);
    }
    Ok(rendered)
}

// --- Orchestrator ---

pub struct Orchestrator<B: Backend> {
    backend: B,
    state_manager: StateManager,
    services: Vec<Box<dyn Service>>,
}

impl<B: Backend> Orchestrator<B> {
    pub fn new(backend: B, state_manager: StateManager) -> Self {
        Self {
            backend,
            state_manager,
            services: services::builtin_services(),
        }
    }

    pub fn find_service(&self, name: &str) -> Result<&dyn Service, GrovError> {
        self.services
            .iter()
            .find(|s| s.name() == name)
            .map(|s| s.as_ref())
            .ok_or_else(|| GrovError::UnknownService {
                name: name.to_string(),
            })
    }

    pub async fn install(&self, service_names: &[String]) -> Result<(), GrovError> {
        for name in service_names {
            let service = self.find_service(name)?;
            info!(service = name, "installing service");
            self.backend.install(service).await?;
            info!(service = name, "installed service");
        }
        Ok(())
    }

    pub async fn up(&self, service_names: &[String]) -> Result<(), GrovError> {
        // Validate ALL names upfront (fail fast)
        for name in service_names {
            self.find_service(name)?;
        }

        let grove_id = self.state_manager.load_state()?.grove_id.clone();

        for name in service_names {
            let service = self.find_service(name)?;

            // Check if already running via saved state
            let state = self.state_manager.load_state()?;
            if let Some(svc_state) = state.services.get(name) {
                let handle: ServiceHandle = svc_state.handle.clone().into();
                if self.backend.is_running(&handle).await? {
                    info!(
                        service = name,
                        port = svc_state.port,
                        "service already running, skipping"
                    );
                    continue;
                }
                // Stale state — clean it up
                debug!(service = name, "cleaning stale state for dead service");
                self.state_manager.with_lock(|grove_state| {
                    grove_state.services.remove(name);
                    Ok(())
                })?;
            }

            // Allocate port
            let port = port::allocate().map_err(|e| {
                GrovError::Backend(BackendError::StartFailed {
                    service: name.to_string(),
                    reason: e.to_string(),
                })
            })?;
            debug!(service = name, port, "allocated port");

            // Ensure data directory
            let data_dir = self.state_manager.ensure_data_dir(name)?;
            debug!(service = name, ?data_dir, "ensured data directory");

            // Build template values and render env templates
            let resolved_env = render_service_env(service, port).map_err(|e| {
                GrovError::Backend(BackendError::StartFailed {
                    service: name.to_string(),
                    reason: e.to_string(),
                })
            })?;

            let resolved = ResolvedService {
                grove_id: grove_id.clone(),
                allocated_port: port,
                data_dir,
                resolved_env,
            };

            // Start the service
            info!(service = name, port, "starting service");
            let handle = self.backend.start(service, &resolved).await?;

            // Health check
            if let Err(e) =
                crate::backend::health::wait_until_healthy(name, "127.0.0.1", port, None, None)
                    .await
            {
                warn!(service = name, "health check failed, stopping service");
                let _ = self.backend.stop(&handle).await;
                return Err(e.into());
            }
            info!(service = name, port, "service is healthy");

            // Save state under lock
            let handle_state: ServiceHandleState = handle.into();
            let backend_type = self.backend.backend_type().to_string();
            let started_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
            let service_name = name.to_string();
            self.state_manager.with_lock(|grove_state| {
                let key = service_name.clone();
                grove_state.services.insert(
                    key,
                    ServiceState {
                        service_name,
                        port,
                        handle: handle_state,
                        backend_type,
                        started_at,
                    },
                );
                Ok(())
            })?;

            info!(service = name, port, "service started and state saved");
        }

        Ok(())
    }

    pub async fn down(&self, service_names: Option<&[String]>) -> Result<(), GrovError> {
        let names_to_stop: Vec<String> = match service_names {
            Some(names) => {
                // Validate names against registry
                for name in names {
                    self.find_service(name)?;
                }
                names.to_vec()
            }
            None => {
                let state = self.state_manager.load_state()?;
                state.services.keys().cloned().collect()
            }
        };

        for name in &names_to_stop {
            let state = self.state_manager.load_state()?;
            let svc_state = match state.services.get(name) {
                Some(s) => s.clone(),
                None => {
                    debug!(service = name, "service not in state, skipping");
                    continue;
                }
            };

            let handle: ServiceHandle = svc_state.handle.into();
            info!(service = name, "stopping service");
            self.backend.stop(&handle).await?;

            let svc_name = name.clone();
            self.state_manager.with_lock(|grove_state| {
                grove_state.services.remove(&svc_name);
                Ok(())
            })?;

            info!(service = name, "service stopped and state removed");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Backend, BackendError, ServiceHandle};
    use crate::orchestration::service::ResolvedService;
    use crate::orchestration::services::Service;
    use crate::storage::state::{ServiceHandleState, ServiceState};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    // --- MockBackend ---

    #[derive(Clone)]
    struct MockBackend {
        // Recording
        installed: Arc<Mutex<Vec<String>>>,
        started: Arc<Mutex<Vec<String>>>,
        stopped: Arc<Mutex<Vec<ServiceHandle>>>,
        // Configuration
        is_running_returns: Arc<Mutex<bool>>,
        bind_listener: bool,
        listeners: Arc<Mutex<Vec<TcpListener>>>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                installed: Arc::new(Mutex::new(vec![])),
                started: Arc::new(Mutex::new(vec![])),
                stopped: Arc::new(Mutex::new(vec![])),
                is_running_returns: Arc::new(Mutex::new(false)),
                bind_listener: false,
                listeners: Arc::new(Mutex::new(vec![])),
            }
        }

        fn with_listener(mut self) -> Self {
            self.bind_listener = true;
            self
        }

        fn with_is_running(self, val: bool) -> Self {
            *self.is_running_returns.lock().unwrap() = val;
            self
        }
    }

    impl Backend for MockBackend {
        async fn install(&self, service: &dyn Service) -> Result<(), BackendError> {
            self.installed
                .lock()
                .unwrap()
                .push(service.name().to_string());
            Ok(())
        }

        async fn start(
            &self,
            service: &dyn Service,
            resolved: &ResolvedService,
        ) -> Result<ServiceHandle, BackendError> {
            self.started
                .lock()
                .unwrap()
                .push(service.name().to_string());
            if self.bind_listener {
                let addr = format!("127.0.0.1:{}", resolved.allocated_port);
                let listener = TcpListener::bind(&addr).expect("mock: failed to bind listener");
                self.listeners.lock().unwrap().push(listener);
            }
            Ok(ServiceHandle::Docker {
                container_id: format!("mock-{}", service.name()),
            })
        }

        async fn stop(&self, handle: &ServiceHandle) -> Result<(), BackendError> {
            self.stopped.lock().unwrap().push(handle.clone());
            Ok(())
        }

        async fn is_running(&self, _handle: &ServiceHandle) -> Result<bool, BackendError> {
            Ok(*self.is_running_returns.lock().unwrap())
        }

        fn backend_type(&self) -> &'static str {
            "mock"
        }
    }

    fn make_orchestrator(
        backend: MockBackend,
        state_manager: StateManager,
    ) -> Orchestrator<MockBackend> {
        Orchestrator::new(backend, state_manager)
    }

    fn temp_state_manager() -> (tempfile::TempDir, StateManager) {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = StateManager::with_path(tmp.path().to_path_buf(), "test-grove").unwrap();
        (tmp, mgr)
    }

    fn seed_service_state(mgr: &StateManager, name: &str, port: u16) {
        mgr.with_lock(|state| {
            state.services.insert(
                name.to_string(),
                ServiceState {
                    service_name: name.to_string(),
                    port,
                    handle: ServiceHandleState::Docker {
                        container_id: format!("mock-{name}"),
                    },
                    backend_type: "mock".to_string(),
                    started_at: "2026-01-01T00:00:00Z".to_string(),
                },
            );
            Ok(())
        })
        .unwrap();
    }

    // --- Tests: find_service ---

    #[test]
    fn find_service_returns_unknown_for_bad_name() {
        let (_tmp, mgr) = temp_state_manager();
        let orch = make_orchestrator(MockBackend::new(), mgr);
        let result = orch.find_service("nonexistent");
        match result {
            Err(GrovError::UnknownService { name }) => assert_eq!(name, "nonexistent"),
            Err(other) => panic!("expected UnknownService, got: {other:?}"),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    #[test]
    fn find_service_succeeds_for_known() {
        let (_tmp, mgr) = temp_state_manager();
        let orch = make_orchestrator(MockBackend::new(), mgr);
        let svc = orch.find_service("postgres").unwrap();
        assert_eq!(svc.name(), "postgres");
        let svc = orch.find_service("minio").unwrap();
        assert_eq!(svc.name(), "minio");
    }

    // --- Tests: install ---

    #[tokio::test]
    async fn install_validates_names() {
        let (_tmp, mgr) = temp_state_manager();
        let orch = make_orchestrator(MockBackend::new(), mgr);
        let result = orch.install(&["nonexistent".to_string()]).await;
        assert!(matches!(result, Err(GrovError::UnknownService { .. })));
    }

    #[tokio::test]
    async fn install_delegates_to_backend() {
        let (_tmp, mgr) = temp_state_manager();
        let backend = MockBackend::new();
        let orch = make_orchestrator(backend.clone(), mgr);
        orch.install(&["postgres".to_string(), "minio".to_string()])
            .await
            .unwrap();
        let installed = backend.installed.lock().unwrap();
        assert_eq!(*installed, vec!["postgres", "minio"]);
    }

    // --- Tests: up ---

    #[tokio::test]
    async fn up_starts_service_and_saves_state() {
        let (_tmp, mgr) = temp_state_manager();
        let backend = MockBackend::new().with_listener();
        let orch = make_orchestrator(backend.clone(), mgr);
        orch.up(&["postgres".to_string()]).await.unwrap();

        // Backend.start was called
        let started = backend.started.lock().unwrap();
        assert_eq!(*started, vec!["postgres"]);

        // State was persisted
        let state = orch.state_manager.load_state().unwrap();
        let svc = state.services.get("postgres").unwrap();
        assert_eq!(svc.service_name, "postgres");
        assert_eq!(svc.backend_type, "mock");
        assert!(svc.port >= 1024);
        assert!(!svc.started_at.is_empty());
    }

    #[tokio::test]
    async fn up_skips_already_running_service() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 55555);

        let backend = MockBackend::new().with_is_running(true).with_listener();
        let orch = make_orchestrator(backend.clone(), mgr);

        // Should skip postgres, start minio
        orch.up(&["postgres".to_string(), "minio".to_string()])
            .await
            .unwrap();

        let started = backend.started.lock().unwrap();
        assert_eq!(*started, vec!["minio"]);
    }

    #[tokio::test]
    async fn up_cleans_stale_state_and_restarts() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 55555);

        // is_running returns false => stale state
        let backend = MockBackend::new().with_listener();
        let orch = make_orchestrator(backend.clone(), mgr);

        orch.up(&["postgres".to_string()]).await.unwrap();

        // Backend.start was called (service was restarted)
        let started = backend.started.lock().unwrap();
        assert_eq!(*started, vec!["postgres"]);

        // State was updated with a new port (not the stale 55555)
        let state = orch.state_manager.load_state().unwrap();
        let svc = state.services.get("postgres").unwrap();
        assert_ne!(svc.port, 55555);
    }

    #[tokio::test]
    async fn up_validates_all_names_before_starting() {
        let (_tmp, mgr) = temp_state_manager();
        let backend = MockBackend::new().with_listener();
        let orch = make_orchestrator(backend.clone(), mgr);

        let result = orch
            .up(&["postgres".to_string(), "nonexistent".to_string()])
            .await;
        assert!(matches!(result, Err(GrovError::UnknownService { .. })));

        // Nothing was started because validation is upfront
        let started = backend.started.lock().unwrap();
        assert!(started.is_empty());
    }

    // --- Tests: down ---

    #[tokio::test]
    async fn down_with_unknown_name_returns_error() {
        let (_tmp, mgr) = temp_state_manager();
        let orch = make_orchestrator(MockBackend::new(), mgr);
        let result = orch.down(Some(&["nonexistent".to_string()])).await;
        assert!(matches!(result, Err(GrovError::UnknownService { .. })));
    }

    #[tokio::test]
    async fn down_skips_services_not_in_state() {
        let (_tmp, mgr) = temp_state_manager();
        let backend = MockBackend::new();
        let orch = make_orchestrator(backend.clone(), mgr);
        orch.down(Some(&["postgres".to_string()])).await.unwrap();
        let stopped = backend.stopped.lock().unwrap();
        assert!(stopped.is_empty());
    }

    #[tokio::test]
    async fn down_stops_service_and_removes_state() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 55555);

        let backend = MockBackend::new();
        let orch = make_orchestrator(backend.clone(), mgr);
        orch.down(Some(&["postgres".to_string()])).await.unwrap();

        // Backend.stop was called
        let stopped = backend.stopped.lock().unwrap();
        assert_eq!(stopped.len(), 1);

        // State was cleaned up
        let state = orch.state_manager.load_state().unwrap();
        assert!(!state.services.contains_key("postgres"));
    }

    #[tokio::test]
    async fn down_none_stops_all_services() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 55555);
        seed_service_state(&mgr, "minio", 9001);

        let backend = MockBackend::new();
        let orch = make_orchestrator(backend.clone(), mgr);
        orch.down(None).await.unwrap();

        // Both services stopped
        let stopped = backend.stopped.lock().unwrap();
        assert_eq!(stopped.len(), 2);

        // State is empty
        let state = orch.state_manager.load_state().unwrap();
        assert!(state.services.is_empty());
    }

    // --- Tests: conversions ---

    #[test]
    fn service_handle_to_state_docker_roundtrip() {
        let handle = ServiceHandle::Docker {
            container_id: "abc123".to_string(),
        };
        let state: ServiceHandleState = handle.clone().into();
        let back: ServiceHandle = state.into();
        match (&handle, &back) {
            (
                ServiceHandle::Docker { container_id: a },
                ServiceHandle::Docker { container_id: b },
            ) => assert_eq!(a, b),
            _ => panic!("expected Docker handles"),
        }
    }

    #[test]
    fn service_handle_to_state_native_roundtrip() {
        let handle = ServiceHandle::Native { pid: 12345 };
        let state: ServiceHandleState = handle.clone().into();
        let back: ServiceHandle = state.into();
        match (&handle, &back) {
            (ServiceHandle::Native { pid: a }, ServiceHandle::Native { pid: b }) => {
                assert_eq!(a, b)
            }
            _ => panic!("expected Native handles"),
        }
    }

    // --- Tests: helpers ---

    #[test]
    fn build_template_values_includes_port_and_defaults() {
        let services = services::builtin_services();
        let pg = services.iter().find(|s| s.name() == "postgres").unwrap();
        let values = build_template_values(pg.as_ref(), 54321);
        assert_eq!(values["port"], "54321");
        assert_eq!(values["username"], "dev");
        assert_eq!(values["password"], "dev");
        assert_eq!(values["database"], "myapp_dev");
    }

    #[test]
    fn render_service_env_postgres() {
        let services = services::builtin_services();
        let pg = services.iter().find(|s| s.name() == "postgres").unwrap();
        let rendered = render_service_env(pg.as_ref(), 54321).unwrap();
        assert_eq!(
            rendered["DATABASE_URL"],
            "postgresql://dev:dev@localhost:54321/myapp_dev"
        );
        assert_eq!(rendered["PGPORT"], "54321");
    }
}
