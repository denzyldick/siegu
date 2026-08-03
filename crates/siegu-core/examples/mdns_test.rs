use siegu_core::mdns;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let daemon = mdns::create_daemon()?;
    println!("Daemon created, registering _siegu._tcp on port 9876");

    mdns::register_service(&daemon, "siegu-mdns-test", 9876)?;
    println!("Registered. Running for 60 seconds...");
    println!("Check from Android now!");

    std::thread::sleep(std::time::Duration::from_secs(60));
    daemon.shutdown();
    println!("Done");
    Ok(())
}
