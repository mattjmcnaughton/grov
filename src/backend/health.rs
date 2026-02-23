use std::fmt;
use std::time::Duration;

#[derive(Debug)]
pub enum HealthCheckError {
    Timeout {
        service: String,
        port: u16,
        elapsed: Duration,
    },
}

impl fmt::Display for HealthCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthCheckError::Timeout {
                service, elapsed, ..
            } => {
                write!(
                    f,
                    "{} failed to become healthy within {} seconds",
                    service,
                    elapsed.as_secs()
                )
            }
        }
    }
}

impl std::error::Error for HealthCheckError {}
