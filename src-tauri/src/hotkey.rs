use crate::error::AppError;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, Size};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const HOTKEY_EVENT: &str = "orcavoice://hotkey-toggle";

/// True between a physical key-down and its matching key-up.
///
/// Windows repeats `WM_HOTKEY` for as long as the trigger key is held, and
/// `global-hotkey` turns every repeat into another `Pressed` event. Without
/// edge-triggering, holding `\` for a fraction of a second fires the toggle
/// several times *and* re-shows the overlay after the UI has hidden it, which
/// looks exactly like a switch that never turns off.
static HOTKEY_DOWN: AtomicBool = AtomicBool::new(false);

pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), AppError> {
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    reset_hotkey_latch();
    shortcuts
        .register(hotkey)
        .map_err(|e| AppError::Hotkey(format!("Cannot register hotkey '{hotkey}': {e}")))
}

/// Collapse a stream of `Pressed`/`Released` events into one toggle per
/// physical key press. Returns true only on the leading edge of a press.
///
/// Takes the latch as a parameter so tests can exercise it without touching
/// (and racing on) the process-wide static.
fn is_leading_edge(latch: &AtomicBool, state: ShortcutState) -> bool {
    match state {
        // swap returns the previous value: if it was already true this is a
        // key-repeat, not a new press, so drop it.
        ShortcutState::Pressed => !latch.swap(true, Ordering::SeqCst),
        ShortcutState::Released => {
            latch.store(false, Ordering::SeqCst);
            false
        }
    }
}

pub fn emit_hotkey_if_pressed(app: &AppHandle, state: ShortcutState) {
    if !is_leading_edge(&HOTKEY_DOWN, state) {
        return;
    }
    show_compact_overlay(app);
    let _ = app.emit(HOTKEY_EVENT, ());
}

/// Re-registering the hotkey drops any in-flight key-up, so clear the latch or
/// the next press would be swallowed as a repeat.
pub fn reset_hotkey_latch() {
    HOTKEY_DOWN.store(false, Ordering::SeqCst);
}

fn show_compact_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_size(Size::Logical(LogicalSize::new(310.0, 60.0)));
        let _ = window.set_focusable(false);
        let _ = window.show();
        let _ = window.set_always_on_top(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_key_toggles_once_not_once_per_repeat() {
        let latch = AtomicBool::new(false);
        assert!(is_leading_edge(&latch, ShortcutState::Pressed));
        // Windows repeats WM_HOTKEY for the whole hold; none of these count.
        for _ in 0..20 {
            assert!(!is_leading_edge(&latch, ShortcutState::Pressed));
        }
        assert!(!is_leading_edge(&latch, ShortcutState::Released));
    }

    #[test]
    fn each_new_press_toggles_again() {
        let latch = AtomicBool::new(false);
        for _ in 0..3 {
            assert!(is_leading_edge(&latch, ShortcutState::Pressed));
            assert!(!is_leading_edge(&latch, ShortcutState::Pressed));
            is_leading_edge(&latch, ShortcutState::Released);
        }
    }

    #[test]
    fn a_dropped_key_up_does_not_wedge_the_hotkey() {
        let latch = AtomicBool::new(false);
        assert!(is_leading_edge(&latch, ShortcutState::Pressed));
        // Re-registering the shortcut loses the pending Released event.
        latch.store(false, Ordering::SeqCst);
        assert!(is_leading_edge(&latch, ShortcutState::Pressed));
    }
}
