use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// Resolve the grove ID for the current working directory.
///
/// Hashes the absolute path of the cwd using SHA-256 and truncates
/// to 16 hex characters, producing a stable, deterministic identifier.
pub fn resolve() -> Result<String> {
    let cwd = std::env::current_dir().context("failed to determine current directory")?;
    Ok(resolve_path(&cwd))
}

/// Compute the grove ID for a given path.
///
/// The path is converted to its absolute canonical string representation
/// before hashing. The result is always 16 lowercase hex characters.
pub fn resolve_path(path: &std::path::Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("failed to determine current directory")
            .join(path)
    };

    let mut hasher = Sha256::new();
    hasher.update(absolute.to_string_lossy().as_bytes());
    let hash = hasher.finalize();
    hash[..8].iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn deterministic_same_path_same_id() {
        let path = PathBuf::from("/home/user/project");
        let id1 = resolve_path(&path);
        let id2 = resolve_path(&path);
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_paths_different_ids() {
        let id1 = resolve_path(&PathBuf::from("/home/user/project-a"));
        let id2 = resolve_path(&PathBuf::from("/home/user/project-b"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn result_is_16_hex_chars() {
        let id = resolve_path(&PathBuf::from("/any/path"));
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn result_is_lowercase_hex() {
        let id = resolve_path(&PathBuf::from("/some/path/with/UPPERCASE"));
        assert!(
            id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        );
    }

    #[test]
    fn similar_paths_differ() {
        let id1 = resolve_path(&PathBuf::from("/home/user/project"));
        let id2 = resolve_path(&PathBuf::from("/home/user/projects"));
        assert_ne!(id1, id2);
    }
}
