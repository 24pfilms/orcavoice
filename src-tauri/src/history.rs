use crate::error::AppError;
use crate::settings::SpeechProvider;
use crate::storage;
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

/// Split out from `load_history` so the corruption behaviour is unit-testable
/// without a Tauri `AppHandle`.
fn parse_history(text: &str) -> Option<Vec<HistoryEntry>> {
    // Parse leniently: drop individual entries this version cannot deserialize
    // (e.g. entries written by a newer build using an unknown provider) so a
    // single incompatible record never breaks dictation.
    let raw: Vec<serde_json::Value> = serde_json::from_str(text).ok()?;
    Some(
        raw.into_iter()
            .filter_map(|value| serde_json::from_value::<HistoryEntry>(value).ok())
            .collect(),
    )
}

pub fn load_history(app: &AppHandle) -> Result<Vec<HistoryEntry>, AppError> {
    let path = history_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    // A crash mid-write can leave a file of NUL bytes whose length looks valid.
    // Erroring here used to fail every subsequent dictation *after* the paste
    // had already happened, so the transcript appeared and the overlay still
    // flashed red. Quarantine the bad file and carry on with an empty history.
    let Ok(text) = fs::read_to_string(&path) else {
        let moved = storage::quarantine(&path);
        eprintln!(
            "OrcaVoice: history file was unreadable and has been set aside ({:?}).",
            moved
        );
        return Ok(Vec::new());
    };

    match parse_history(&text) {
        Some(history) => Ok(history),
        None => {
            let moved = storage::quarantine(&path);
            eprintln!(
                "OrcaVoice: history file was corrupt and has been set aside ({:?}). Starting a new history.",
                moved
            );
            Ok(Vec::new())
        }
    }
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
    storage::write_atomic(&path, &text)
}

pub fn clear_history(app: &AppHandle) -> Result<(), AppError> {
    save_history(app, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_of_nul_bytes_is_treated_as_empty_not_an_error() {
        // Exactly how history.json was found on disk: length looks sane, every
        // byte is zero. This must not be able to fail a dictation.
        let corrupt = "\u{0}".repeat(1024);
        assert!(parse_history(&corrupt).is_none());
    }

    #[test]
    fn truncated_json_is_rejected_rather_than_half_parsed() {
        assert!(parse_history("[{\"id\":").is_none());
        assert!(parse_history("").is_none());
    }

    #[test]
    fn valid_history_parses_and_skips_unreadable_entries() {
        let text = r#"[
            {"id":"00000000-0000-4000-8000-000000000000",
             "created_at":"2026-07-28T00:00:00Z",
             "provider":"groq","model":"whisper-large-v3-turbo",
             "text":"hello","raw_text":"hello",
             "latency_ms":10,"estimated_cost_usd":0.0,"audio_duration_ms":10},
            {"garbage":true}
        ]"#;
        let parsed = parse_history(text).expect("array itself is valid JSON");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].text, "hello");
    }

    #[test]
    fn an_empty_array_is_valid() {
        assert_eq!(parse_history("[]").map(|h| h.len()), Some(0));
    }
}
