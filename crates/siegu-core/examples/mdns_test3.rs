use siegu_core::mdns;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let daemon = mdns::create_daemon()?;
    println!("[1] Created daemon");

    mdns::register_service(&daemon, "siegu-test-single", 9998)?;
    println!("[1] Registered _siegu._tcp on port 9998");

    println!("[2] Browsing for _siegu._tcp...");
    let hosts = mdns::discover_hosts(&daemon, 5)?;

    if hosts.is_empty() {
        println!("[2] No services found");
    } else {
        for h in &hosts {
            println!("[2] FOUND! {} at {}:{}", h.name, h.ip, h.port);
        }
    }

    daemon.shutdown();
    println!("Done");
    Ok(())
}
