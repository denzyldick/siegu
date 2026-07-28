use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
pub use mdns_sd::ServiceDaemon as DaemonHandle;
use std::collections::HashMap;
use std::time::Duration;

use crate::mesh::PROTOCOL_VERSION;

const SERVICE_TYPE: &str = "_siegu._tcp.local.";

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveredHost {
    pub name: String,
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    Added(DiscoveredHost),
    Removed(String),
}

/// Creates a new mDNS daemon.
pub fn create_daemon() -> Result<ServiceDaemon, Box<dyn std::error::Error>> {
    Ok(ServiceDaemon::new()?)
}

/// Registers a Siegu service on the local network via mDNS.
pub fn register_service(
    daemon: &ServiceDaemon,
    hostname: &str,
    port: u16,
    room_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let properties: &[(&str, &str)] = &[
        ("room", room_id),
        ("version", &PROTOCOL_VERSION.to_string()),
    ];
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

/// Unregisters a previously registered service.
pub fn unregister_service(daemon: &ServiceDaemon, hostname: &str) {
    let full_name = format!("{}.{SERVICE_TYPE}", hostname.trim_end_matches('.'));
    daemon.unregister(&full_name).ok();
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
        if let Ok(ServiceEvent::ServiceResolved(info)) =
            receiver.recv_timeout(Duration::from_millis(200))
        {
            for addr in info.get_addresses().iter() {
                let key = format!("{}:{}", addr, info.get_port());
                hosts.entry(key).or_insert_with(|| DiscoveredHost {
                    name: info.get_hostname().trim_end_matches('.').to_string(),
                    ip: addr.to_string(),
                    port: info.get_port(),
                });
            }
        }
    }

    Ok(hosts.into_values().collect())
}

/// Continuously monitors mDNS for hosts being added or removed.
/// Sends events through the returned receiver until the daemon shuts down.
pub fn watch_hosts(
    daemon: &ServiceDaemon,
) -> Result<std::sync::mpsc::Receiver<DiscoveryEvent>, Box<dyn std::error::Error>> {
    let receiver = daemon.browse(SERVICE_TYPE)?;
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        loop {
            match receiver.recv_timeout(Duration::from_secs(1)) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    for addr in info.get_addresses().iter() {
                        let host = DiscoveredHost {
                            name: info.get_hostname().trim_end_matches('.').to_string(),
                            ip: addr.to_string(),
                            port: info.get_port(),
                        };
                        if tx.send(DiscoveryEvent::Added(host)).is_err() {
                            return;
                        }
                    }
                }
                Ok(ServiceEvent::ServiceRemoved(_, full_name)) => {
                    if tx.send(DiscoveryEvent::Removed(full_name)).is_err() {
                        return;
                    }
                }
                Ok(_) => continue,
                Err(_) => continue,
            }
        }
    });

    Ok(rx)
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
