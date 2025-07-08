//! Port allocation utilities for tests
//!
//! This module provides utilities for dynamically allocating unique ports
//! to prevent conflicts when tests run in parallel.

use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Global port counter to prevent port conflicts in parallel tests
/// Starts at 50000 to avoid conflicts with system ports and common development ports
static PORT_COUNTER: AtomicU16 = AtomicU16::new(50000);

/// Helper function to get a unique port for testing
///
/// This function uses an atomic counter to ensure each test gets a unique port,
/// preventing conflicts when tests run in parallel.
pub fn get_unique_test_port() -> u16 {
    PORT_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Helper function to get a unique test address with IP 127.0.0.1
///
/// Returns a formatted string like "127.0.0.1:50001"
pub fn get_unique_test_address() -> String {
    let port = get_unique_test_port();
    format!("127.0.0.1:{}", port)
}

/// Helper function to get a unique unreachable address for testing failures
///
/// This is useful for testing connection timeouts and error handling.
/// Returns a formatted string like "127.0.0.1:50002"
pub fn get_unreachable_address() -> String {
    get_unique_test_address()
}

/// Helper function to create multiple unique test addresses
///
/// Returns a vector of unique addresses for tests that need multiple servers
pub fn get_multiple_test_addresses(count: usize) -> Vec<String> {
    (0..count).map(|_| get_unique_test_address()).collect()
}

/// Get a unique port with an offset for specific port ranges
///
/// This is useful when you need ports in a specific range for testing
pub fn get_unique_test_port_with_offset(offset: u16) -> u16 {
    let base_port = get_unique_test_port();
    base_port.saturating_add(offset)
}

/// Create a unique temporary directory name using timestamp and port
///
/// This helps prevent conflicts in temporary directory creation during parallel tests
pub fn get_unique_temp_suffix() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let port = get_unique_test_port();
    format!("test_{}_{}", timestamp, port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::thread;

    #[test]
    fn test_unique_port_generation() {
        let ports: Vec<u16> = (0..100).map(|_| get_unique_test_port()).collect();
        let unique_ports: HashSet<_> = ports.iter().collect();

        assert_eq!(
            ports.len(),
            unique_ports.len(),
            "All generated ports should be unique"
        );

        // All ports should be >= 50000
        assert!(
            ports.iter().all(|&p| p >= 50000),
            "All ports should be >= 50000"
        );
    }

    #[test]
    fn test_concurrent_port_generation() {
        let thread_count = 10;
        let ports_per_thread = 10;
        let all_ports = Arc::new(Mutex::new(Vec::new()));
        let mut handles = vec![];

        // Spawn threads that generate ports concurrently
        for _ in 0..thread_count {
            let ports_clone = Arc::clone(&all_ports);
            let handle = thread::spawn(move || {
                let mut thread_ports = Vec::new();
                for _ in 0..ports_per_thread {
                    thread_ports.push(get_unique_test_port());
                }

                let mut all_ports = ports_clone.lock().unwrap();
                all_ports.extend(thread_ports);
            });
            handles.push(handle);
        }

        // Wait for completion
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }

        // Verify all ports are unique
        let all_ports = all_ports.lock().unwrap();
        let unique_ports: HashSet<_> = all_ports.iter().collect();

        assert_eq!(
            unique_ports.len(),
            all_ports.len(),
            "All concurrently generated ports should be unique"
        );
        assert_eq!(
            all_ports.len(),
            thread_count * ports_per_thread,
            "Should have generated expected number of ports"
        );
    }

    #[test]
    fn test_address_generation() {
        let addr1 = get_unique_test_address();
        let addr2 = get_unique_test_address();

        assert_ne!(addr1, addr2, "Generated addresses should be different");
        assert!(
            addr1.starts_with("127.0.0.1:"),
            "Address should start with 127.0.0.1:"
        );
        assert!(
            addr2.starts_with("127.0.0.1:"),
            "Address should start with 127.0.0.1:"
        );
    }

    #[test]
    fn test_multiple_addresses() {
        let addresses = get_multiple_test_addresses(5);

        assert_eq!(
            addresses.len(),
            5,
            "Should generate requested number of addresses"
        );

        let unique_addresses: HashSet<_> = addresses.iter().collect();
        assert_eq!(
            addresses.len(),
            unique_addresses.len(),
            "All addresses should be unique"
        );

        for addr in &addresses {
            assert!(
                addr.starts_with("127.0.0.1:"),
                "All addresses should start with 127.0.0.1:"
            );
        }
    }

    #[test]
    fn test_temp_suffix_uniqueness() {
        let suffix1 = get_unique_temp_suffix();
        let suffix2 = get_unique_temp_suffix();

        assert_ne!(suffix1, suffix2, "Temp suffixes should be unique");
        assert!(
            suffix1.starts_with("test_"),
            "Suffix should start with 'test_'"
        );
        assert!(
            suffix2.starts_with("test_"),
            "Suffix should start with 'test_'"
        );
    }
}
