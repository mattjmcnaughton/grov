use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::time::Duration;
use tokio::fs::OpenOptions;
use tracing::{debug, info, warn};

use super::{Backend, BackendError, ServiceHandle};
use crate::orchestration::service::ResolvedService;
use crate::orchestration::services::Service;

const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub struct NativeBackend;

/// Check whether a process with the given PID is alive.
///
/// Uses `kill(pid, 0)` which checks existence without sending a signal.
/// Treats EPERM (permission denied) as alive -- the process exists but
/// is owned by another user.
fn is_process_alive(pid: u32) -> bool {
    match signal::kill(Pid::from_raw(pid as i32), None) {
        Ok(()) => true,
        Err(nix::errno::Errno::EPERM) => true,
        Err(_) => false,
    }
}

impl Backend for NativeBackend {
    async fn install(&self, service: &dyn Service) -> Result<(), BackendError> {
        let binary = service
            .native_binary()
            .ok_or_else(|| BackendError::BinaryNotFound {
                binary: format!("{} (no native binary defined)", service.name()),
            })?;

        which::which(binary).map_err(|_| BackendError::BinaryNotFound {
            binary: binary.to_string(),
        })?;

        debug!(binary, "native binary found in PATH");
        Ok(())
    }

    async fn start(
        &self,
        service: &dyn Service,
        resolved: &ResolvedService,
    ) -> Result<ServiceHandle, BackendError> {
        let binary = service
            .native_binary()
            .ok_or_else(|| BackendError::BinaryNotFound {
                binary: format!("{} (no native binary defined)", service.name()),
            })?;

        // Run init step if needed (e.g., initdb for postgres)
        if let Some(init) = service.native_init(resolved) {
            debug!(command = %init.command, "running native init step");
            let output = tokio::process::Command::new(&init.command)
                .args(&init.args)
                .output()
                .await
                .map_err(|e| BackendError::StartFailed {
                    service: service.name().to_string(),
                    reason: format!("failed to run init command '{}': {}", init.command, e),
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(BackendError::StartFailed {
                    service: service.name().to_string(),
                    reason: format!(
                        "init command '{}' failed (exit {}): {}",
                        init.command,
                        output.status,
                        stderr.trim()
                    ),
                });
            }
            info!(command = %init.command, "native init step completed");
        }

        // Prepare log file: {data_dir}/../{service_name}.log
        let service_dir = resolved.data_dir.parent().unwrap_or(&resolved.data_dir);
        let log_path = service_dir.join(format!("{}.log", service.name()));
        let pid_path = service_dir.join(format!("{}.pid", service.name()));

        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .await
            .map_err(|e| BackendError::StartFailed {
                service: service.name().to_string(),
                reason: format!("failed to open log file {}: {}", log_path.display(), e),
            })?;

        let stdout_file = log_file.into_std().await;
        let stderr_file = stdout_file
            .try_clone()
            .map_err(|e| BackendError::StartFailed {
                service: service.name().to_string(),
                reason: format!("failed to clone log file handle: {e}"),
            })?;

        // Spawn the service process
        let args = service.native_args(resolved);
        let envs = service.process_env();

        debug!(binary, ?args, "spawning native process");

        let child = tokio::process::Command::new(binary)
            .args(&args)
            .envs(&envs)
            .stdout(stdout_file)
            .stderr(stderr_file)
            .spawn()
            .map_err(|e| BackendError::StartFailed {
                service: service.name().to_string(),
                reason: format!("failed to spawn '{}': {}", binary, e),
            })?;

        let pid = child.id().ok_or_else(|| BackendError::StartFailed {
            service: service.name().to_string(),
            reason: "process exited immediately after spawn".to_string(),
        })?;

        // Write PID file
        tokio::fs::write(&pid_path, pid.to_string())
            .await
            .map_err(|e| BackendError::StartFailed {
                service: service.name().to_string(),
                reason: format!("failed to write PID file {}: {}", pid_path.display(), e),
            })?;

        info!(
            binary,
            pid,
            log = %log_path.display(),
            "started native process"
        );

        Ok(ServiceHandle::Native { pid })
    }

    async fn stop(&self, handle: &ServiceHandle) -> Result<(), BackendError> {
        let pid = match handle {
            ServiceHandle::Native { pid } => *pid,
            _ => {
                return Err(BackendError::StopFailed {
                    service: "unknown".to_string(),
                    reason: "expected Native handle".to_string(),
                });
            }
        };

        if !is_process_alive(pid) {
            debug!(pid, "process already dead");
            return Ok(());
        }

        // Send SIGTERM for graceful shutdown
        debug!(pid, "sending SIGTERM");
        if let Err(e) = signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
            // ESRCH means process already gone -- that's fine
            if e != nix::errno::Errno::ESRCH {
                return Err(BackendError::StopFailed {
                    service: "unknown".to_string(),
                    reason: format!("failed to send SIGTERM to pid {pid}: {e}"),
                });
            }
            return Ok(());
        }

        // Poll for process exit
        let mut elapsed = Duration::ZERO;
        while elapsed < GRACEFUL_SHUTDOWN_TIMEOUT {
            tokio::time::sleep(SHUTDOWN_POLL_INTERVAL).await;
            elapsed += SHUTDOWN_POLL_INTERVAL;
            if !is_process_alive(pid) {
                info!(pid, "process stopped gracefully");
                return Ok(());
            }
        }

        // Escalate to SIGKILL
        warn!(pid, "graceful shutdown timed out, sending SIGKILL");
        if let Err(e) = signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL)
            && e != nix::errno::Errno::ESRCH
        {
            return Err(BackendError::StopFailed {
                service: "unknown".to_string(),
                reason: format!("failed to send SIGKILL to pid {pid}: {e}"),
            });
        }

        info!(pid, "process killed with SIGKILL");
        Ok(())
    }

    async fn is_running(&self, handle: &ServiceHandle) -> Result<bool, BackendError> {
        match handle {
            ServiceHandle::Native { pid } => Ok(is_process_alive(*pid)),
            _ => Ok(false),
        }
    }

    fn backend_type(&self) -> &'static str {
        "native"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::service::{NativeInitStep, ResolvedService};
    use crate::orchestration::services::Service;
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Test service that uses `sleep` as its binary (universally available).
    struct SleepTestService;

    impl Service for SleepTestService {
        fn name(&self) -> &str {
            "sleep-test"
        }
        fn docker_image(&self) -> &str {
            "unused"
        }
        fn process_env(&self) -> HashMap<String, String> {
            HashMap::new()
        }
        fn default_port(&self) -> u16 {
            0
        }
        fn docker_data_mount(&self) -> &str {
            "/unused"
        }
        fn env_template(&self) -> HashMap<String, String> {
            HashMap::new()
        }
        fn defaults(&self) -> HashMap<String, String> {
            HashMap::new()
        }
        fn native_binary(&self) -> Option<&str> {
            Some("sleep")
        }
        fn native_args(&self, _resolved: &ResolvedService) -> Vec<String> {
            vec!["300".to_string()]
        }
    }

    /// Test service with no native binary defined.
    struct NoNativeBinaryService;

    impl Service for NoNativeBinaryService {
        fn name(&self) -> &str {
            "no-native"
        }
        fn docker_image(&self) -> &str {
            "unused"
        }
        fn process_env(&self) -> HashMap<String, String> {
            HashMap::new()
        }
        fn default_port(&self) -> u16 {
            0
        }
        fn docker_data_mount(&self) -> &str {
            "/unused"
        }
        fn env_template(&self) -> HashMap<String, String> {
            HashMap::new()
        }
        fn defaults(&self) -> HashMap<String, String> {
            HashMap::new()
        }
    }

    /// Test service with a native init step that succeeds (`true` command).
    struct InitSuccessService;

    impl Service for InitSuccessService {
        fn name(&self) -> &str {
            "init-success"
        }
        fn docker_image(&self) -> &str {
            "unused"
        }
        fn process_env(&self) -> HashMap<String, String> {
            HashMap::new()
        }
        fn default_port(&self) -> u16 {
            0
        }
        fn docker_data_mount(&self) -> &str {
            "/unused"
        }
        fn env_template(&self) -> HashMap<String, String> {
            HashMap::new()
        }
        fn defaults(&self) -> HashMap<String, String> {
            HashMap::new()
        }
        fn native_binary(&self) -> Option<&str> {
            Some("sleep")
        }
        fn native_args(&self, _resolved: &ResolvedService) -> Vec<String> {
            vec!["300".to_string()]
        }
        fn native_init(&self, _resolved: &ResolvedService) -> Option<NativeInitStep> {
            Some(NativeInitStep {
                command: "true".to_string(),
                args: vec![],
            })
        }
    }

    /// Test service with a native init step that fails (`false` command).
    struct InitFailService;

    impl Service for InitFailService {
        fn name(&self) -> &str {
            "init-fail"
        }
        fn docker_image(&self) -> &str {
            "unused"
        }
        fn process_env(&self) -> HashMap<String, String> {
            HashMap::new()
        }
        fn default_port(&self) -> u16 {
            0
        }
        fn docker_data_mount(&self) -> &str {
            "/unused"
        }
        fn env_template(&self) -> HashMap<String, String> {
            HashMap::new()
        }
        fn defaults(&self) -> HashMap<String, String> {
            HashMap::new()
        }
        fn native_binary(&self) -> Option<&str> {
            Some("sleep")
        }
        fn native_args(&self, _resolved: &ResolvedService) -> Vec<String> {
            vec!["300".to_string()]
        }
        fn native_init(&self, _resolved: &ResolvedService) -> Option<NativeInitStep> {
            Some(NativeInitStep {
                command: "false".to_string(),
                args: vec![],
            })
        }
    }

    fn test_resolved(data_dir: PathBuf) -> ResolvedService {
        ResolvedService {
            grove_id: "test-grove".to_string(),
            allocated_port: 0,
            data_dir,
            resolved_env: HashMap::new(),
        }
    }

    #[test]
    fn backend_type_is_native() {
        let backend = NativeBackend;
        assert_eq!(backend.backend_type(), "native");
    }

    #[tokio::test]
    async fn install_succeeds_for_existing_binary() {
        let backend = NativeBackend;
        let result = backend.install(&SleepTestService).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn install_fails_for_missing_binary() {
        struct MissingBinaryService;
        impl Service for MissingBinaryService {
            fn name(&self) -> &str {
                "missing"
            }
            fn docker_image(&self) -> &str {
                "unused"
            }
            fn process_env(&self) -> HashMap<String, String> {
                HashMap::new()
            }
            fn default_port(&self) -> u16 {
                0
            }
            fn docker_data_mount(&self) -> &str {
                "/unused"
            }
            fn env_template(&self) -> HashMap<String, String> {
                HashMap::new()
            }
            fn defaults(&self) -> HashMap<String, String> {
                HashMap::new()
            }
            fn native_binary(&self) -> Option<&str> {
                Some("totally_nonexistent_binary_xyz_123")
            }
        }

        let backend = NativeBackend;
        let result = backend.install(&MissingBinaryService).await;
        assert!(matches!(result, Err(BackendError::BinaryNotFound { .. })));
    }

    #[tokio::test]
    async fn install_fails_for_no_native_binary() {
        let backend = NativeBackend;
        let result = backend.install(&NoNativeBinaryService).await;
        assert!(matches!(result, Err(BackendError::BinaryNotFound { .. })));
    }

    #[tokio::test]
    async fn start_and_stop_lifecycle() {
        let backend = NativeBackend;
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let resolved = test_resolved(data_dir);

        let handle = backend.start(&SleepTestService, &resolved).await.unwrap();
        assert!(matches!(handle, ServiceHandle::Native { .. }));
        assert!(backend.is_running(&handle).await.unwrap());

        backend.stop(&handle).await.unwrap();

        // Give the OS a moment to reap
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!backend.is_running(&handle).await.unwrap());
    }

    #[tokio::test]
    async fn stop_already_dead_process_succeeds() {
        let backend = NativeBackend;
        // Use a PID that almost certainly doesn't exist
        let handle = ServiceHandle::Native { pid: 4_000_000 };
        let result = backend.stop(&handle).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn is_running_returns_false_for_dead_pid() {
        let backend = NativeBackend;
        let handle = ServiceHandle::Native { pid: 4_000_000 };
        assert!(!backend.is_running(&handle).await.unwrap());
    }

    #[tokio::test]
    async fn is_running_returns_false_for_docker_handle() {
        let backend = NativeBackend;
        let handle = ServiceHandle::Docker {
            container_id: "abc123".to_string(),
        };
        assert!(!backend.is_running(&handle).await.unwrap());
    }

    #[tokio::test]
    async fn start_runs_init_step() {
        let backend = NativeBackend;
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let resolved = test_resolved(data_dir);

        // InitSuccessService uses `true` as init command -- should succeed
        let handle = backend.start(&InitSuccessService, &resolved).await.unwrap();
        assert!(backend.is_running(&handle).await.unwrap());
        backend.stop(&handle).await.unwrap();
    }

    #[tokio::test]
    async fn start_fails_on_bad_init() {
        let backend = NativeBackend;
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let resolved = test_resolved(data_dir);

        // InitFailService uses `false` as init command -- should fail
        let result = backend.start(&InitFailService, &resolved).await;
        assert!(matches!(result, Err(BackendError::StartFailed { .. })));
    }

    #[tokio::test]
    async fn log_file_created_on_start() {
        let backend = NativeBackend;
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let resolved = test_resolved(data_dir);

        let handle = backend.start(&SleepTestService, &resolved).await.unwrap();

        let log_path = dir.path().join("sleep-test.log");
        assert!(log_path.exists(), "log file should be created");

        backend.stop(&handle).await.unwrap();
    }

    #[tokio::test]
    async fn pid_file_created_on_start() {
        let backend = NativeBackend;
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let resolved = test_resolved(data_dir);

        let handle = backend.start(&SleepTestService, &resolved).await.unwrap();

        let pid_path = dir.path().join("sleep-test.pid");
        assert!(pid_path.exists(), "PID file should be created");

        let pid_content = std::fs::read_to_string(&pid_path).unwrap();
        if let ServiceHandle::Native { pid } = handle {
            assert_eq!(pid_content, pid.to_string());
        } else {
            panic!("expected Native handle");
        }

        backend.stop(&handle).await.unwrap();
    }

    #[test]
    fn is_process_alive_for_current_process() {
        let pid = std::process::id();
        assert!(is_process_alive(pid));
    }

    #[test]
    fn is_process_alive_for_nonexistent_pid() {
        assert!(!is_process_alive(4_000_000));
    }
}
