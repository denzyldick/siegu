use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("wallpaper")
        .setup(|_app, api| {
            #[cfg(target_os = "android")]
            api.register_android_plugin("io.denzyl.siegu", "WallpaperPlugin")?;
            #[cfg(not(target_os = "android"))]
            let _ = api;
            Ok(())
        })
        .build()
}
