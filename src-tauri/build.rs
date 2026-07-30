fn main() {
    let attrs = tauri_build::Attributes::new()
        .plugin(
            "mdns",
            tauri_build::InlinedPlugin::new()
                .commands(&["ping", "discover"])
                .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
        )
        .plugin(
            "permission",
            tauri_build::InlinedPlugin::new()
                .commands(&["requestCamera"])
                .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
        );
    if let Err(e) = tauri_build::try_build(attrs) {
        eprintln!("{e:#}");
        std::process::exit(1);
    }
}
