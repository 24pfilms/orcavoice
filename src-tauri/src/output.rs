use crate::desktop_context::DesktopContext;
use crate::error::AppError;
use tauri::AppHandle;

#[cfg(not(target_os = "windows"))]
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
#[cfg(not(target_os = "windows"))]
use std::{thread, time::Duration};
#[cfg(not(target_os = "windows"))]
use tauri_plugin_clipboard_manager::ClipboardExt;

pub fn paste_text(app: &AppHandle, text: &str, context: &DesktopContext) -> Result<(), AppError> {
    if text.trim().is_empty() {
        return Err(AppError::Output("Cannot paste empty text.".to_string()));
    }
    context.restore_target()?;

    #[cfg(target_os = "windows")]
    {
        let _ = app;
        send_unicode_text(text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        paste_with_clipboard(app, text)
    }
}

#[cfg(target_os = "windows")]
const INPUTS_PER_CHUNK: usize = 128;

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UnicodeEvent {
    code_unit: u16,
    key_up: bool,
}

#[cfg(target_os = "windows")]
fn unicode_events(text: &str) -> Vec<UnicodeEvent> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .chars()
        .flat_map(|character| {
            let character = if character == '\n' { '\r' } else { character };
            let mut encoded = [0; 2];
            let units = character.encode_utf16(&mut encoded);
            units.to_vec()
        })
        .flat_map(|code_unit| {
            [
                UnicodeEvent {
                    code_unit,
                    key_up: false,
                },
                UnicodeEvent {
                    code_unit,
                    key_up: true,
                },
            ]
        })
        .collect()
}

#[cfg(target_os = "windows")]
fn send_unicode_text(text: &str) -> Result<(), AppError> {
    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    };

    let events = unicode_events(text);
    for chunk in events.chunks(INPUTS_PER_CHUNK) {
        let inputs = chunk
            .iter()
            .map(|event| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wScan: event.code_unit,
                        dwFlags: if event.key_up {
                            KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                        } else {
                            KEYEVENTF_UNICODE
                        },
                        ..Default::default()
                    },
                },
            })
            .collect::<Vec<_>>();
        let inserted = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) } as usize;
        if inserted != inputs.len() {
            return Err(AppError::Output(format!(
                "Windows inserted {inserted} of {} text-input events; the target may be elevated.",
                inputs.len()
            )));
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn paste_with_clipboard(app: &AppHandle, text: &str) -> Result<(), AppError> {
    let saved = app.clipboard().read_text().ok();
    app.clipboard()
        .write_text(text.to_string())
        .map_err(|e| AppError::Output(format!("Cannot write transcript to clipboard: {e}")))?;

    thread::sleep(Duration::from_millis(80));
    simulate_paste()?;
    thread::sleep(Duration::from_millis(150));

    if let Some(original) = saved {
        let _ = app.clipboard().write_text(original);
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
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

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn unicode_events_emit_down_and_up_for_surrogate_pairs() {
        let events = unicode_events("A🦀");
        let units = "A🦀".encode_utf16().collect::<Vec<_>>();
        assert_eq!(events.len(), units.len() * 2);
        for (pair, unit) in events.chunks_exact(2).zip(units) {
            assert_eq!(
                pair[0],
                UnicodeEvent {
                    code_unit: unit,
                    key_up: false
                }
            );
            assert_eq!(
                pair[1],
                UnicodeEvent {
                    code_unit: unit,
                    key_up: true
                }
            );
        }
    }

    #[test]
    fn line_endings_are_normalized_to_carriage_return_events() {
        let events = unicode_events("a\r\nb\rc\nd");
        let pressed = events
            .iter()
            .filter(|event| !event.key_up)
            .map(|event| event.code_unit)
            .collect::<Vec<_>>();
        assert_eq!(pressed, "a\rb\rc\rd".encode_utf16().collect::<Vec<_>>());
    }

    #[test]
    fn chunks_never_split_a_key_down_from_its_key_up() {
        let events = unicode_events(&"x".repeat(INPUTS_PER_CHUNK));
        for chunk in events.chunks(INPUTS_PER_CHUNK) {
            assert_eq!(chunk.len() % 2, 0);
            assert!(!chunk.first().unwrap().key_up);
            assert!(chunk.last().unwrap().key_up);
        }
    }
}
