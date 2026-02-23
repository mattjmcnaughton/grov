use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroveState {
    pub grove_id: String,
    #[serde(default)]
    pub services: HashMap<String, ServiceState>,
}

impl GroveState {
    pub fn new(grove_id: String) -> Self {
        Self {
            grove_id,
            services: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceState {
    pub service_name: String,
    pub port: u16,
    pub handle: ServiceHandleState,
    pub backend_type: String,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServiceHandleState {
    Docker { container_id: String },
    Native { pid: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> GroveState {
        let mut services = HashMap::new();
        services.insert(
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
        services.insert(
            "minio".to_string(),
            ServiceState {
                service_name: "minio".to_string(),
                port: 9001,
                handle: ServiceHandleState::Native { pid: 12345 },
                backend_type: "native".to_string(),
                started_at: "2026-02-06T10:30:01Z".to_string(),
            },
        );
        GroveState {
            grove_id: "a1b2c3d4e5f6g7h8".to_string(),
            services,
        }
    }

    #[test]
    fn serialization_roundtrip() {
        let state = sample_state();
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: GroveState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn forward_compatibility_unknown_fields_ignored() {
        let json = r#"{
            "grove_id": "abc123",
            "services": {},
            "unknown_field": "should be ignored",
            "another_unknown": 42
        }"#;
        let state: GroveState = serde_json::from_str(json).unwrap();
        assert_eq!(state.grove_id, "abc123");
        assert!(state.services.is_empty());
    }

    #[test]
    fn empty_state_roundtrip() {
        let state = GroveState::new("testgrove".to_string());
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: GroveState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
        assert!(deserialized.services.is_empty());
    }

    #[test]
    fn missing_services_field_defaults_to_empty() {
        let json = r#"{"grove_id": "abc123"}"#;
        let state: GroveState = serde_json::from_str(json).unwrap();
        assert_eq!(state.grove_id, "abc123");
        assert!(state.services.is_empty());
    }

    #[test]
    fn docker_handle_serialization() {
        let handle = ServiceHandleState::Docker {
            container_id: "abc123def456".to_string(),
        };
        let json = serde_json::to_string(&handle).unwrap();
        assert!(json.contains("Docker"));
        assert!(json.contains("abc123def456"));
        let deserialized: ServiceHandleState = serde_json::from_str(&json).unwrap();
        assert_eq!(handle, deserialized);
    }

    #[test]
    fn native_handle_serialization() {
        let handle = ServiceHandleState::Native { pid: 99999 };
        let json = serde_json::to_string(&handle).unwrap();
        assert!(json.contains("Native"));
        assert!(json.contains("99999"));
        let deserialized: ServiceHandleState = serde_json::from_str(&json).unwrap();
        assert_eq!(handle, deserialized);
    }
}
