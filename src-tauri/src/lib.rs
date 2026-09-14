mod commands;
mod hotkey_dispatch;
mod instances;
mod settings_io;
mod state;
mod tray;
mod views;
mod watcher;
mod window_layout;

use std::sync::Mutex;

use tauri::{Manager, WindowEvent};

use dofustabs_platform::{BackendSet, HotkeyId, PlatformError};

use state::{AppState, PlatformHandles, SharedState};

#[cfg(target_os = "linux")]
fn create_backend() -> Result<BackendSet, PlatformError> {
    dofustabs_platform::linux_backend()
}

#[cfg(windows)]
fn create_backend() -> Result<BackendSet, PlatformError> {
    dofustabs_platform::windows_backend()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn create_backend() -> Result<BackendSet, PlatformError> {
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
            commands::move_up,
            commands::move_down,
            commands::set_primary,
            commands::get_settings,
            commands::set_global_hotkey,
            commands::set_instance_hotkey,
            commands::set_show_names,
            commands::set_hotkeys_suspended,
            commands::set_manage_game_windows,
            commands::arrange_windows,
            commands::set_sidebar_extension,
            commands::set_launcher_path,
            commands::launch_game,
            commands::layout_constants,
        ])
        .setup(|app| {
            let settings_path = match app.path().app_config_dir() {
                Ok(dir) => dir.join("settings.json"),
                Err(err) => {
                    log::error!("no app config dir ({err}); storing settings in temp dir");
                    std::env::temp_dir().join("dofustabs").join("settings.json")
                }
            };

            let home_dir = match app.path().home_dir() {
                Ok(dir) => dir,
                Err(err) => {
                    log::error!("no home dir ({err}); default launcher path will be relative");
                    std::path::PathBuf::new()
                }
            };

            let settings = settings_io::load(&settings_path);
            app.manage(SharedState(Mutex::new(AppState::new(
                settings,
                settings_path,
                home_dir,
            ))));
            app.manage(PlatformHandles::default());

            tray::setup(app.handle())?;

            match create_backend() {
                Ok(BackendSet {
                    discovery,
                    activator,
                    mut hotkeys,
                    window_manager,
                    gpu,
                    launcher,
                }) => {
                    hotkeys.set_handler(hotkey_dispatch::make_handler(app.handle().clone()));

                    {
                        let state = app.state::<SharedState>();
                        let s = state.lock();
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
                    *handles.discovery.lock().unwrap() = Some(discovery);
                    *handles.activator.lock().unwrap() = Some(activator);
                    *handles.hotkeys.lock().unwrap() = Some(hotkeys);
                    *handles.window_manager.lock().unwrap() = Some(window_manager);
                    *handles.gpu.lock().unwrap() = Some(gpu);
                    *handles.launcher.lock().unwrap() = Some(launcher);

                    // Best effort: the own window may not be mapped yet, in
                    // which case the watcher retries on later scans.
                    window_layout::ensure_prepared(app.handle());

                    watcher::start(app.handle().clone());
                }
                Err(err) => {
                    // Degraded mode: the UI still works, but nothing can be
                    // discovered or activated. Never a startup failure.
                    log::warn!("platform backend unavailable, running degraded: {err}");
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window hides it to the tray; the tray's Quit
            // entry is the only way to really exit.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(err) = window.hide() {
                    log::warn!("failed to hide main window: {err}");
                } else {
                    // The window unmaps while hidden and gets a new address
                    // once re-shown from the tray; release the reserved
                    // column and forget the stale own-window reference so
                    // the watcher re-prepares it from scratch.
                    window_layout::suspend(window.app_handle());
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                // Release the reserved sidebar column before the hotkey
                // backend teardown below, so the monitor is back to its full
                // width even if cleanup gets interrupted after this point.
                window_layout::release(app_handle);
                // Tauri terminates via process::exit, which skips Drop for
                // managed state — release the hotkey backend explicitly so
                // its Hyprland binds and FIFO get cleaned up.
                let handles = app_handle.state::<PlatformHandles>();
                drop(handles.hotkeys.lock().unwrap().take());
            }
        });
}
