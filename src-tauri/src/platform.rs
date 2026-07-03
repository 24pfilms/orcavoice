use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub clipboard_paste_supported: bool,
    pub keyboard_fallback_supported: bool,
    pub notes: Vec<String>,
}

pub fn platform_info() -> PlatformInfo {
    let os = std::env::consts::OS.to_string();
    let mut notes = Vec::new();
    if os == "linux" {
        notes.push("Wayland may restrict global hotkeys and synthetic input. Clipboard output remains the safest path.".to_string());
    }
    if os == "macos" {
        notes.push("macOS may require Accessibility/Input Monitoring permission for paste simulation.".to_string());
    }
    PlatformInfo {
        os,
        clipboard_paste_supported: true,
        keyboard_fallback_supported: true,
        notes,
    }
}
