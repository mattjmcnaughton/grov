use super::Service;
use crate::orchestration::service::{NativeInitStep, ResolvedService};
use std::collections::HashMap;

pub struct Postgres;

impl Service for Postgres {
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
        HashMap::from([
            (
                "DATABASE_URL".to_string(),
                "postgresql://{{ username }}:{{ password }}@localhost:{{ port }}/{{ database }}"
                    .to_string(),
            ),
            ("PGHOST".to_string(), "localhost".to_string()),
            ("PGPORT".to_string(), "{{ port }}".to_string()),
            ("PGUSER".to_string(), "{{ username }}".to_string()),
            ("PGPASSWORD".to_string(), "{{ password }}".to_string()),
            ("PGDATABASE".to_string(), "{{ database }}".to_string()),
        ])
    }

    fn defaults(&self) -> HashMap<String, String> {
        HashMap::from([
            ("username".to_string(), "dev".to_string()),
            ("password".to_string(), "dev".to_string()),
            ("database".to_string(), "myapp_dev".to_string()),
        ])
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

    fn native_init(&self, resolved: &ResolvedService) -> Option<NativeInitStep> {
        if resolved.data_dir.join("PG_VERSION").exists() {
            return None;
        }
        Some(NativeInitStep {
            command: "initdb".to_string(),
            args: vec![
                "-D".to_string(),
                resolved.data_dir.to_string_lossy().to_string(),
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_definition_correctness() {
        let pg = Postgres;
        assert_eq!(pg.name(), "postgres");
        assert_eq!(pg.docker_image(), "postgres:16-alpine");
        assert_eq!(pg.default_port(), 5432);
        assert_eq!(pg.docker_data_mount(), "/var/lib/postgresql/data");
        assert!(pg.docker_cmd().is_none());
    }

    #[test]
    fn postgres_env_template_keys() {
        let pg = Postgres;
        let tmpl = pg.env_template();
        assert!(tmpl.contains_key("DATABASE_URL"));
        assert!(tmpl.contains_key("PGHOST"));
        assert!(tmpl.contains_key("PGPORT"));
        assert!(tmpl.contains_key("PGUSER"));
        assert!(tmpl.contains_key("PGPASSWORD"));
        assert!(tmpl.contains_key("PGDATABASE"));
    }

    #[test]
    fn postgres_env_template_has_placeholders() {
        let pg = Postgres;
        let db_url = &pg.env_template()["DATABASE_URL"];
        assert!(db_url.contains("{{ username }}"));
        assert!(db_url.contains("{{ password }}"));
        assert!(db_url.contains("{{ port }}"));
        assert!(db_url.contains("{{ database }}"));
    }

    #[test]
    fn postgres_defaults_contain_credentials() {
        let pg = Postgres;
        let defaults = pg.defaults();
        assert_eq!(defaults["username"], "dev");
        assert_eq!(defaults["password"], "dev");
        assert_eq!(defaults["database"], "myapp_dev");
    }

    #[test]
    fn postgres_native_binary() {
        let pg = Postgres;
        assert_eq!(pg.native_binary(), Some("postgres"));
    }

    #[test]
    fn postgres_native_args_correctness() {
        let pg = Postgres;
        let resolved = ResolvedService {
            grove_id: "test".to_string(),
            allocated_port: 5432,
            data_dir: "/tmp/pgdata".into(),
            resolved_env: HashMap::new(),
        };
        let args = pg.native_args(&resolved);
        assert_eq!(args, vec!["-D", "/tmp/pgdata", "-p", "5432", "-k", "/tmp"]);
    }
}
