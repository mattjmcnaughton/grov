use std::collections::HashMap;
use std::path::PathBuf;

pub struct NativeInitStep {
    pub command: String,
    pub args: Vec<String>,
}

pub struct ServiceDefinition {
    pub name: String,
    pub docker_image: String,
    pub docker_cmd: Option<Vec<String>>,
    pub process_env: HashMap<String, String>,
    pub native_binary: Option<String>,
    pub native_args_fn: Option<fn(&ResolvedService) -> Vec<String>>,
    pub native_init_fn: Option<fn(&ResolvedService) -> Option<NativeInitStep>>,
    pub default_port: u16,
    pub docker_data_mount: String,
    pub env_template: HashMap<String, String>,
    pub defaults: HashMap<String, String>,
}

pub struct ResolvedService {
    pub definition: ServiceDefinition,
    pub grove_id: String,
    pub allocated_port: u16,
    pub data_dir: PathBuf,
    pub resolved_env: HashMap<String, String>,
}

fn postgres_native_args(resolved: &ResolvedService) -> Vec<String> {
    let data_dir = resolved.data_dir.to_string_lossy();
    vec![
        "-D".to_string(),
        data_dir.to_string(),
        "-l".to_string(),
        format!("{data_dir}/logfile"),
        "-o".to_string(),
        format!("-p {} -k /tmp", resolved.allocated_port),
        "start".to_string(),
    ]
}

fn postgres_native_init(resolved: &ResolvedService) -> Option<NativeInitStep> {
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

fn minio_native_args(resolved: &ResolvedService) -> Vec<String> {
    vec![
        "server".to_string(),
        resolved.data_dir.to_string_lossy().to_string(),
        "--address".to_string(),
        format!(":{}", resolved.allocated_port),
    ]
}

fn postgres_definition() -> ServiceDefinition {
    ServiceDefinition {
        name: "postgres".to_string(),
        docker_image: "postgres:16-alpine".to_string(),
        docker_cmd: None,
        process_env: HashMap::from([
            ("POSTGRES_USER".to_string(), "dev".to_string()),
            ("POSTGRES_PASSWORD".to_string(), "dev".to_string()),
            ("POSTGRES_DB".to_string(), "myapp_dev".to_string()),
        ]),
        native_binary: Some("pg_ctl".to_string()),
        native_args_fn: Some(postgres_native_args),
        native_init_fn: Some(postgres_native_init),
        default_port: 5432,
        docker_data_mount: "/var/lib/postgresql/data".to_string(),
        env_template: HashMap::from([
            (
                "DATABASE_URL".to_string(),
                "postgresql://{username}:{password}@localhost:{port}/{database}".to_string(),
            ),
            ("PGHOST".to_string(), "localhost".to_string()),
            ("PGPORT".to_string(), "{port}".to_string()),
            ("PGUSER".to_string(), "{username}".to_string()),
            ("PGPASSWORD".to_string(), "{password}".to_string()),
            ("PGDATABASE".to_string(), "{database}".to_string()),
        ]),
        defaults: HashMap::from([
            ("username".to_string(), "dev".to_string()),
            ("password".to_string(), "dev".to_string()),
            ("database".to_string(), "myapp_dev".to_string()),
        ]),
    }
}

fn minio_definition() -> ServiceDefinition {
    ServiceDefinition {
        name: "minio".to_string(),
        docker_image: "minio/minio:latest".to_string(),
        docker_cmd: Some(vec!["server".to_string(), "/data".to_string()]),
        process_env: HashMap::from([
            ("MINIO_ROOT_USER".to_string(), "minioadmin".to_string()),
            ("MINIO_ROOT_PASSWORD".to_string(), "minioadmin".to_string()),
        ]),
        native_binary: Some("minio".to_string()),
        native_args_fn: Some(minio_native_args),
        native_init_fn: None,
        default_port: 9000,
        docker_data_mount: "/data".to_string(),
        env_template: HashMap::from([
            (
                "MINIO_ENDPOINT".to_string(),
                "http://localhost:{port}".to_string(),
            ),
            ("AWS_ACCESS_KEY_ID".to_string(), "minioadmin".to_string()),
            (
                "AWS_SECRET_ACCESS_KEY".to_string(),
                "minioadmin".to_string(),
            ),
        ]),
        defaults: HashMap::new(),
    }
}

pub fn builtin_services() -> Vec<ServiceDefinition> {
    vec![postgres_definition(), minio_definition()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_returns_two_services() {
        let services = builtin_services();
        assert_eq!(services.len(), 2);
        assert_eq!(services[0].name, "postgres");
        assert_eq!(services[1].name, "minio");
    }

    #[test]
    fn postgres_definition_correctness() {
        let services = builtin_services();
        let pg = &services[0];
        assert_eq!(pg.docker_image, "postgres:16-alpine");
        assert_eq!(pg.default_port, 5432);
        assert!(pg.env_template.contains_key("DATABASE_URL"));
        assert!(pg.env_template.contains_key("PGHOST"));
        assert!(pg.env_template.contains_key("PGPORT"));
        assert!(pg.env_template.contains_key("PGUSER"));
        assert!(pg.env_template.contains_key("PGPASSWORD"));
        assert!(pg.env_template.contains_key("PGDATABASE"));
        assert_eq!(pg.native_binary.as_deref(), Some("pg_ctl"));
        assert!(pg.native_args_fn.is_some());
        assert!(pg.native_init_fn.is_some());
    }

    #[test]
    fn minio_definition_correctness() {
        let services = builtin_services();
        let minio = &services[1];
        assert_eq!(minio.docker_image, "minio/minio:latest");
        assert_eq!(minio.default_port, 9000);
        assert!(minio.env_template.contains_key("MINIO_ENDPOINT"));
        assert!(minio.env_template.contains_key("AWS_ACCESS_KEY_ID"));
        assert!(minio.env_template.contains_key("AWS_SECRET_ACCESS_KEY"));
        assert_eq!(minio.native_binary.as_deref(), Some("minio"));
        assert!(minio.native_args_fn.is_some());
        assert!(minio.native_init_fn.is_none());
    }

    #[test]
    fn postgres_env_template_has_placeholders() {
        let services = builtin_services();
        let pg = &services[0];
        let db_url = &pg.env_template["DATABASE_URL"];
        assert!(db_url.contains("{username}"));
        assert!(db_url.contains("{password}"));
        assert!(db_url.contains("{port}"));
        assert!(db_url.contains("{database}"));
    }

    #[test]
    fn postgres_defaults_contain_credentials() {
        let services = builtin_services();
        let pg = &services[0];
        assert_eq!(pg.defaults["username"], "dev");
        assert_eq!(pg.defaults["password"], "dev");
        assert_eq!(pg.defaults["database"], "myapp_dev");
    }

    #[test]
    fn minio_docker_cmd_includes_server() {
        let services = builtin_services();
        let minio = &services[1];
        let cmd = minio.docker_cmd.as_ref().unwrap();
        assert!(cmd.contains(&"server".to_string()));
    }
}
