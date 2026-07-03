use crate::error::AppError;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, Size};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const HOTKEY_EVENT: &str = "orcavoice://hotkey-toggle";

pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), AppError> {
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    shortcuts
        .register(hotkey)
        .map_err(|e| AppError::Hotkey(format!("Cannot register hotkey '{hotkey}': {e}")))
}

pub fn emit_hotkey_if_pressed(app: &AppHandle, state: ShortcutState) {
    if state == ShortcutState::Pressed {
        show_compact_overlay(app);
        let _ = app.emit(HOTKEY_EVENT, ());
    }
}

fn show_compact_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_size(Size::Logical(LogicalSize::new(310.0, 60.0)));
        let _ = window.set_focusable(false);
        let _ = window.show();
        let _ = window.set_always_on_top(true);
    }
}
