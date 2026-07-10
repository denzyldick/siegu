#![allow(dead_code)]

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::time::Duration;

const SERVICE_TYPE: &str = "_siegu._tcp.local.";

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveredHost {
    pub name: String,
    pub ip: String,
    pub port: u16,
}

/// Creates a new mDNS daemon.
pub fn create_daemon() -> Result<ServiceDaemon, Box<dyn std::error::Error>> {
    Ok(ServiceDaemon::new()?)
}

/// Registers a Siegu service on the local network via mDNS.
/// Other Siegu devices on the same LAN will discover this host.
pub fn register_service(
    daemon: &ServiceDaemon,
    hostname: &str,
    port: u16,
    room_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let properties: &[(&str, &str)] = &[("room", room_id)];
    let service_info = ServiceInfo::new(
        SERVICE_TYPE,
        hostname,
        &format!("{}.local.", hostname),
        "",
        port,
        properties,
    )?;
    daemon.register(service_info)?;
    Ok(())
}

/// Discovers Siegu hosts on the local network.
/// Scans for the given duration and returns all discovered hosts.
pub fn discover_hosts(
    daemon: &ServiceDaemon,
    timeout_secs: u64,
) -> Result<Vec<DiscoveredHost>, Box<dyn std::error::Error>> {
    let receiver = daemon.browse(SERVICE_TYPE)?;
    let mut hosts = HashMap::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);

    while std::time::Instant::now() < deadline {
        if let Ok(event) = receiver.recv_timeout(Duration::from_millis(200)) {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    for addr in info.get_addresses().iter() {
                        let key = format!("{}:{}", addr, info.get_port());
                        hosts.entry(key).or_insert_with(|| DiscoveredHost {
                            name: info.get_hostname().trim_end_matches('.').to_string(),
                            ip: addr.to_string(),
                            port: info.get_port(),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    Ok(hosts.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that registering a service returns Ok.
    /// This validates the API compiles and basic registration works.
    #[test]
    fn test_register_service_ok() {
        let daemon = ServiceDaemon::new().expect("Failed to create mDNS daemon");
        let result = register_service(&daemon, "siegu-test-reg", 9876, "test-room");
        assert!(result.is_ok(), "Should register successfully: {result:?}");
    }

    /// Test that discover_hosts doesn't panic or error.
    /// This validates the API is callable without registration.
    #[test]
    fn test_discover_hosts_no_crash() {
        let daemon = ServiceDaemon::new().expect("Failed to create mDNS daemon");
        let result = discover_hosts(&daemon, 1);
        assert!(result.is_ok(), "Should not error: {result:?}");
    }
}
