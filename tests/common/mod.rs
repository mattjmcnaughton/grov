use std::collections::HashMap;
use std::path::PathBuf;

use assert_cmd::Command;
use bollard::Docker;
use bollard::container::ListContainersOptions;
use bollard::container::RemoveContainerOptions;
use bollard::container::StopContainerOptions;
use tempfile::TempDir;

/// Test fixture that provides an isolated grove (temp directory + computed grove ID)
/// and cleans up Docker containers + state directory on drop.
pub struct TestGrove {
    pub temp_dir: TempDir,
    pub grove_id: String,
    pub grove_prefix: String,
}

impl TestGrove {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        // Canonicalize to match what the binary sees via std::env::current_dir().
        // On macOS, TempDir returns /var/folders/... but current_dir() resolves
        // the symlink to /private/var/folders/..., producing a different hash.
        let canonical = temp_dir
            .path()
            .canonicalize()
            .expect("canonicalize temp dir");
        let grove_id = grov::orchestration::grove::resolve_path(&canonical);
        let grove_prefix = grove_id[..8].to_string();
        Self {
            temp_dir,
            grove_id,
            grove_prefix,
        }
    }

    /// Returns an `assert_cmd::Command` for the `grov` binary, with cwd set to this grove's
    /// temp directory.
    pub fn cmd(&self) -> Command {
        let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("grov");
        cmd.current_dir(self.temp_dir.path());
        cmd
    }

    /// Parse `grov env` stdout into a HashMap of KEY=VALUE pairs.
    pub fn parse_env_output(stdout: &str) -> HashMap<String, String> {
        stdout
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let (key, value) = line.split_once('=')?;
                Some((key.to_string(), value.to_string()))
            })
            .collect()
    }

    /// Path to `~/.grov/store/{grove_id}/`
    pub fn store_path(&self) -> PathBuf {
        directories::BaseDirs::new()
            .expect("home dir")
            .home_dir()
            .join(".grov")
            .join("store")
            .join(&self.grove_id)
    }
}

/// Connect to the Docker daemon, reusing grov's own client resolution logic
/// (DOCKER_HOST, Docker contexts, then local defaults).
pub fn connect_docker() -> Option<Docker> {
    grov::backend::docker::DockerBackend::create_client().ok()
}

impl Drop for TestGrove {
    fn drop(&mut self) {
        let prefix = format!("grov-{}", self.grove_prefix);
        let store_path = self.store_path();

        // Spawn a dedicated thread for cleanup to avoid "cannot start a runtime
        // from within a runtime" when Drop runs inside #[tokio::test].
        let handle = std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    if let Some(docker) = connect_docker() {
                        let filters: HashMap<String, Vec<String>> =
                            [("name".to_string(), vec![prefix])].into();
                        let opts = ListContainersOptions {
                            all: true,
                            filters,
                            ..Default::default()
                        };
                        if let Ok(containers) = docker.list_containers(Some(opts)).await {
                            for c in containers {
                                let id = match c.id {
                                    Some(ref id) => id.clone(),
                                    None => continue,
                                };
                                let _ = docker
                                    .stop_container(&id, Some(StopContainerOptions { t: 5 }))
                                    .await;
                                let _ = docker
                                    .remove_container(
                                        &id,
                                        Some(RemoveContainerOptions {
                                            force: true,
                                            ..Default::default()
                                        }),
                                    )
                                    .await;
                            }
                        }
                    }
                });
            }

            // Remove state directory to avoid accumulation
            let _ = std::fs::remove_dir_all(&store_path);
        });
        let _ = handle.join();
    }
}

/// Connect to postgres with a retry loop, returning the client and connection join handle.
pub async fn connect_postgres(
    port: u16,
) -> (
    tokio_postgres::Client,
    tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
) {
    let connstr = format!(
        "host=localhost port={} user=dev password=dev dbname=myapp_dev",
        port
    );
    for attempt in 1..=30 {
        match tokio_postgres::connect(&connstr, tokio_postgres::NoTls).await {
            Ok((client, connection)) => {
                let handle = tokio::spawn(connection);
                return (client, handle);
            }
            Err(_) if attempt < 30 => {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            Err(e) => panic!("failed to connect to postgres after 30 attempts: {e}"),
        }
    }
    unreachable!()
}
