//! System tray: "Show" / "Quit" menu, left-click restores the main window,
//! and the main window hides to the tray instead of closing (see `lib.rs`).

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

const MAIN_WINDOW: &str = "main";
const MENU_SHOW: &str = "show";
const MENU_QUIT: &str = "quit";

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, MENU_SHOW, "Show", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .tooltip("DofusTabs")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            MENU_SHOW => show_main_window(app),
            // `exit` raises RunEvent::Exit, where the hotkey backend is
            // dropped so its OS binds get cleaned up.
            MENU_QUIT => app.exit(0),
            other => log::debug!("unhandled tray menu item {other}"),
        })
        .on_tray_icon_event(|tray, event| {
            // Linux app indicators only deliver menu events; on other
            // platforms a plain left click restores the window.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    } else {
        log::warn!("no default window icon configured; tray icon will be blank");
    }

    builder.build(app)?;
    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        log::warn!("main window not found");
        return;
    };
    for (what, result) in [
        ("show", window.show()),
        ("unminimize", window.unminimize()),
        ("focus", window.set_focus()),
    ] {
        if let Err(err) = result {
            log::warn!("failed to {what} main window: {err}");
        }
    }
}
