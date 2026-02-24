#![cfg(feature = "integration-tests")]

use grov::backend::docker::DockerBackend;
use grov::backend::{Backend, ServiceHandle};
use grov::orchestration::service::ResolvedService;
use grov::orchestration::services::Service;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Lightweight test service using nginx:alpine for Docker backend tests.
struct NginxTestService;

impl Service for NginxTestService {
    fn name(&self) -> &str {
        "nginx-test"
    }

    fn docker_image(&self) -> &str {
        "nginx:alpine"
    }

    fn process_env(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    fn default_port(&self) -> u16 {
        80
    }

    fn docker_data_mount(&self) -> &str {
        "/usr/share/nginx/html"
    }

    fn env_template(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    fn defaults(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

/// Generate a unique grove ID for test isolation.
fn unique_grove_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:08x}{:08x}", (nanos & 0xFFFF_FFFF) as u32, count as u32)
}

fn test_resolved(grove_id: String, port: u16, data_dir: std::path::PathBuf) -> ResolvedService {
    ResolvedService {
        grove_id,
        allocated_port: port,
        data_dir,
        resolved_env: HashMap::new(),
    }
}

#[tokio::test]
async fn install_pulls_image() {
    let backend = DockerBackend::new().await.expect("Docker must be running");
    let result = backend.install(&NginxTestService).await;
    assert!(result.is_ok(), "install failed: {:?}", result.err());
}

#[tokio::test]
async fn container_lifecycle() {
    let backend = DockerBackend::new().await.expect("Docker must be running");
    backend.install(&NginxTestService).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let resolved = test_resolved(
        unique_grove_id(),
        grov::orchestration::port::allocate().unwrap(),
        dir.path().to_path_buf(),
    );

    let handle = backend.start(&NginxTestService, &resolved).await.unwrap();
    assert!(matches!(handle, ServiceHandle::Docker { .. }));
    assert!(backend.is_running(&handle).await.unwrap());

    backend.stop(&handle).await.unwrap();
    assert!(!backend.is_running(&handle).await.unwrap());
}

#[tokio::test]
async fn idempotent_start_returns_same_container() {
    let backend = DockerBackend::new().await.expect("Docker must be running");
    backend.install(&NginxTestService).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let resolved = test_resolved(
        unique_grove_id(),
        grov::orchestration::port::allocate().unwrap(),
        dir.path().to_path_buf(),
    );

    let handle1 = backend.start(&NginxTestService, &resolved).await.unwrap();
    let handle2 = backend.start(&NginxTestService, &resolved).await.unwrap();

    match (&handle1, &handle2) {
        (
            ServiceHandle::Docker {
                container_id: id1, ..
            },
            ServiceHandle::Docker {
                container_id: id2, ..
            },
        ) => assert_eq!(id1, id2, "idempotent start should return same container"),
        _ => panic!("expected Docker handles"),
    }

    backend.stop(&handle1).await.unwrap();
}

#[tokio::test]
async fn port_binding_accessible() {
    let backend = DockerBackend::new().await.expect("Docker must be running");
    backend.install(&NginxTestService).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let port = grov::orchestration::port::allocate().unwrap();
    let resolved = test_resolved(unique_grove_id(), port, dir.path().to_path_buf());

    let handle = backend.start(&NginxTestService, &resolved).await.unwrap();

    // Give nginx a moment to start accepting connections
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let connected = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .is_ok();
    assert!(connected, "should connect to nginx on port {port}");

    backend.stop(&handle).await.unwrap();
}

#[tokio::test]
async fn data_directory_preserved_after_stop() {
    let backend = DockerBackend::new().await.expect("Docker must be running");
    backend.install(&NginxTestService).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let data_path = dir.path().to_path_buf();
    let resolved = test_resolved(
        unique_grove_id(),
        grov::orchestration::port::allocate().unwrap(),
        data_path.clone(),
    );

    let handle = backend.start(&NginxTestService, &resolved).await.unwrap();
    backend.stop(&handle).await.unwrap();

    assert!(
        data_path.exists(),
        "data directory should survive container removal"
    );
}

#[tokio::test]
async fn stop_nonexistent_container_succeeds() {
    let backend = DockerBackend::new().await.expect("Docker must be running");
    let handle = ServiceHandle::Docker {
        container_id: "nonexistent_container_id_12345".to_string(),
    };
    // Stopping a container that doesn't exist should not error
    let result = backend.stop(&handle).await;
    assert!(result.is_ok());
}
