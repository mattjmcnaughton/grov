pub mod state;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("failed to acquire lock: {0}")]
    Lock(String),
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
