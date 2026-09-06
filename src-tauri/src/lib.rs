mod commands;
mod hotkey_dispatch;
mod settings_io;
mod state;
mod watcher;

use std::sync::Mutex;

use tauri::Manager;

use dofustabs_platform::{
    GameDiscovery, HotkeyBackend, HotkeyId, PlatformError, WindowActivator,
};

use state::{AppState, PlatformHandles, SharedState};

type Backend = (
    Box<dyn GameDiscovery + Send>,
    Box<dyn WindowActivator + Send>,
    Box<dyn HotkeyBackend + Send>,
);

#[cfg(target_os = "linux")]
fn create_backend() -> Result<Backend, PlatformError> {
    dofustabs_platform::linux_backend()
}

#[cfg(windows)]
fn create_backend() -> Result<Backend, PlatformError> {
    dofustabs_platform::windows_backend()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn create_backend() -> Result<Backend, PlatformError> {
    Err(PlatformError::UnsupportedSession(
        "no platform backend implemented for this OS yet".to_string(),
    ))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::refresh,
            commands::activate,
            commands::set_enabled,
            commands::reorder,
            commands::set_primary,
            commands::get_settings,
            commands::set_global_hotkey,
            commands::set_instance_hotkey,
            commands::set_show_names,
        ])
        .setup(|app| {
            let settings_path = match app.path().app_config_dir() {
                Ok(dir) => dir.join("settings.json"),
                Err(err) => {
                    log::error!("no app config dir ({err}); storing settings in temp dir");
                    std::env::temp_dir().join("dofustabs").join("settings.json")
                }
            };

            let settings = settings_io::load(&settings_path);
            app.manage(SharedState(Mutex::new(AppState::new(settings, settings_path))));
            app.manage(PlatformHandles::default());

            match create_backend() {
                Ok((discovery, activator, mut hotkeys)) => {
                    hotkeys.set_handler(hotkey_dispatch::make_handler(app.handle().clone()));

                    {
                        let state = app.state::<SharedState>();
                        let s = state.0.lock().unwrap();
                        for (id, hotkey) in [
                            (HotkeyId::Next, &s.settings.next_hotkey),
                            (HotkeyId::Previous, &s.settings.previous_hotkey),
                            (HotkeyId::Primary, &s.settings.primary_hotkey),
                        ] {
                            if let Err(err) = hotkeys.register(id, hotkey) {
                                log::warn!("failed to register global hotkey {id:?}: {err}");
                            }
                        }
                    }

                    let handles = app.state::<PlatformHandles>();
                    *handles.activator.lock().unwrap() = Some(activator);
                    *handles.hotkeys.lock().unwrap() = Some(hotkeys);

                    watcher::start(app.handle().clone(), discovery);
                }
                Err(err) => {
                    // Degraded mode: the UI still works, but nothing can be
                    // discovered or activated. Never a startup failure.
                    log::warn!("platform backend unavailable, running degraded: {err}");
                }
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                // Tauri terminates via process::exit, which skips Drop for
                // managed state — release the hotkey backend explicitly so
                // its Hyprland binds and FIFO get cleaned up.
                let handles = app_handle.state::<PlatformHandles>();
                drop(handles.hotkeys.lock().unwrap().take());
            }
        });
}
