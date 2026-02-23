use super::Service;
use crate::orchestration::service::ResolvedService;
use std::collections::HashMap;

pub struct Minio;

impl Service for Minio {
    fn name(&self) -> &str {
        "minio"
    }

    fn docker_image(&self) -> &str {
        "minio/minio:latest"
    }

    fn docker_cmd(&self) -> Option<Vec<String>> {
        Some(vec!["server".to_string(), "/data".to_string()])
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
        HashMap::from([
            (
                "MINIO_ENDPOINT".to_string(),
                "http://localhost:{port}".to_string(),
            ),
            ("AWS_ACCESS_KEY_ID".to_string(), "minioadmin".to_string()),
            (
                "AWS_SECRET_ACCESS_KEY".to_string(),
                "minioadmin".to_string(),
            ),
        ])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minio_definition_correctness() {
        let m = Minio;
        assert_eq!(m.name(), "minio");
        assert_eq!(m.docker_image(), "minio/minio:latest");
        assert_eq!(m.default_port(), 9000);
        assert_eq!(m.docker_data_mount(), "/data");
    }

    #[test]
    fn minio_docker_cmd_includes_server() {
        let m = Minio;
        let cmd = m.docker_cmd().unwrap();
        assert!(cmd.contains(&"server".to_string()));
    }

    #[test]
    fn minio_env_template_keys() {
        let m = Minio;
        let tmpl = m.env_template();
        assert!(tmpl.contains_key("MINIO_ENDPOINT"));
        assert!(tmpl.contains_key("AWS_ACCESS_KEY_ID"));
        assert!(tmpl.contains_key("AWS_SECRET_ACCESS_KEY"));
    }

    #[test]
    fn minio_native_binary() {
        let m = Minio;
        assert_eq!(m.native_binary(), Some("minio"));
    }

    #[test]
    fn minio_has_no_native_init() {
        let m = Minio;
        let resolved = ResolvedService {
            grove_id: "test".to_string(),
            allocated_port: 9000,
            data_dir: "/tmp/test".into(),
            resolved_env: HashMap::new(),
        };
        assert!(m.native_init(&resolved).is_none());
    }
}
