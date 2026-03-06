pub mod env_template;
pub mod grove;
pub mod port;
pub mod service;
pub mod services;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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

// --- Grove-level helpers ---

pub async fn stop_grove_services<B: Backend>(
    backend: &B,
    state: &crate::storage::state::GroveState,
) {
    for (name, svc_state) in &state.services {
        let handle: ServiceHandle = svc_state.handle.clone().into();
        if let Err(e) = backend.stop(&handle).await {
            warn!(service = name.as_str(), error = %e, "failed to stop service during grove cleanup");
        }
    }
}

// --- Return types ---

pub struct EnvEntry {
    pub key: String,
    pub value: String,
}

pub struct ServiceStatus {
    pub name: String,
    pub backend: String,
    pub status: ServiceRunState,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceRunState {
    Running,
    Stopped,
}

impl std::fmt::Display for ServiceRunState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceRunState::Running => write!(f, "running"),
            ServiceRunState::Stopped => write!(f, "stopped"),
        }
    }
}

// --- Orchestrator ---

pub struct Orchestrator<B: Backend> {
    backend: B,
    state_manager: StateManager,
    services: Vec<Box<dyn Service>>,
    shutdown: Arc<AtomicBool>,
}

impl<B: Backend> Orchestrator<B> {
    pub fn new(backend: B, state_manager: StateManager, shutdown: Arc<AtomicBool>) -> Self {
        Self {
            backend,
            state_manager,
            services: services::builtin_services(),
            shutdown,
        }
    }

    pub fn store_path(&self) -> &std::path::PathBuf {
        self.state_manager.store_path()
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn find_service(&self, name: &str) -> Result<&dyn Service, GrovError> {
        self.services
            .iter()
            .find(|s| s.name() == name)
            .map(|s| s.as_ref())
            .ok_or_else(|| {
                let mut names: Vec<&str> = self.services.iter().map(|s| s.name()).collect();
                names.sort();
                GrovError::UnknownService {
                    name: name.to_string(),
                    available: names.join(", "),
                }
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
            if self.shutdown.load(Ordering::Relaxed) {
                return Err(GrovError::Interrupted);
            }

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

            // Ensure image/binary is available (auto-install)
            self.backend.install(service).await?;

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

    pub async fn clean(&self) -> Result<(), GrovError> {
        // Stop any running services first
        self.down(None).await?;
        info!("removing grove data");
        self.state_manager.remove_grove()?;
        info!("grove data removed");
        Ok(())
    }

    pub async fn env(&self) -> Result<Vec<EnvEntry>, GrovError> {
        let state = self.state_manager.load_state()?;
        if state.services.is_empty() {
            return Ok(vec![]);
        }

        let mut names: Vec<&String> = state.services.keys().collect();
        names.sort();

        let mut entries = Vec::new();
        let mut stale_names = Vec::new();
        for name in names {
            let svc_state = &state.services[name];
            let handle: ServiceHandle = svc_state.handle.clone().into();
            if !self.backend.is_running(&handle).await? {
                debug!(service = name, "skipping dead service in env output");
                stale_names.push(name.clone());
                continue;
            }

            let service = self.find_service(name)?;
            let rendered = render_service_env(service, svc_state.port).map_err(|e| {
                GrovError::Backend(BackendError::StartFailed {
                    service: name.to_string(),
                    reason: e.to_string(),
                })
            })?;

            let mut keys: Vec<String> = rendered.keys().cloned().collect();
            keys.sort();
            for key in keys {
                entries.push(EnvEntry {
                    key: key.clone(),
                    value: rendered[&key].clone(),
                });
            }
        }

        if !stale_names.is_empty() {
            self.state_manager.with_lock(|grove_state| {
                for name in &stale_names {
                    debug!(service = name, "removing stale state from env");
                    grove_state.services.remove(name);
                }
                Ok(())
            })?;
        }

        Ok(entries)
    }

    pub async fn status(&self) -> Result<Vec<ServiceStatus>, GrovError> {
        let state = self.state_manager.load_state()?;
        if state.services.is_empty() {
            return Ok(vec![]);
        }

        let mut names: Vec<&String> = state.services.keys().collect();
        names.sort();

        let mut statuses = Vec::new();
        let mut stale_names = Vec::new();
        for name in names {
            let svc_state = &state.services[name];
            let handle: ServiceHandle = svc_state.handle.clone().into();
            let running = self.backend.is_running(&handle).await?;
            if !running {
                stale_names.push(name.clone());
            }
            statuses.push(ServiceStatus {
                name: name.clone(),
                backend: svc_state.backend_type.clone(),
                status: if running {
                    ServiceRunState::Running
                } else {
                    ServiceRunState::Stopped
                },
                port: svc_state.port,
            });
        }

        if !stale_names.is_empty() {
            self.state_manager.with_lock(|grove_state| {
                for name in &stale_names {
                    debug!(service = name, "removing stale state from status");
                    grove_state.services.remove(name);
                }
                Ok(())
            })?;
        }

        Ok(statuses)
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
        Orchestrator::new(backend, state_manager, Arc::new(AtomicBool::new(false)))
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
            Err(GrovError::UnknownService { name, .. }) => assert_eq!(name, "nonexistent"),
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

    #[test]
    fn find_service_lists_available_services() {
        let (_tmp, mgr) = temp_state_manager();
        let orch = make_orchestrator(MockBackend::new(), mgr);
        match orch.find_service("postgre") {
            Err(GrovError::UnknownService { available, .. }) => {
                assert_eq!(available, "minio, postgres");
            }
            _ => panic!("expected UnknownService with available list"),
        }
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

    #[tokio::test]
    async fn up_stops_on_shutdown_signal() {
        let (_tmp, mgr) = temp_state_manager();
        let backend = MockBackend::new().with_listener();
        let shutdown = Arc::new(AtomicBool::new(true));
        let orch = Orchestrator::new(backend.clone(), mgr, shutdown);

        let result = orch.up(&["postgres".to_string()]).await;
        assert!(matches!(result, Err(GrovError::Interrupted)));

        // Nothing was started because shutdown was already set
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

    // --- Tests: clean ---

    #[tokio::test]
    async fn clean_removes_store_directory() {
        let (_tmp, mgr) = temp_state_manager();
        mgr.ensure_data_dir("postgres").unwrap();
        let store_path = mgr.store_path().clone();
        assert!(store_path.exists());

        let orch = make_orchestrator(MockBackend::new(), mgr);
        orch.clean().await.unwrap();

        assert!(!store_path.exists(), "store directory should be removed");
    }

    #[tokio::test]
    async fn clean_stops_running_services_first() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 55555);

        let backend = MockBackend::new();
        let orch = make_orchestrator(backend.clone(), mgr);
        orch.clean().await.unwrap();

        // Backend.stop was called
        let stopped = backend.stopped.lock().unwrap();
        assert_eq!(stopped.len(), 1);
    }

    #[tokio::test]
    async fn clean_succeeds_with_no_services() {
        let (_tmp, mgr) = temp_state_manager();
        let store_path = mgr.store_path().clone();
        let orch = make_orchestrator(MockBackend::new(), mgr);
        orch.clean().await.unwrap();
        assert!(!store_path.exists());
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

    // --- Tests: env ---

    #[tokio::test]
    async fn env_returns_empty_when_no_services() {
        let (_tmp, mgr) = temp_state_manager();
        let orch = make_orchestrator(MockBackend::new(), mgr);
        let entries = orch.env().await.unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn env_returns_rendered_variables_for_running_service() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 54321);
        let orch = make_orchestrator(MockBackend::new().with_is_running(true), mgr);
        let entries = orch.env().await.unwrap();

        let find = |k: &str| {
            entries
                .iter()
                .find(|e| e.key == k)
                .map(|e| e.value.as_str())
        };
        assert_eq!(
            find("DATABASE_URL"),
            Some("postgresql://dev:dev@localhost:54321/myapp_dev")
        );
        assert_eq!(find("PGPORT"), Some("54321"));
    }

    #[tokio::test]
    async fn env_returns_entries_for_multiple_services_sorted() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 54321);
        seed_service_state(&mgr, "minio", 9001);
        let orch = make_orchestrator(MockBackend::new().with_is_running(true), mgr);
        let entries = orch.env().await.unwrap();

        // postgres has 6 env vars, minio has 3 => 9 total
        assert_eq!(entries.len(), 9);

        // minio keys come first (alphabetical by service name)
        // and within each service, keys are sorted
        let keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys[0], "AWS_ACCESS_KEY_ID");
        assert_eq!(keys[1], "AWS_SECRET_ACCESS_KEY");
        assert_eq!(keys[2], "MINIO_ENDPOINT");
        // then postgres keys, sorted
        assert_eq!(keys[3], "DATABASE_URL");
        assert_eq!(keys[4], "PGDATABASE");
        assert_eq!(keys[5], "PGHOST");
        assert_eq!(keys[6], "PGPASSWORD");
        assert_eq!(keys[7], "PGPORT");
        assert_eq!(keys[8], "PGUSER");
    }

    #[tokio::test]
    async fn env_returns_error_for_unknown_service_in_state() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "nonexistent", 12345);
        let orch = make_orchestrator(MockBackend::new().with_is_running(true), mgr);
        let result = orch.env().await;
        assert!(matches!(result, Err(GrovError::UnknownService { .. })));
    }

    #[tokio::test]
    async fn env_skips_dead_services_and_cleans_state() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 54321);
        let orch = make_orchestrator(MockBackend::new().with_is_running(false), mgr);
        let entries = orch.env().await.unwrap();

        // Dead service is skipped — no env entries
        assert!(entries.is_empty());

        // State was cleaned
        let state = orch.state_manager.load_state().unwrap();
        assert!(!state.services.contains_key("postgres"));
    }

    // --- Tests: status ---

    #[tokio::test]
    async fn status_returns_empty_when_no_services() {
        let (_tmp, mgr) = temp_state_manager();
        let orch = make_orchestrator(MockBackend::new(), mgr);
        let statuses = orch.status().await.unwrap();
        assert!(statuses.is_empty());
    }

    #[tokio::test]
    async fn status_returns_running_service() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 54321);
        let orch = make_orchestrator(MockBackend::new().with_is_running(true), mgr);
        let statuses = orch.status().await.unwrap();

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].name, "postgres");
        assert_eq!(statuses[0].backend, "mock");
        assert_eq!(statuses[0].status, ServiceRunState::Running);
        assert_eq!(statuses[0].port, 54321);
    }

    #[tokio::test]
    async fn status_returns_stopped_service() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 54321);
        let orch = make_orchestrator(MockBackend::new().with_is_running(false), mgr);
        let statuses = orch.status().await.unwrap();

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].status, ServiceRunState::Stopped);
    }

    #[tokio::test]
    async fn status_returns_multiple_services_sorted() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 54321);
        seed_service_state(&mgr, "minio", 9001);
        let orch = make_orchestrator(MockBackend::new().with_is_running(true), mgr);
        let statuses = orch.status().await.unwrap();

        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].name, "minio");
        assert_eq!(statuses[1].name, "postgres");
    }

    #[tokio::test]
    async fn status_cleans_stale_state_for_dead_services() {
        let (_tmp, mgr) = temp_state_manager();
        seed_service_state(&mgr, "postgres", 54321);
        let orch = make_orchestrator(MockBackend::new().with_is_running(false), mgr);
        let statuses = orch.status().await.unwrap();

        // Dead service is reported as Stopped this one time
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].status, ServiceRunState::Stopped);

        // State was cleaned
        let state = orch.state_manager.load_state().unwrap();
        assert!(!state.services.contains_key("postgres"));
    }

    #[test]
    fn service_run_state_display() {
        assert_eq!(ServiceRunState::Running.to_string(), "running");
        assert_eq!(ServiceRunState::Stopped.to_string(), "stopped");
    }

    // --- Tests: stop_grove_services ---

    #[tokio::test]
    async fn stop_grove_services_stops_all_handles() {
        let backend = MockBackend::new();
        let mut state =
            crate::storage::state::GroveState::new("test".to_string(), "/test".to_string());
        state.services.insert(
            "postgres".to_string(),
            ServiceState {
                service_name: "postgres".to_string(),
                port: 54321,
                handle: ServiceHandleState::Docker {
                    container_id: "abc".to_string(),
                },
                backend_type: "mock".to_string(),
                started_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        state.services.insert(
            "minio".to_string(),
            ServiceState {
                service_name: "minio".to_string(),
                port: 9001,
                handle: ServiceHandleState::Docker {
                    container_id: "def".to_string(),
                },
                backend_type: "mock".to_string(),
                started_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );

        stop_grove_services(&backend, &state).await;

        let stopped = backend.stopped.lock().unwrap();
        assert_eq!(stopped.len(), 2);
    }

    #[tokio::test]
    async fn stop_grove_services_continues_past_errors() {
        // Create a backend that fails on stop
        let backend = FailingStopBackend {
            stop_called: Arc::new(Mutex::new(0)),
        };
        let mut state =
            crate::storage::state::GroveState::new("test".to_string(), "/test".to_string());
        state.services.insert(
            "postgres".to_string(),
            ServiceState {
                service_name: "postgres".to_string(),
                port: 54321,
                handle: ServiceHandleState::Docker {
                    container_id: "abc".to_string(),
                },
                backend_type: "mock".to_string(),
                started_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );
        state.services.insert(
            "minio".to_string(),
            ServiceState {
                service_name: "minio".to_string(),
                port: 9001,
                handle: ServiceHandleState::Docker {
                    container_id: "def".to_string(),
                },
                backend_type: "mock".to_string(),
                started_at: "2026-01-01T00:00:00Z".to_string(),
            },
        );

        stop_grove_services(&backend, &state).await;

        // Both services were attempted despite errors
        let count = *backend.stop_called.lock().unwrap();
        assert_eq!(count, 2);
    }

    struct FailingStopBackend {
        stop_called: Arc<Mutex<u32>>,
    }

    impl Backend for FailingStopBackend {
        async fn install(&self, _service: &dyn Service) -> Result<(), BackendError> {
            Ok(())
        }

        async fn start(
            &self,
            _service: &dyn Service,
            _resolved: &ResolvedService,
        ) -> Result<ServiceHandle, BackendError> {
            Ok(ServiceHandle::Docker {
                container_id: "x".to_string(),
            })
        }

        async fn stop(&self, _handle: &ServiceHandle) -> Result<(), BackendError> {
            *self.stop_called.lock().unwrap() += 1;
            Err(BackendError::StopFailed {
                service: "test".to_string(),
                reason: "mock failure".to_string(),
            })
        }

        async fn is_running(&self, _handle: &ServiceHandle) -> Result<bool, BackendError> {
            Ok(false)
        }

        fn backend_type(&self) -> &'static str {
            "failing-mock"
        }
    }
}
