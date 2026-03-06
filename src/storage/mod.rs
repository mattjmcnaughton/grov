pub mod state;

use std::fs;
use std::path::PathBuf;

use fs2::FileExt;

use state::GroveState;

const GROV_DIR: &str = ".grov";
const STORE_DIR: &str = "store";
const DATA_DIR: &str = "data";
const STATE_FILE: &str = "state.json";
const STATE_TMP_FILE: &str = "state.json.tmp";
const STATE_LOCK_FILE: &str = "state.lock";

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("failed to acquire lock: {0}")]
    Lock(String),
}

pub struct StateManager {
    store_path: PathBuf,
    grove_id: String,
    worktree_path: String,
}

impl StateManager {
    pub fn new(grove_id: &str, worktree_path: &str) -> Result<Self, StorageError> {
        let base = directories::BaseDirs::new().ok_or_else(|| {
            StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not determine home directory",
            ))
        })?;
        let store_path = base
            .home_dir()
            .join(GROV_DIR)
            .join(STORE_DIR)
            .join(grove_id);
        Ok(Self {
            store_path,
            grove_id: grove_id.to_string(),
            worktree_path: worktree_path.to_string(),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_path(store_path: PathBuf, grove_id: &str) -> Result<Self, StorageError> {
        Ok(Self {
            store_path,
            grove_id: grove_id.to_string(),
            worktree_path: "/test/worktree".to_string(),
        })
    }

    fn ensure_store_dir(&self) -> Result<(), StorageError> {
        fs::create_dir_all(&self.store_path)?;
        Ok(())
    }

    pub fn load_state(&self) -> Result<GroveState, StorageError> {
        let state_file = self.store_path.join(STATE_FILE);
        if !state_file.exists() {
            return Ok(GroveState::new(
                self.grove_id.clone(),
                self.worktree_path.clone(),
            ));
        }
        let contents = fs::read_to_string(&state_file)?;
        let state = serde_json::from_str(&contents)?;
        Ok(state)
    }

    pub fn save_state(&self, state: &GroveState) -> Result<(), StorageError> {
        self.ensure_store_dir()?;
        let state_file = self.store_path.join(STATE_FILE);
        let tmp_file = self.store_path.join(STATE_TMP_FILE);
        let contents = serde_json::to_string_pretty(state)?;
        fs::write(&tmp_file, contents)?;
        fs::rename(&tmp_file, &state_file)?;
        Ok(())
    }

    pub fn with_lock<F, T>(&self, f: F) -> Result<T, StorageError>
    where
        F: FnOnce(&mut GroveState) -> Result<T, StorageError>,
    {
        self.ensure_store_dir()?;
        let lock_path = self.store_path.join(STATE_LOCK_FILE);
        let lock_file = fs::File::create(&lock_path)?;
        lock_file
            .lock_exclusive()
            .map_err(|e| StorageError::Lock(e.to_string()))?;
        let mut state = self.load_state()?;
        let result = f(&mut state)?;
        self.save_state(&state)?;
        lock_file
            .unlock()
            .map_err(|e| StorageError::Lock(e.to_string()))?;
        Ok(result)
    }

    pub fn data_dir(&self, service_name: &str) -> PathBuf {
        self.store_path.join(DATA_DIR).join(service_name)
    }

    pub fn ensure_data_dir(&self, service_name: &str) -> Result<PathBuf, StorageError> {
        self.ensure_store_dir()?;
        let path = self.data_dir(service_name);
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub fn store_path(&self) -> &PathBuf {
        &self.store_path
    }

    /// Remove the entire grove store directory (state + data).
    /// Returns Ok(()) even if the directory does not exist.
    pub fn remove_grove(&self) -> Result<(), StorageError> {
        match fs::remove_dir_all(&self.store_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StorageError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::{ServiceHandleState, ServiceState};

    #[test]
    fn io_error_converts_to_storage_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let storage_err: StorageError = io_err.into();
        assert!(matches!(storage_err, StorageError::Io(_)));
        assert_eq!(storage_err.to_string(), "I/O error: file not found");
    }

    #[test]
    fn lock_error_message() {
        let err = StorageError::Lock("resource busy".to_string());
        assert_eq!(err.to_string(), "failed to acquire lock: resource busy");
    }

    #[test]
    fn construction_does_not_create_store_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("new-grove");
        let mgr = StateManager::with_path(store.clone(), "test-grove").unwrap();
        assert!(
            !store.exists(),
            "store dir should not be created on construction"
        );
        // load_state (read-only) should also not create it
        let state = mgr.load_state().unwrap();
        assert!(
            !store.exists(),
            "store dir should not be created by load_state"
        );
        assert!(state.services.is_empty());
    }

    #[test]
    fn load_state_returns_empty_when_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = StateManager::with_path(tmp.path().to_path_buf(), "test-grove").unwrap();
        let state = mgr.load_state().unwrap();
        assert_eq!(state.grove_id, "test-grove");
        assert!(state.services.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = StateManager::with_path(tmp.path().to_path_buf(), "test-grove").unwrap();
        let mut state = GroveState::new("test-grove".to_string(), "/test/worktree".to_string());
        state.services.insert(
            "postgres".to_string(),
            ServiceState {
                service_name: "postgres".to_string(),
                port: 54321,
                handle: ServiceHandleState::Docker {
                    container_id: "abc123".to_string(),
                },
                backend_type: "docker".to_string(),
                started_at: "2026-02-06T10:30:00Z".to_string(),
            },
        );
        mgr.save_state(&state).unwrap();
        let loaded = mgr.load_state().unwrap();
        assert_eq!(state, loaded);
    }

    #[test]
    fn atomic_write_produces_state_file() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = StateManager::with_path(tmp.path().to_path_buf(), "test-grove").unwrap();
        let state = GroveState::new("test-grove".to_string(), "/test/worktree".to_string());
        mgr.save_state(&state).unwrap();
        assert!(tmp.path().join(STATE_FILE).exists());
        // Temp file should not remain
        assert!(!tmp.path().join(STATE_TMP_FILE).exists());
    }

    #[test]
    fn with_lock_read_modify_write() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = StateManager::with_path(tmp.path().to_path_buf(), "test-grove").unwrap();
        mgr.with_lock(|state| {
            state.services.insert(
                "postgres".to_string(),
                ServiceState {
                    service_name: "postgres".to_string(),
                    port: 54321,
                    handle: ServiceHandleState::Docker {
                        container_id: "abc123".to_string(),
                    },
                    backend_type: "docker".to_string(),
                    started_at: "2026-02-06T10:30:00Z".to_string(),
                },
            );
            Ok(())
        })
        .unwrap();
        let loaded = mgr.load_state().unwrap();
        assert_eq!(loaded.services.len(), 1);
        assert!(loaded.services.contains_key("postgres"));
    }

    #[test]
    fn concurrent_lock_no_lost_updates() {
        let tmp = tempfile::tempdir().unwrap();
        let store_path = tmp.path().to_path_buf();

        let mut handles = vec![];
        for i in 0..4 {
            let path = store_path.clone();
            let handle = std::thread::spawn(move || {
                let mgr = StateManager::with_path(path, "test-grove").unwrap();
                mgr.with_lock(|state| {
                    let svc_name = format!("service-{}", i);
                    state.services.insert(
                        svc_name.clone(),
                        ServiceState {
                            service_name: svc_name,
                            port: 5000 + i,
                            handle: ServiceHandleState::Docker {
                                container_id: format!("container-{}", i),
                            },
                            backend_type: "docker".to_string(),
                            started_at: "2026-02-06T10:30:00Z".to_string(),
                        },
                    );
                    Ok(())
                })
                .unwrap();
            });
            handles.push(handle);
        }
        for h in handles {
            h.join().unwrap();
        }

        let mgr = StateManager::with_path(store_path, "test-grove").unwrap();
        let state = mgr.load_state().unwrap();
        assert_eq!(state.services.len(), 4);
        for i in 0..4u16 {
            assert!(state.services.contains_key(&format!("service-{}", i)));
        }
    }

    #[test]
    fn data_dir_returns_correct_path() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = StateManager::with_path(tmp.path().to_path_buf(), "test-grove").unwrap();
        let path = mgr.data_dir("postgres");
        assert_eq!(path, tmp.path().join(DATA_DIR).join("postgres"));
    }

    #[test]
    fn remove_grove_deletes_store_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = StateManager::with_path(tmp.path().to_path_buf(), "test-grove").unwrap();
        mgr.ensure_data_dir("postgres").unwrap();
        assert!(tmp.path().exists());
        mgr.remove_grove().unwrap();
        assert!(!tmp.path().exists());
    }

    #[test]
    fn remove_grove_succeeds_when_directory_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("nonexistent");
        // Manually construct — directory doesn't exist, but that's fine for this test
        let mgr = StateManager {
            store_path: store.clone(),
            grove_id: "test-grove".to_string(),
            worktree_path: "/test/worktree".to_string(),
        };
        // Should not error even though the directory doesn't exist
        mgr.remove_grove().unwrap();
    }

    #[test]
    fn ensure_data_dir_creates_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = StateManager::with_path(tmp.path().to_path_buf(), "test-grove").unwrap();
        let path = mgr.ensure_data_dir("postgres").unwrap();
        assert!(path.exists());
        assert!(path.is_dir());
        assert_eq!(path, tmp.path().join(DATA_DIR).join("postgres"));
    }
}
