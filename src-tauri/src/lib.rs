mod audio;
mod ducking;
mod error;
mod feedback;
mod history;
mod hotkey;
mod output;
mod platform;
mod secrets;
mod settings;
mod storage;
mod stt;
mod tray;

use audio::{InputLevel, MicrophoneDevice, RecorderState, RecordingSummary};
use history::HistoryEntry;
use serde::{Deserialize, Serialize};
use settings::{AppSettings, SpeechProvider};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use stt::TranscriptionResult;
use tauri::{utils::config::Color, AppHandle, LogicalSize, Manager, PhysicalPosition, Position, Size, State};

static HAS_POSITIONED_OVERLAY: AtomicBool = AtomicBool::new(false);
/// Where the compact bubble sat before the settings panel pushed it up the
/// screen, so collapsing puts it back exactly where the user left it.
static COMPACT_ANCHOR: Mutex<Option<(i32, i32)>> = Mutex::new(None);

const COMPACT_WIDTH: f64 = 310.0;
const COMPACT_HEIGHT: f64 = 60.0;
const SETTINGS_WIDTH: f64 = 396.0;
/// Tall enough for the full settings list including the Groq key row. The panel
/// scrolls internally, so a short screen clamps this instead of clipping.
const SETTINGS_HEIGHT: f64 = 660.0;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TranscriptionOperation {
    /// `None` when the transcript could not be written to history. Recording
    /// history is a convenience, never a reason to fail a dictation.
    pub entry: Option<HistoryEntry>,
    pub result: TranscriptionResult,
    pub recording: RecordingSummary,
    pub pasted: bool,
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    settings::load_settings(&app).map_err(String::from)
}

#[tauri::command]
fn save_settings(app: AppHandle, new_settings: AppSettings) -> Result<AppSettings, String> {
    settings::save_settings(&app, &new_settings).map_err(String::from)?;
    hotkey::register_hotkey(&app, &new_settings.hotkey).map_err(String::from)?;
    Ok(new_settings)
}

#[tauri::command]
fn get_secret_status(app: AppHandle) -> secrets::SecretStatus {
    let settings = settings::load_settings(&app).unwrap_or_default();
    secrets::secret_status_with_fallback(&settings.groq_api_key)
}

#[tauri::command]
fn set_api_key(app: AppHandle, provider: SpeechProvider, api_key: String) -> Result<secrets::SecretStatus, String> {
    // Store in OS keyring (best-effort)
    let _ = secrets::set_api_key(provider, api_key.clone());
    // Also persist in settings.json so it survives cargo recompiles in dev
    let mut current = settings::load_settings(&app).unwrap_or_default();
    match provider {
        settings::SpeechProvider::Groq => current.groq_api_key = api_key,
    }
    settings::save_settings(&app, &current).map_err(String::from)?;
    Ok(secrets::secret_status_with_fallback(&current.groq_api_key))
}

#[tauri::command]
fn clear_api_key(app: AppHandle, provider: SpeechProvider) -> Result<secrets::SecretStatus, String> {
    let _ = secrets::clear_api_key(provider);
    let mut current = settings::load_settings(&app).unwrap_or_default();
    match provider {
        settings::SpeechProvider::Groq => current.groq_api_key = String::new(),
    }
    settings::save_settings(&app, &current).map_err(String::from)?;
    Ok(secrets::secret_status_with_fallback(&current.groq_api_key))
}

#[tauri::command]
fn list_microphones() -> Result<Vec<MicrophoneDevice>, String> {
    audio::list_input_devices().map_err(String::from)
}

#[tauri::command]
fn is_recording(recorder: State<RecorderState>) -> bool {
    audio::is_recording(&recorder)
}

#[tauri::command]
fn duck_audio() {
    ducking::duck_audio();
}

#[tauri::command]
fn unduck_audio() {
    ducking::unduck_audio();
}

#[tauri::command]
fn restore_audio() {
    ducking::restore_audio();
}

#[tauri::command]
fn play_feedback_tone(kind: String) {
    match kind.as_str() {
        "stop" => feedback::play_stop_tone(),
        _ => feedback::play_start_tone(),
    }
}

#[tauri::command]
fn start_recording(app: AppHandle, recorder: State<RecorderState>) -> Result<(), String> {
    let preferred = settings::load_settings(&app)
        .map(|settings| settings.input_device)
        .unwrap_or_default();
    audio::start_recording(&recorder, Some(preferred.as_str())).map_err(String::from)
}

#[tauri::command]
fn get_input_level(recorder: State<RecorderState>) -> InputLevel {
    audio::input_level(&recorder)
}

/// Windows silently feeds blocked apps an all-zero capture, so the only fix is
/// a settings page the user has to visit. Take them straight there.
#[tauri::command]
fn open_microphone_settings() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", "ms-settings:privacy-microphone"])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Cannot open Windows microphone settings: {e}"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Microphone settings can only be opened on Windows.".to_string())
    }
}

#[tauri::command]
fn cancel_recording(recorder: State<RecorderState>) {
    audio::cancel_recording(&recorder);
}

#[tauri::command]
async fn stop_and_transcribe(
    app: AppHandle,
    recorder: State<'_, RecorderState>,
) -> Result<TranscriptionOperation, String> {
    let captured = audio::stop_recording(&recorder).map_err(String::from)?;
    let settings = settings::load_settings(&app).map_err(String::from)?;
    let result = stt::transcribe_active_provider(&settings, &captured)
        .await
        .map_err(String::from)?;
    let pasted = if settings.auto_paste {
        output::paste_text(&app, &result.text).map_err(String::from)?;
        true
    } else {
        false
    };
    // Best-effort: the user already has their text. A history failure must not
    // turn a successful dictation into a red error state on screen.
    let entry = match history::append_history(&app, result.clone()) {
        Ok(entry) => Some(entry),
        Err(error) => {
            eprintln!("OrcaVoice could not append to history: {error}");
            None
        }
    };
    Ok(TranscriptionOperation {
        entry,
        result,
        recording: captured.summary,
        pasted,
    })
}

#[tauri::command]
fn paste_text(app: AppHandle, text: String) -> Result<(), String> {
    output::paste_text(&app, &text).map_err(String::from)
}

#[tauri::command]
fn get_history(app: AppHandle) -> Result<Vec<HistoryEntry>, String> {
    history::load_history(&app).map_err(String::from)
}

#[tauri::command]
fn clear_history(app: AppHandle) -> Result<(), String> {
    history::clear_history(&app).map_err(String::from)
}

#[tauri::command]
fn get_platform_info() -> platform::PlatformInfo {
    platform::platform_info()
}

#[tauri::command]
fn show_compact_overlay(app: AppHandle) -> Result<(), String> {
    set_main_window(&app, COMPACT_WIDTH, COMPACT_HEIGHT, false)
}

#[tauri::command]
fn show_settings_overlay(app: AppHandle) -> Result<(), String> {
    set_main_window(&app, SETTINGS_WIDTH, SETTINGS_HEIGHT, true)
}

#[tauri::command]
fn hide_overlay(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Cannot find main window".to_string())?;
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
fn start_overlay_drag(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Cannot find main window".to_string())?;
    window.start_dragging().map_err(|e| e.to_string())
}

fn set_main_window(app: &AppHandle, width: f64, height: f64, focus: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Cannot find main window".to_string())?;
    #[cfg(target_os = "windows")]
    window.set_shadow(false).map_err(|e| e.to_string())?;

    let expanding = height > COMPACT_HEIGHT;
    // The bubble lives just above the taskbar, so growing downwards ran the
    // settings panel straight off the bottom of the screen and clipped the
    // Groq key row. Remember where the bubble was, then pull it back on screen.
    let anchor_before = window.outer_position().ok().map(|p| (p.x, p.y));
    let height = if expanding {
        fit_height_to_screen(&window, height)
    } else {
        height
    };

    window
        .set_size(Size::Logical(LogicalSize::new(width, height)))
        .map_err(|e| e.to_string())?;
    window
        .set_background_color(Some(Color(0, 0, 0, 0)))
        .map_err(|e| e.to_string())?;

    if !HAS_POSITIONED_OVERLAY.swap(true, Ordering::Relaxed) {
        position_near_taskbar(&window, width, height)?;
    } else if expanding {
        let mut anchor = COMPACT_ANCHOR.lock().map_err(|e| e.to_string())?;
        if anchor.is_none() {
            *anchor = anchor_before;
        }
        drop(anchor);
        clamp_into_work_area(&window, width, height)?;
    } else if let Some((x, y)) = COMPACT_ANCHOR.lock().map_err(|e| e.to_string())?.take() {
        window
            .set_position(Position::Physical(PhysicalPosition::new(x, y)))
            .map_err(|e| e.to_string())?;
    }

    window.set_focusable(focus).map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    if focus {
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Never ask for a window taller than the usable desktop; the panel scrolls.
fn fit_height_to_screen(window: &tauri::WebviewWindow, height: f64) -> f64 {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return height;
    };
    let scale = monitor.scale_factor();
    let available = monitor.work_area().size.height as f64 / scale - 16.0;
    height.min(available.max(COMPACT_HEIGHT))
}

/// Shift the window so its whole height stays inside the monitor work area.
fn clamp_into_work_area(window: &tauri::WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    let Some(monitor) = window.current_monitor().map_err(|e| e.to_string())? else {
        return Ok(());
    };
    let position = window.outer_position().map_err(|e| e.to_string())?;
    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();
    let width_px = (width * scale).round() as i32;
    let height_px = (height * scale).round() as i32;
    let margin_px = (8.0 * scale).round() as i32;

    let min_x = work_area.position.x + margin_px;
    let max_x = work_area.position.x + work_area.size.width as i32 - width_px - margin_px;
    let min_y = work_area.position.y + margin_px;
    let max_y = work_area.position.y + work_area.size.height as i32 - height_px - margin_px;
    let x = position.x.clamp(min_x.min(max_x), max_x.max(min_x));
    let y = position.y.clamp(min_y.min(max_y), max_y.max(min_y));
    if (x, y) == (position.x, position.y) {
        return Ok(());
    }
    window
        .set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|e| e.to_string())
}

fn position_near_taskbar(window: &tauri::WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    let Some(monitor) = window.current_monitor().map_err(|e| e.to_string())? else {
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();
    let width_px = (width * scale).round() as i32;
    let height_px = (height * scale).round() as i32;
    let margin_px = (30.0 * scale).round() as i32;
    let x = work_area.position.x + ((work_area.size.width as i32 - width_px) / 2).max(0);
    let y = work_area.position.y + work_area.size.height as i32 - height_px - margin_px;
    window
        .set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|e| e.to_string())
}

/// Re-assert the OS login entry on every launch so a reinstall, a moved
/// executable, or a wiped profile cannot silently stop OrcaVoice from starting
/// with Windows. Never fatal: a locked registry must not block dictation.
fn sync_autostart(app: &AppHandle, wanted: bool) {
    use tauri_plugin_autostart::ManagerExt;
    // The plugin registers whichever executable is running, so doing this in a
    // dev build would repoint the user's login entry at target\debug and leave
    // it dangling after a `cargo clean`. Only the installed build may claim it.
    if cfg!(debug_assertions) {
        return;
    }
    let manager = app.autolaunch();
    let current = manager.is_enabled().unwrap_or(false);
    if current == wanted {
        return;
    }
    let outcome = if wanted {
        manager.enable()
    } else {
        manager.disable()
    };
    if let Err(error) = outcome {
        eprintln!("OrcaVoice could not set start-on-login to {wanted}: {error}");
    }
}

pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin: a second launch (autostart + manual start)
        // would otherwise fail to claim the global hotkey and leave a dead,
        // invisible process behind. Instead we surface the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Err(error) = show_compact_overlay(app.clone()) {
                eprintln!("OrcaVoice could not surface the running instance: {error}");
            }
        }))
        .manage(RecorderState::default())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let app_handle = app.handle().clone();
            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(|app, _shortcut, event| {
                        hotkey::emit_hotkey_if_pressed(app, event.state());
                    })
                    .build(),
            )?;

            ducking::restore_audio();

            let settings = settings::load_settings(&app_handle)?;
            if let Err(error) = hotkey::register_hotkey(&app_handle, &settings.hotkey) {
                eprintln!("OrcaVoice hotkey registration failed: {error}");
            }
            if let Err(error) = tray::setup_tray(&app_handle) {
                eprintln!("OrcaVoice tray setup failed: {error}");
            }
            sync_autostart(&app_handle, settings.start_on_login);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_secret_status,
            set_api_key,
            clear_api_key,
            list_microphones,
            get_input_level,
            open_microphone_settings,
            is_recording,
            duck_audio,
            unduck_audio,
            restore_audio,
            play_feedback_tone,
            start_recording,
            cancel_recording,
            stop_and_transcribe,
            paste_text,
            get_history,
            clear_history,
            get_platform_info,
            show_compact_overlay,
            show_settings_overlay,
            hide_overlay,
            start_overlay_drag
        ])
        .run(tauri::generate_context!())
        .expect("error while running OrcaVoice");
}
