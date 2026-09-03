use crate::error::AppError;
use crate::storage;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SpeechProvider {
    #[default]
    Groq,
}

impl SpeechProvider {
    pub fn key_name(self) -> &'static str {
        match self {
            SpeechProvider::Groq => "groq",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderSettings {
    pub model: String,
    pub language: String,
    pub prompt: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DictationMode {
    Raw,
    Grammar,
    Email,
    TranslateEnglish,
    Custom,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ActivationMode {
    #[default]
    Toggle,
    PushToTalk,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppSettings {
    // Legacy settings files may name a provider OrcaVoice no longer supports;
    // fall back to Groq rather than failing the whole load.
    #[serde(default)]
    pub active_provider: SpeechProvider,
    pub groq: ProviderSettings,
    pub hotkey: String,
    #[serde(default)]
    pub activation_mode: ActivationMode,
    pub mode: DictationMode,
    pub custom_mode_instruction: String,
    pub auto_paste: bool,
    #[serde(default)]
    pub selection_actions_enabled: bool,
    pub retain_audio: bool,
    pub output_mode: String,
    #[serde(default = "default_bubble_outline")]
    pub bubble_outline: String,
    #[serde(default = "default_outline_width")]
    pub outline_width: u32,
    #[serde(default)]
    pub groq_api_key: String,
    #[serde(default = "default_enhancement_model")]
    pub enhancement_model: String,
    /// Empty means "whatever Windows reports as the default input". Stored by
    /// name because cpal device ids are not stable across reboots.
    #[serde(default)]
    pub input_device: String,
    /// Off by default in the Actions Preview so it cannot race the stable app
    /// for the global hotkey at login. An explicit opt-in is still honoured.
    #[serde(default)]
    pub start_on_login: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            active_provider: SpeechProvider::Groq,
            groq: ProviderSettings {
                model: "whisper-large-v3-turbo".to_string(),
                language: "en".to_string(),
                prompt: String::new(),
                endpoint: "https://api.groq.com/openai/v1/audio/transcriptions".to_string(),
            },
            hotkey: "\\".to_string(),
            activation_mode: ActivationMode::Toggle,
            mode: DictationMode::Raw,
            custom_mode_instruction: String::new(),
            auto_paste: true,
            selection_actions_enabled: false,
            retain_audio: false,
            output_mode: "clipboard-paste".to_string(),
            bubble_outline: default_bubble_outline(),
            outline_width: default_outline_width(),
            groq_api_key: String::new(),
            enhancement_model: default_enhancement_model(),
            input_device: String::new(),
            start_on_login: false,
        }
    }
}

fn default_bubble_outline() -> String {
    "#ff4057".to_string()
}

fn default_outline_width() -> u32 {
    1
}

fn default_enhancement_model() -> String {
    "llama-4-scout-17b-16e-instruct".to_string()
}

pub fn settings_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Config(format!("Cannot resolve app data directory: {e}")))?;
    fs::create_dir_all(&dir)
        .map_err(|e| AppError::Config(format!("Cannot create app data directory: {e}")))?;
    Ok(dir.join("settings.json"))
}

pub fn load_settings(app: &AppHandle) -> Result<AppSettings, AppError> {
    let path = settings_path(app)?;
    if !path.exists() {
        let defaults = AppSettings::default();
        save_settings(app, &defaults)?;
        return Ok(defaults);
    }

    let text = fs::read_to_string(&path).map_err(|e| {
        AppError::Config(format!("Cannot read settings file {}: {e}", path.display()))
    })?;
    let mut loaded: AppSettings = match serde_json::from_str(&text) {
        Ok(loaded) => loaded,
        Err(error) => {
            // Same crash signature as history.json: rather than bricking every
            // launch, set the bad file aside and fall back to defaults.
            let moved = storage::quarantine(&path);
            eprintln!(
                "OrcaVoice: settings file was corrupt ({error}) and has been set aside ({moved:?}). Using defaults."
            );
            let defaults = AppSettings::default();
            save_settings(app, &defaults)?;
            return Ok(defaults);
        }
    };
    if loaded.hotkey == "CommandOrControl+Shift+Space" || loaded.hotkey == "/" {
        loaded.hotkey = "\\".to_string();
    }

    // Rewrite whenever the file differs from the canonical shape so retired
    // provider blocks (openai, parakeet) are dropped instead of lingering.
    if serde_json::to_string_pretty(&loaded)? != text {
        save_settings(app, &loaded)?;
    }
    Ok(loaded)
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), AppError> {
    let path = settings_path(app)?;
    let text = serde_json::to_string_pretty(settings)?;
    // Atomic: a crash mid-write would otherwise leave a NUL-filled settings
    // file and the app would refuse to start with a JSON parse error.
    storage::write_atomic(&path, &text)
}
