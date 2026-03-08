use std::collections::HashMap;
use std::path::Path;

use tracing::warn;

use crate::orchestration::services::Service;

// --- Compose file structures ---

#[derive(Debug, serde::Deserialize)]
pub struct ComposeFile {
    #[serde(default)]
    pub services: HashMap<String, ComposeServiceDef>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ComposeServiceDef {
    pub image: Option<String>,
    #[serde(default)]
    pub environment: ComposeEnvironment,
    #[serde(default)]
    pub ports: Vec<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(untagged)]
pub enum ComposeEnvironment {
    Map(HashMap<String, serde_yaml::Value>),
    List(Vec<String>),
    #[default]
    Empty,
}

impl ComposeEnvironment {
    pub fn to_map(&self) -> HashMap<String, String> {
        match self {
            ComposeEnvironment::Map(m) => m
                .iter()
                .map(|(k, v)| {
                    let val = match v {
                        serde_yaml::Value::String(s) => s.clone(),
                        serde_yaml::Value::Number(n) => n.to_string(),
                        serde_yaml::Value::Bool(b) => b.to_string(),
                        serde_yaml::Value::Null => String::new(),
                        other => format!("{other:?}"),
                    };
                    (k.clone(), val)
                })
                .collect(),
            ComposeEnvironment::List(list) => list
                .iter()
                .filter_map(|entry| {
                    let (k, v) = entry.split_once('=')?;
                    Some((k.to_string(), v.to_string()))
                })
                .collect(),
            ComposeEnvironment::Empty => HashMap::new(),
        }
    }
}

// --- Compose file discovery and parsing ---

static COMPOSE_FILENAMES: &[&str] = &[
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
];

pub fn find_compose_file(dir: &Path) -> Option<std::path::PathBuf> {
    for name in COMPOSE_FILENAMES {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

pub fn parse_file(path: &Path) -> Result<ComposeFile, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    serde_yaml::from_str(&content).map_err(|e| format!("failed to parse {}: {e}", path.display()))
}

// --- Image matching ---

#[derive(Debug, Clone, Copy, PartialEq)]
enum KnownServiceKind {
    Postgres,
    Minio,
}

fn match_image(image: &str) -> Option<KnownServiceKind> {
    let name = image.split(':').next().unwrap_or(image);
    match name {
        "postgres" => Some(KnownServiceKind::Postgres),
        "minio/minio" => Some(KnownServiceKind::Minio),
        _ => None,
    }
}

// --- ComposeService: implements Service from compose definitions ---

pub struct ComposeService {
    svc_name: String,
    image: String,
    docker_cmd: Option<Vec<String>>,
    process_env: HashMap<String, String>,
    default_port: u16,
    docker_data_mount: String,
    env_template: HashMap<String, String>,
    defaults: HashMap<String, String>,
}

impl Service for ComposeService {
    fn name(&self) -> &str {
        &self.svc_name
    }

    fn docker_image(&self) -> &str {
        &self.image
    }

    fn docker_cmd(&self) -> Option<Vec<String>> {
        self.docker_cmd.clone()
    }

    fn process_env(&self) -> HashMap<String, String> {
        self.process_env.clone()
    }

    fn default_port(&self) -> u16 {
        self.default_port
    }

    fn docker_data_mount(&self) -> &str {
        &self.docker_data_mount
    }

    fn env_template(&self) -> HashMap<String, String> {
        self.env_template.clone()
    }

    fn defaults(&self) -> HashMap<String, String> {
        self.defaults.clone()
    }
}

fn build_postgres_compose_service(
    name: String,
    image: String,
    compose_env: &HashMap<String, String>,
) -> ComposeService {
    let username = compose_env
        .get("POSTGRES_USER")
        .cloned()
        .unwrap_or_else(|| "dev".to_string());
    let password = compose_env
        .get("POSTGRES_PASSWORD")
        .cloned()
        .unwrap_or_else(|| "dev".to_string());
    let database = compose_env
        .get("POSTGRES_DB")
        .cloned()
        .unwrap_or_else(|| "myapp_dev".to_string());

    let mut process_env = HashMap::from([
        ("POSTGRES_USER".to_string(), username.clone()),
        ("POSTGRES_PASSWORD".to_string(), password.clone()),
        ("POSTGRES_DB".to_string(), database.clone()),
    ]);
    // Include any additional env vars from compose that aren't the standard ones
    for (k, v) in compose_env {
        process_env.entry(k.clone()).or_insert_with(|| v.clone());
    }

    ComposeService {
        svc_name: name,
        image,
        docker_cmd: None,
        process_env,
        default_port: 5432,
        docker_data_mount: "/var/lib/postgresql/data".to_string(),
        env_template: HashMap::from([
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
        ]),
        defaults: HashMap::from([
            ("username".to_string(), username),
            ("password".to_string(), password),
            ("database".to_string(), database),
        ]),
    }
}

fn build_minio_compose_service(
    name: String,
    image: String,
    compose_env: &HashMap<String, String>,
) -> ComposeService {
    let root_user = compose_env
        .get("MINIO_ROOT_USER")
        .cloned()
        .unwrap_or_else(|| "minioadmin".to_string());
    let root_password = compose_env
        .get("MINIO_ROOT_PASSWORD")
        .cloned()
        .unwrap_or_else(|| "minioadmin".to_string());

    let mut process_env = HashMap::from([
        ("MINIO_ROOT_USER".to_string(), root_user.clone()),
        ("MINIO_ROOT_PASSWORD".to_string(), root_password.clone()),
    ]);
    for (k, v) in compose_env {
        process_env.entry(k.clone()).or_insert_with(|| v.clone());
    }

    ComposeService {
        svc_name: name,
        image,
        docker_cmd: Some(vec!["server".to_string(), "/data".to_string()]),
        process_env,
        default_port: 9000,
        docker_data_mount: "/data".to_string(),
        env_template: HashMap::from([
            (
                "MINIO_ENDPOINT".to_string(),
                "http://localhost:{{ port }}".to_string(),
            ),
            ("AWS_ACCESS_KEY_ID".to_string(), root_user),
            ("AWS_SECRET_ACCESS_KEY".to_string(), root_password),
        ]),
        defaults: HashMap::new(),
    }
}

/// Resolve compose service definitions into grov Service objects.
/// Returns the services that could be matched and a list of warning messages
/// for services that were skipped.
pub fn resolve_compose_services(compose: &ComposeFile) -> (Vec<Box<dyn Service>>, Vec<String>) {
    let mut services: Vec<Box<dyn Service>> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let mut names: Vec<&String> = compose.services.keys().collect();
    names.sort();

    for name in names {
        let def = &compose.services[name];
        let image = match &def.image {
            Some(img) => img,
            None => {
                warnings.push(format!(
                    "skipping compose service '{name}': no image specified"
                ));
                continue;
            }
        };

        let kind = match match_image(image) {
            Some(k) => k,
            None => {
                warnings.push(format!(
                    "skipping compose service '{name}': unsupported image '{image}'"
                ));
                continue;
            }
        };

        let compose_env = def.environment.to_map();

        let svc: Box<dyn Service> = match kind {
            KnownServiceKind::Postgres => Box::new(build_postgres_compose_service(
                name.clone(),
                image.clone(),
                &compose_env,
            )),
            KnownServiceKind::Minio => Box::new(build_minio_compose_service(
                name.clone(),
                image.clone(),
                &compose_env,
            )),
        };

        services.push(svc);
    }

    // Emit warnings via tracing
    for w in &warnings {
        warn!("{w}");
    }

    (services, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_compose() {
        let yaml = r#"
services:
  db:
    image: postgres:16-alpine
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(compose.services.len(), 1);
        assert_eq!(
            compose.services["db"].image.as_deref(),
            Some("postgres:16-alpine")
        );
    }

    #[test]
    fn parse_compose_with_env_map() {
        let yaml = r#"
services:
  db:
    image: postgres:16
    environment:
      POSTGRES_USER: myuser
      POSTGRES_PASSWORD: mypass
      POSTGRES_DB: mydb
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let env = compose.services["db"].environment.to_map();
        assert_eq!(env["POSTGRES_USER"], "myuser");
        assert_eq!(env["POSTGRES_PASSWORD"], "mypass");
        assert_eq!(env["POSTGRES_DB"], "mydb");
    }

    #[test]
    fn parse_compose_with_env_list() {
        let yaml = r#"
services:
  db:
    image: postgres:16
    environment:
      - POSTGRES_USER=listuser
      - POSTGRES_PASSWORD=listpass
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let env = compose.services["db"].environment.to_map();
        assert_eq!(env["POSTGRES_USER"], "listuser");
        assert_eq!(env["POSTGRES_PASSWORD"], "listpass");
    }

    #[test]
    fn parse_compose_no_environment() {
        let yaml = r#"
services:
  db:
    image: postgres:16
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let env = compose.services["db"].environment.to_map();
        assert!(env.is_empty());
    }

    #[test]
    fn parse_compose_with_ports() {
        let yaml = r#"
services:
  db:
    image: postgres:16
    ports:
      - "5432:5432"
      - "5433:5432"
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(compose.services["db"].ports.len(), 2);
    }

    #[test]
    fn match_image_postgres_variants() {
        assert_eq!(
            match_image("postgres:16-alpine"),
            Some(KnownServiceKind::Postgres)
        );
        assert_eq!(match_image("postgres:16"), Some(KnownServiceKind::Postgres));
        assert_eq!(match_image("postgres"), Some(KnownServiceKind::Postgres));
    }

    #[test]
    fn match_image_minio_variants() {
        assert_eq!(
            match_image("minio/minio:latest"),
            Some(KnownServiceKind::Minio)
        );
        assert_eq!(match_image("minio/minio"), Some(KnownServiceKind::Minio));
    }

    #[test]
    fn match_image_unknown() {
        assert_eq!(match_image("redis:7"), None);
        assert_eq!(match_image("elasticsearch:8"), None);
        assert_eq!(match_image("nginx:latest"), None);
    }

    #[test]
    fn resolve_postgres_with_defaults() {
        let yaml = r#"
services:
  db:
    image: postgres:16-alpine
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let (services, warnings) = resolve_compose_services(&compose);
        assert!(warnings.is_empty());
        assert_eq!(services.len(), 1);

        let svc = &services[0];
        assert_eq!(svc.name(), "db");
        assert_eq!(svc.docker_image(), "postgres:16-alpine");
        assert_eq!(svc.default_port(), 5432);
        assert_eq!(svc.docker_data_mount(), "/var/lib/postgresql/data");

        let env = svc.process_env();
        assert_eq!(env["POSTGRES_USER"], "dev");
        assert_eq!(env["POSTGRES_PASSWORD"], "dev");
        assert_eq!(env["POSTGRES_DB"], "myapp_dev");

        let defaults = svc.defaults();
        assert_eq!(defaults["username"], "dev");
        assert_eq!(defaults["password"], "dev");
        assert_eq!(defaults["database"], "myapp_dev");

        let tmpl = svc.env_template();
        assert!(tmpl.contains_key("DATABASE_URL"));
        assert!(tmpl.contains_key("PGPORT"));
    }

    #[test]
    fn resolve_postgres_with_custom_env() {
        let yaml = r#"
services:
  mydb:
    image: postgres:16
    environment:
      POSTGRES_USER: admin
      POSTGRES_PASSWORD: secret
      POSTGRES_DB: production
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let (services, warnings) = resolve_compose_services(&compose);
        assert!(warnings.is_empty());
        assert_eq!(services.len(), 1);

        let svc = &services[0];
        assert_eq!(svc.name(), "mydb");

        let env = svc.process_env();
        assert_eq!(env["POSTGRES_USER"], "admin");
        assert_eq!(env["POSTGRES_PASSWORD"], "secret");
        assert_eq!(env["POSTGRES_DB"], "production");

        let defaults = svc.defaults();
        assert_eq!(defaults["username"], "admin");
        assert_eq!(defaults["password"], "secret");
        assert_eq!(defaults["database"], "production");
    }

    #[test]
    fn resolve_minio_with_defaults() {
        let yaml = r#"
services:
  storage:
    image: minio/minio:latest
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let (services, warnings) = resolve_compose_services(&compose);
        assert!(warnings.is_empty());
        assert_eq!(services.len(), 1);

        let svc = &services[0];
        assert_eq!(svc.name(), "storage");
        assert_eq!(svc.docker_image(), "minio/minio:latest");
        assert_eq!(svc.default_port(), 9000);
        assert_eq!(svc.docker_data_mount(), "/data");
        assert!(svc.docker_cmd().is_some());

        let tmpl = svc.env_template();
        assert_eq!(tmpl["AWS_ACCESS_KEY_ID"], "minioadmin");
        assert_eq!(tmpl["AWS_SECRET_ACCESS_KEY"], "minioadmin");
    }

    #[test]
    fn resolve_minio_with_custom_env() {
        let yaml = r#"
services:
  s3:
    image: minio/minio:latest
    environment:
      MINIO_ROOT_USER: myadmin
      MINIO_ROOT_PASSWORD: mysecret
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let (services, warnings) = resolve_compose_services(&compose);
        assert!(warnings.is_empty());

        let svc = &services[0];
        assert_eq!(svc.name(), "s3");

        let env = svc.process_env();
        assert_eq!(env["MINIO_ROOT_USER"], "myadmin");
        assert_eq!(env["MINIO_ROOT_PASSWORD"], "mysecret");

        let tmpl = svc.env_template();
        assert_eq!(tmpl["AWS_ACCESS_KEY_ID"], "myadmin");
        assert_eq!(tmpl["AWS_SECRET_ACCESS_KEY"], "mysecret");
    }

    #[test]
    fn resolve_skips_unsupported_image() {
        let yaml = r#"
services:
  cache:
    image: redis:7
  db:
    image: postgres:16
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let (services, warnings) = resolve_compose_services(&compose);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name(), "db");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("redis:7"));
        assert!(warnings[0].contains("cache"));
    }

    #[test]
    fn resolve_skips_service_without_image() {
        let yaml = r#"
services:
  app:
    build: .
  db:
    image: postgres:16
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let (services, warnings) = resolve_compose_services(&compose);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name(), "db");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("app"));
        assert!(warnings[0].contains("no image"));
    }

    #[test]
    fn resolve_multiple_supported_services() {
        let yaml = r#"
services:
  db:
    image: postgres:16
  storage:
    image: minio/minio:latest
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let (services, warnings) = resolve_compose_services(&compose);
        assert!(warnings.is_empty());
        assert_eq!(services.len(), 2);
        // Sorted by name
        assert_eq!(services[0].name(), "db");
        assert_eq!(services[1].name(), "storage");
    }

    #[test]
    fn resolve_empty_compose() {
        let yaml = r#"
services: {}
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let (services, warnings) = resolve_compose_services(&compose);
        assert!(services.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn find_compose_file_discovers_standard_names() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_compose_file(dir.path()).is_none());

        std::fs::write(dir.path().join("docker-compose.yml"), "").unwrap();
        let found = find_compose_file(dir.path()).unwrap();
        assert!(found.ends_with("docker-compose.yml"));
    }

    #[test]
    fn find_compose_file_prefers_docker_compose_yml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("docker-compose.yml"), "").unwrap();
        std::fs::write(dir.path().join("compose.yml"), "").unwrap();
        let found = find_compose_file(dir.path()).unwrap();
        assert!(found.ends_with("docker-compose.yml"));
    }

    #[test]
    fn find_compose_file_discovers_compose_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("compose.yaml"), "").unwrap();
        let found = find_compose_file(dir.path()).unwrap();
        assert!(found.ends_with("compose.yaml"));
    }

    #[test]
    fn compose_env_numeric_values() {
        let yaml = r#"
services:
  db:
    image: postgres:16
    environment:
      POSTGRES_USER: dev
      PGPORT: 5432
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let env = compose.services["db"].environment.to_map();
        assert_eq!(env["PGPORT"], "5432");
    }

    #[test]
    fn parse_file_returns_error_for_missing() {
        let result = parse_file(Path::new("/nonexistent/docker-compose.yml"));
        assert!(result.is_err());
    }

    #[test]
    fn parse_file_returns_error_for_invalid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("docker-compose.yml");
        std::fs::write(&path, "not: [valid: yaml: {{").unwrap();
        let result = parse_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn parse_file_succeeds_for_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("docker-compose.yml");
        std::fs::write(&path, "services:\n  db:\n    image: postgres:16\n").unwrap();
        let compose = parse_file(&path).unwrap();
        assert_eq!(compose.services.len(), 1);
    }

    #[test]
    fn compose_service_has_no_native_support() {
        let yaml = r#"
services:
  db:
    image: postgres:16
"#;
        let compose: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        let (services, _) = resolve_compose_services(&compose);
        // ComposeService doesn't implement native_binary
        assert!(services[0].native_binary().is_none());
    }
}
