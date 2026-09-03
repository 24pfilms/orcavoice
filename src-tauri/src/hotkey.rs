use crate::error::AppError;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, Size};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const HOTKEY_DOWN_EVENT: &str = "orcavoice://hotkey-down";
pub const HOTKEY_UP_EVENT: &str = "orcavoice://hotkey-up";

/// True between a physical key-down and its matching key-up. Windows repeats
/// `WM_HOTKEY` while held, so both edges must be paired and repeats suppressed.
static HOTKEY_DOWN: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HotkeyEdge {
    Down,
    Up,
}

pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), AppError> {
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    reset_hotkey_latch();
    shortcuts
        .register(hotkey)
        .map_err(|e| AppError::Hotkey(format!("Cannot register hotkey '{hotkey}': {e}")))
}

/// Collapse repeated native events into one paired down/up edge sequence.
fn hotkey_edge(latch: &AtomicBool, state: ShortcutState) -> Option<HotkeyEdge> {
    match state {
        ShortcutState::Pressed if !latch.swap(true, Ordering::SeqCst) => Some(HotkeyEdge::Down),
        ShortcutState::Released if latch.swap(false, Ordering::SeqCst) => Some(HotkeyEdge::Up),
        _ => None,
    }
}

pub fn emit_hotkey_event(app: &AppHandle, state: ShortcutState) {
    match hotkey_edge(&HOTKEY_DOWN, state) {
        Some(HotkeyEdge::Down) => {
            show_compact_overlay(app);
            let _ = app.emit(HOTKEY_DOWN_EVENT, ());
        }
        Some(HotkeyEdge::Up) => {
            let _ = app.emit(HOTKEY_UP_EVENT, ());
        }
        None => {}
    }
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
    fn repeated_presses_emit_one_down_edge() {
        let latch = AtomicBool::new(false);
        assert_eq!(
            hotkey_edge(&latch, ShortcutState::Pressed),
            Some(HotkeyEdge::Down)
        );
        for _ in 0..20 {
            assert_eq!(hotkey_edge(&latch, ShortcutState::Pressed), None);
        }
    }

    #[test]
    fn release_is_emitted_only_after_a_leading_press() {
        let latch = AtomicBool::new(false);
        assert_eq!(hotkey_edge(&latch, ShortcutState::Released), None);
        hotkey_edge(&latch, ShortcutState::Pressed);
        assert_eq!(
            hotkey_edge(&latch, ShortcutState::Released),
            Some(HotkeyEdge::Up)
        );
        assert_eq!(hotkey_edge(&latch, ShortcutState::Released), None);
    }

    #[test]
    fn reregistration_drops_an_in_flight_pair() {
        let latch = AtomicBool::new(false);
        hotkey_edge(&latch, ShortcutState::Pressed);
        latch.store(false, Ordering::SeqCst);
        assert_eq!(hotkey_edge(&latch, ShortcutState::Released), None);
        assert_eq!(
            hotkey_edge(&latch, ShortcutState::Pressed),
            Some(HotkeyEdge::Down)
        );
    }

    #[test]
    fn toggle_mode_can_consume_only_down_edges() {
        let latch = AtomicBool::new(false);
        let states = [
            ShortcutState::Pressed,
            ShortcutState::Released,
            ShortcutState::Pressed,
            ShortcutState::Released,
        ];
        let down_count = states
            .into_iter()
            .filter_map(|state| hotkey_edge(&latch, state))
            .filter(|edge| *edge == HotkeyEdge::Down)
            .count();
        assert_eq!(down_count, 2);
    }

    #[test]
    fn push_to_talk_mode_receives_paired_edges() {
        let latch = AtomicBool::new(false);
        let edges = [ShortcutState::Pressed, ShortcutState::Released]
            .into_iter()
            .filter_map(|state| hotkey_edge(&latch, state))
            .collect::<Vec<_>>();
        assert_eq!(edges, vec![HotkeyEdge::Down, HotkeyEdge::Up]);
    }
}
