use crate::error::AppError;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

pub fn setup_tray(app: &AppHandle) -> Result<(), AppError> {
    let show = MenuItem::with_id(
        app,
        "show",
        "Show OrcaVoice Actions Preview",
        true,
        None::<&str>,
    )
        .map_err(|e| AppError::Config(format!("Cannot build tray menu item: {e}")))?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)
        .map_err(|e| AppError::Config(format!("Cannot build tray menu item: {e}")))?;
    let menu = Menu::with_items(app, &[&show, &quit])
        .map_err(|e| AppError::Config(format!("Cannot build tray menu: {e}")))?;

    // Without an explicit icon the tray entry renders blank on Windows, which
    // makes an autostarted (hidden-window) OrcaVoice look like it never launched.
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| AppError::Config("No default window icon available for tray.".to_string()))?;

    TrayIconBuilder::new()
        .icon(icon)
        .tooltip("OrcaVoice Actions Preview")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)
        .map_err(|e| AppError::Config(format!("Cannot create tray icon: {e}")))?;
    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
