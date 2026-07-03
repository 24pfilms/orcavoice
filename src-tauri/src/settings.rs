use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SpeechProvider {
    Groq,
    OpenAi,
}

impl SpeechProvider {
    pub fn key_name(self) -> &'static str {
        match self {
            SpeechProvider::Groq => "groq",
            SpeechProvider::OpenAi => "openai",
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppSettings {
    pub active_provider: SpeechProvider,
    pub groq: ProviderSettings,
    pub openai: ProviderSettings,
    pub hotkey: String,
    pub mode: DictationMode,
    pub custom_mode_instruction: String,
    pub auto_paste: bool,
    pub retain_audio: bool,
    pub output_mode: String,
    #[serde(default = "default_bubble_outline")]
    pub bubble_outline: String,
    #[serde(default)]
    pub groq_api_key: String,
    #[serde(default)]
    pub openai_api_key: String,
    #[serde(default = "default_enhancement_model")]
    pub enhancement_model: String,
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
            openai: ProviderSettings {
                model: "gpt-4o-mini-transcribe".to_string(),
                language: "en".to_string(),
                prompt: String::new(),
                endpoint: "https://api.openai.com/v1/audio/transcriptions".to_string(),
            },
            hotkey: "\\".to_string(),
            mode: DictationMode::Raw,
            custom_mode_instruction: String::new(),
            auto_paste: true,
            retain_audio: false,
            output_mode: "clipboard-paste".to_string(),
            bubble_outline: default_bubble_outline(),
            groq_api_key: String::new(),
            openai_api_key: String::new(),
            enhancement_model: default_enhancement_model(),
        }
    }
}

fn default_bubble_outline() -> String {
    "#ff4057".to_string()
}

fn default_enhancement_model() -> String {
    "llama-4-scout-17b-16e-instruct".to_string()
}

impl AppSettings {
    pub fn active_provider_settings(&self) -> &ProviderSettings {
        match self.active_provider {
            SpeechProvider::Groq => &self.groq,
            SpeechProvider::OpenAi => &self.openai,
        }
    }
}

pub fn settings_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Config(format!("Cannot resolve app data directory: {e}")))?;
    fs::create_dir_all(&dir).map_err(|e| AppError::Config(format!("Cannot create app data directory: {e}")))?;
    Ok(dir.join("settings.json"))
}

pub fn load_settings(app: &AppHandle) -> Result<AppSettings, AppError> {
    let path = settings_path(app)?;
    if !path.exists() {
        let defaults = AppSettings::default();
        save_settings(app, &defaults)?;
        return Ok(defaults);
    }

    let text = fs::read_to_string(&path)
        .map_err(|e| AppError::Config(format!("Cannot read settings file {}: {e}", path.display())))?;
    let mut loaded: AppSettings = serde_json::from_str(&text)
        .map_err(|e| AppError::Config(format!("Settings file is invalid JSON: {e}")))?;
    if loaded.hotkey == "CommandOrControl+Shift+Space" || loaded.hotkey == "/" {
        loaded.hotkey = "\\".to_string();
        save_settings(app, &loaded)?;
    }
    Ok(loaded)
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), AppError> {
    let path = settings_path(app)?;
    let text = serde_json::to_string_pretty(settings)?;
    fs::write(&path, text)
        .map_err(|e| AppError::Config(format!("Cannot write settings file {}: {e}", path.display())))
}
