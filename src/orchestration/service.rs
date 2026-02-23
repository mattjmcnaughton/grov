use std::collections::HashMap;
use std::path::PathBuf;

pub struct NativeInitStep {
    pub command: String,
    pub args: Vec<String>,
}

pub struct ResolvedService {
    pub grove_id: String,
    pub allocated_port: u16,
    pub data_dir: PathBuf,
    pub resolved_env: HashMap<String, String>,
}
