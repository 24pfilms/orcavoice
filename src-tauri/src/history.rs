use crate::error::AppError;
use crate::settings::SpeechProvider;
use crate::stt::TranscriptionResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const MAX_HISTORY: usize = 200;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub provider: SpeechProvider,
    pub model: String,
    pub text: String,
    pub raw_text: String,
    pub latency_ms: u128,
    pub estimated_cost_usd: f64,
    pub audio_duration_ms: u128,
}

impl From<TranscriptionResult> for HistoryEntry {
    fn from(value: TranscriptionResult) -> Self {
        Self {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            provider: value.provider,
            model: value.model,
            text: value.text,
            raw_text: value.raw_text,
            latency_ms: value.latency_ms,
            estimated_cost_usd: value.estimated_cost_usd,
            audio_duration_ms: value.audio_duration_ms,
        }
    }
}

fn history_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::History(format!("Cannot resolve app data directory: {e}")))?;
    fs::create_dir_all(&dir)
        .map_err(|e| AppError::History(format!("Cannot create app data directory: {e}")))?;
    Ok(dir.join("history.json"))
}

pub fn load_history(app: &AppHandle) -> Result<Vec<HistoryEntry>, AppError> {
    let path = history_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| AppError::History(format!("Cannot read history file {}: {e}", path.display())))?;
    // Parse leniently: drop individual entries this version cannot deserialize
    // (e.g. entries written by a newer build using an unknown provider) so a
    // single incompatible record never breaks dictation.
    let raw: Vec<serde_json::Value> = serde_json::from_str(&text)
        .map_err(|e| AppError::History(format!("History file is invalid JSON: {e}")))?;
    let history = raw
        .into_iter()
        .filter_map(|value| serde_json::from_value::<HistoryEntry>(value).ok())
        .collect();
    Ok(history)
}

pub fn append_history(app: &AppHandle, result: TranscriptionResult) -> Result<HistoryEntry, AppError> {
    let mut history = load_history(app)?;
    let entry = HistoryEntry::from(result);
    history.insert(0, entry.clone());
    history.truncate(MAX_HISTORY);
    save_history(app, &history)?;
    Ok(entry)
}

pub fn save_history(app: &AppHandle, history: &[HistoryEntry]) -> Result<(), AppError> {
    let path = history_path(app)?;
    let text = serde_json::to_string_pretty(history)
        .map_err(|e| AppError::History(format!("Cannot serialize history: {e}")))?;
    fs::write(&path, text)
        .map_err(|e| AppError::History(format!("Cannot write history file {}: {e}", path.display())))
}

pub fn clear_history(app: &AppHandle) -> Result<(), AppError> {
    save_history(app, &[])
}
