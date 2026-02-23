use std::fmt;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

const DEFAULT_INTERVAL: Duration = Duration::from_millis(250);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

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

pub async fn wait_until_healthy(
    service: &str,
    host: &str,
    port: u16,
    check_timeout: Option<Duration>,
    interval: Option<Duration>,
) -> Result<(), HealthCheckError> {
    let check_timeout = check_timeout.unwrap_or(DEFAULT_TIMEOUT);
    let interval = interval.unwrap_or(DEFAULT_INTERVAL);
    let addr = format!("{}:{}", host, port);

    let poll = async {
        loop {
            if TcpStream::connect(&addr).await.is_ok() {
                return;
            }
            sleep(interval).await;
        }
    };

    timeout(check_timeout, poll)
        .await
        .map_err(|_| HealthCheckError::Timeout {
            service: service.to_string(),
            port,
            elapsed: check_timeout,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[tokio::test]
    async fn healthy_service_detected() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let result = wait_until_healthy(
            "test-svc",
            "127.0.0.1",
            port,
            Some(Duration::from_secs(2)),
            Some(Duration::from_millis(50)),
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn timeout_on_unhealthy() {
        // Use a port that nothing is listening on
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let result = wait_until_healthy(
            "test-svc",
            "127.0.0.1",
            port,
            Some(Duration::from_secs(1)),
            Some(Duration::from_millis(100)),
        )
        .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            HealthCheckError::Timeout {
                service,
                port: p,
                elapsed,
            } => {
                assert_eq!(service, "test-svc");
                assert_eq!(p, port);
                assert_eq!(elapsed, Duration::from_secs(1));
            }
        }
    }

    #[tokio::test]
    async fn delayed_readiness() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        // Spawn a task that starts listening after a delay
        let addr = format!("127.0.0.1:{}", port);
        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(300)).await;
            let _listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
            // Keep listener alive long enough for health check to connect
            sleep(Duration::from_secs(2)).await;
        });

        let result = wait_until_healthy(
            "test-svc",
            "127.0.0.1",
            port,
            Some(Duration::from_secs(3)),
            Some(Duration::from_millis(50)),
        )
        .await;
        assert!(result.is_ok());
        handle.abort();
    }
}
