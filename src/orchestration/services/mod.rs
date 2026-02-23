mod minio;
mod postgres;

use super::service::{NativeInitStep, ResolvedService};
use std::collections::HashMap;

pub trait Service: Send + Sync {
    fn name(&self) -> &str;
    fn docker_image(&self) -> &str;
    fn docker_cmd(&self) -> Option<Vec<String>> {
        None
    }
    fn process_env(&self) -> HashMap<String, String>;
    fn default_port(&self) -> u16;
    fn docker_data_mount(&self) -> &str;
    fn env_template(&self) -> HashMap<String, String>;
    fn defaults(&self) -> HashMap<String, String>;

    fn native_binary(&self) -> Option<&str> {
        None
    }
    fn native_args(&self, _resolved: &ResolvedService) -> Vec<String> {
        vec![]
    }
    fn native_init(&self, _resolved: &ResolvedService) -> Option<NativeInitStep> {
        None
    }
}

pub fn builtin_services() -> Vec<Box<dyn Service>> {
    vec![Box::new(postgres::Postgres), Box::new(minio::Minio)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_returns_two_services() {
        let services = builtin_services();
        assert_eq!(services.len(), 2);
        assert_eq!(services[0].name(), "postgres");
        assert_eq!(services[1].name(), "minio");
    }
}
