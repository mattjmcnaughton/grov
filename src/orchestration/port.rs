use anyhow::{Context, Result};
use std::net::TcpListener;

/// Allocate an available port by binding to port 0.
///
/// The OS assigns an ephemeral port. We record the port number,
/// drop the listener (releasing the port), and return it. There is
/// a TOCTOU race between release and the service binding to the port,
/// but this is acceptable for local development tooling.
pub fn allocate() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("failed to bind to ephemeral port")?;
    let port = listener
        .local_addr()
        .context("failed to get local address")?
        .port();
    drop(listener);
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocated_port_in_valid_range() {
        let port = allocate().unwrap();
        assert!(port >= 1024, "port {port} should be >= 1024");
    }

    #[test]
    fn two_allocations_return_different_ports() {
        let port1 = allocate().unwrap();
        let port2 = allocate().unwrap();
        assert_ne!(port1, port2, "sequential allocations should differ");
    }

    #[test]
    fn port_is_released_after_allocation() {
        let port = allocate().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let listener = TcpListener::bind(format!("127.0.0.1:{port}"));
        assert!(
            listener.is_ok(),
            "should be able to bind to port {port} after allocation released it"
        );
    }
}
