use crate::error::AppError;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

pub fn paste_text(app: &AppHandle, text: &str) -> Result<(), AppError> {
    if text.trim().is_empty() {
        return Err(AppError::Output("Cannot paste empty text.".to_string()));
    }

    // Snapshot the user's current clipboard so we can restore it after pasting.
    let saved = app
        .clipboard()
        .read_text()
        .ok();

    app.clipboard()
        .write_text(text.to_string())
        .map_err(|e| AppError::Output(format!("Cannot write transcript to clipboard: {e}")))?;

    thread::sleep(Duration::from_millis(80));
    simulate_paste()?;

    // Give the target app time to consume the paste before we clobber the clipboard.
    thread::sleep(Duration::from_millis(150));

    // Restore the user's original clipboard.
    if let Some(original) = saved {
        let _ = app.clipboard().write_text(original);
    }

    Ok(())
}

fn simulate_paste() -> Result<(), AppError> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| AppError::Output(format!("Cannot initialize keyboard simulator: {e}")))?;

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    enigo
        .key(modifier, Direction::Press)
        .map_err(|e| AppError::Output(format!("Cannot press paste modifier: {e}")))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| AppError::Output(format!("Cannot press paste key: {e}")))?;
    enigo
        .key(modifier, Direction::Release)
        .map_err(|e| AppError::Output(format!("Cannot release paste modifier: {e}")))?;
    Ok(())
}
