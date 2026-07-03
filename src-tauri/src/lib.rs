mod audio;
mod error;
mod history;
mod hotkey;
mod output;
mod platform;
mod secrets;
mod settings;
mod stt;
mod tray;

use audio::{MicrophoneDevice, RecorderState, RecordingSummary};
use history::HistoryEntry;
use serde::{Deserialize, Serialize};
use settings::{AppSettings, SpeechProvider};
use std::sync::atomic::{AtomicBool, Ordering};
use stt::{BenchmarkResult, TranscriptionResult};
use tauri::{utils::config::Color, AppHandle, LogicalSize, Manager, PhysicalPosition, Position, Size, State};

static HAS_POSITIONED_OVERLAY: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TranscriptionOperation {
    pub entry: HistoryEntry,
    pub result: TranscriptionResult,
    pub recording: RecordingSummary,
    pub pasted: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BenchmarkOperation {
    pub recording: RecordingSummary,
    pub benchmark: BenchmarkResult,
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
    secrets::secret_status_with_fallback(&settings.groq_api_key, &settings.openai_api_key)
}

#[tauri::command]
fn set_api_key(app: AppHandle, provider: SpeechProvider, api_key: String) -> Result<secrets::SecretStatus, String> {
    // Store in OS keyring (best-effort)
    let _ = secrets::set_api_key(provider, api_key.clone());
    // Also persist in settings.json so it survives cargo recompiles in dev
    let mut current = settings::load_settings(&app).unwrap_or_default();
    match provider {
        settings::SpeechProvider::Groq => current.groq_api_key = api_key,
        settings::SpeechProvider::OpenAi => current.openai_api_key = api_key,
    }
    settings::save_settings(&app, &current).map_err(String::from)?;
    Ok(secrets::secret_status_with_fallback(&current.groq_api_key, &current.openai_api_key))
}

#[tauri::command]
fn clear_api_key(app: AppHandle, provider: SpeechProvider) -> Result<secrets::SecretStatus, String> {
    let _ = secrets::clear_api_key(provider);
    let mut current = settings::load_settings(&app).unwrap_or_default();
    match provider {
        settings::SpeechProvider::Groq => current.groq_api_key = String::new(),
        settings::SpeechProvider::OpenAi => current.openai_api_key = String::new(),
    }
    settings::save_settings(&app, &current).map_err(String::from)?;
    Ok(secrets::secret_status_with_fallback(&current.groq_api_key, &current.openai_api_key))
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
fn start_recording(recorder: State<RecorderState>) -> Result<(), String> {
    audio::start_recording(&recorder).map_err(String::from)
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
    let entry = history::append_history(&app, result.clone()).map_err(String::from)?;
    Ok(TranscriptionOperation {
        entry,
        result,
        recording: captured.summary,
        pasted,
    })
}

#[tauri::command]
async fn stop_and_benchmark(
    app: AppHandle,
    recorder: State<'_, RecorderState>,
) -> Result<BenchmarkOperation, String> {
    let captured = audio::stop_recording(&recorder).map_err(String::from)?;
    let settings = settings::load_settings(&app).map_err(String::from)?;
    let benchmark = stt::benchmark_providers(&settings, &captured).await;
    Ok(BenchmarkOperation {
        recording: captured.summary,
        benchmark,
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
    set_main_window(&app, 310.0, 60.0, false)
}

#[tauri::command]
fn show_settings_overlay(app: AppHandle) -> Result<(), String> {
    set_main_window(&app, 366.0, 460.0, true)
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
    window
        .set_size(Size::Logical(LogicalSize::new(width, height)))
        .map_err(|e| e.to_string())?;
    window
        .set_background_color(Some(Color(0, 0, 0, 0)))
        .map_err(|e| e.to_string())?;
    if !HAS_POSITIONED_OVERLAY.swap(true, Ordering::Relaxed) {
        position_near_taskbar(&window, width, height)?;
    }
    window.set_focusable(focus).map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    if focus {
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
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

pub fn run() {
    tauri::Builder::default()
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

            let settings = settings::load_settings(&app_handle)?;
            if let Err(error) = hotkey::register_hotkey(&app_handle, &settings.hotkey) {
                eprintln!("OrcaVoice hotkey registration failed: {error}");
            }
            if let Err(error) = tray::setup_tray(&app_handle) {
                eprintln!("OrcaVoice tray setup failed: {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_secret_status,
            set_api_key,
            clear_api_key,
            list_microphones,
            is_recording,
            start_recording,
            cancel_recording,
            stop_and_transcribe,
            stop_and_benchmark,
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
