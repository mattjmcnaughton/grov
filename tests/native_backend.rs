#![cfg(all(feature = "native-integration-tests", target_os = "linux"))]

use grov::backend::native::NativeBackend;
use grov::backend::{Backend, ServiceHandle};
use grov::orchestration::service::ResolvedService;
use grov::orchestration::services::Service;
use std::collections::HashMap;
use std::path::PathBuf;

fn test_resolved(port: u16, data_dir: PathBuf) -> ResolvedService {
    ResolvedService {
        grove_id: "native-test".to_string(),
        allocated_port: port,
        data_dir,
        resolved_env: HashMap::new(),
    }
}

/// Postgres service definition for integration tests.
struct PostgresTestService;

impl Service for PostgresTestService {
    fn name(&self) -> &str {
        "postgres"
    }
    fn docker_image(&self) -> &str {
        "postgres:16-alpine"
    }
    fn process_env(&self) -> HashMap<String, String> {
        HashMap::from([
            ("POSTGRES_USER".to_string(), "dev".to_string()),
            ("POSTGRES_PASSWORD".to_string(), "dev".to_string()),
            ("POSTGRES_DB".to_string(), "myapp_dev".to_string()),
        ])
    }
    fn default_port(&self) -> u16 {
        5432
    }
    fn docker_data_mount(&self) -> &str {
        "/var/lib/postgresql/data"
    }
    fn env_template(&self) -> HashMap<String, String> {
        HashMap::new()
    }
    fn defaults(&self) -> HashMap<String, String> {
        HashMap::new()
    }
    fn native_binary(&self) -> Option<&str> {
        Some("postgres")
    }
    fn native_args(&self, resolved: &ResolvedService) -> Vec<String> {
        vec![
            "-D".to_string(),
            resolved.data_dir.to_string_lossy().to_string(),
            "-p".to_string(),
            resolved.allocated_port.to_string(),
            "-k".to_string(),
            "/tmp".to_string(),
        ]
    }
    fn native_init(
        &self,
        resolved: &ResolvedService,
    ) -> Option<grov::orchestration::service::NativeInitStep> {
        if resolved.data_dir.join("PG_VERSION").exists() {
            return None;
        }
        Some(grov::orchestration::service::NativeInitStep {
            command: "initdb".to_string(),
            args: vec![
                "-D".to_string(),
                resolved.data_dir.to_string_lossy().to_string(),
            ],
        })
    }
}

/// MinIO service definition for integration tests.
struct MinioTestService;

impl Service for MinioTestService {
    fn name(&self) -> &str {
        "minio"
    }
    fn docker_image(&self) -> &str {
        "minio/minio:latest"
    }
    fn process_env(&self) -> HashMap<String, String> {
        HashMap::from([
            ("MINIO_ROOT_USER".to_string(), "minioadmin".to_string()),
            ("MINIO_ROOT_PASSWORD".to_string(), "minioadmin".to_string()),
        ])
    }
    fn default_port(&self) -> u16 {
        9000
    }
    fn docker_data_mount(&self) -> &str {
        "/data"
    }
    fn env_template(&self) -> HashMap<String, String> {
        HashMap::new()
    }
    fn defaults(&self) -> HashMap<String, String> {
        HashMap::new()
    }
    fn native_binary(&self) -> Option<&str> {
        Some("minio")
    }
    fn native_args(&self, resolved: &ResolvedService) -> Vec<String> {
        vec![
            "server".to_string(),
            resolved.data_dir.to_string_lossy().to_string(),
            "--address".to_string(),
            format!(":{}", resolved.allocated_port),
        ]
    }
}

#[tokio::test]
async fn postgres_native_lifecycle() {
    let backend = NativeBackend;
    backend.install(&PostgresTestService).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("pgdata");
    std::fs::create_dir_all(&data_dir).unwrap();
    let port = grov::orchestration::port::allocate().unwrap();
    let resolved = test_resolved(port, data_dir);

    let handle = backend
        .start(&PostgresTestService, &resolved)
        .await
        .unwrap();
    assert!(matches!(handle, ServiceHandle::Native { .. }));

    // Wait for postgres to be ready
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    assert!(backend.is_running(&handle).await.unwrap());

    // Verify TCP connectivity
    let connected = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .is_ok();
    assert!(connected, "should connect to postgres on port {port}");

    backend.stop(&handle).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(!backend.is_running(&handle).await.unwrap());
}

#[tokio::test]
async fn minio_native_lifecycle() {
    let backend = NativeBackend;
    backend.install(&MinioTestService).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("miniodata");
    std::fs::create_dir_all(&data_dir).unwrap();
    let port = grov::orchestration::port::allocate().unwrap();
    let resolved = test_resolved(port, data_dir);

    let handle = backend.start(&MinioTestService, &resolved).await.unwrap();
    assert!(matches!(handle, ServiceHandle::Native { .. }));

    // Wait for minio to be ready
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    assert!(backend.is_running(&handle).await.unwrap());

    // Verify TCP connectivity
    let connected = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .is_ok();
    assert!(connected, "should connect to minio on port {port}");

    backend.stop(&handle).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(!backend.is_running(&handle).await.unwrap());
}

#[tokio::test]
async fn postgres_initdb_creates_pg_version() {
    let backend = NativeBackend;
    backend.install(&PostgresTestService).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("pgdata");
    std::fs::create_dir_all(&data_dir).unwrap();
    let port = grov::orchestration::port::allocate().unwrap();
    let resolved = test_resolved(port, data_dir.clone());

    let handle = backend
        .start(&PostgresTestService, &resolved)
        .await
        .unwrap();

    assert!(
        data_dir.join("PG_VERSION").exists(),
        "initdb should create PG_VERSION file"
    );

    backend.stop(&handle).await.unwrap();
}

#[tokio::test]
async fn postgres_second_start_skips_initdb() {
    let backend = NativeBackend;
    backend.install(&PostgresTestService).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("pgdata");
    std::fs::create_dir_all(&data_dir).unwrap();
    let port = grov::orchestration::port::allocate().unwrap();
    let resolved = test_resolved(port, data_dir.clone());

    // First start -- initdb runs
    let handle1 = backend
        .start(&PostgresTestService, &resolved)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    backend.stop(&handle1).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Record modification time of PG_VERSION
    let pg_version_path = data_dir.join("PG_VERSION");
    let mtime_before = std::fs::metadata(&pg_version_path)
        .unwrap()
        .modified()
        .unwrap();

    // Small sleep to ensure any new write would have a different mtime
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Second start -- initdb should be skipped because PG_VERSION exists
    let port2 = grov::orchestration::port::allocate().unwrap();
    let resolved2 = test_resolved(port2, data_dir.clone());
    let handle2 = backend
        .start(&PostgresTestService, &resolved2)
        .await
        .unwrap();

    let mtime_after = std::fs::metadata(&pg_version_path)
        .unwrap()
        .modified()
        .unwrap();

    assert_eq!(
        mtime_before, mtime_after,
        "PG_VERSION should not be modified on second start (initdb skipped)"
    );

    backend.stop(&handle2).await.unwrap();
}
