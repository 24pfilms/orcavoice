use crate::error::AppError;
use crate::settings::SpeechProvider;
use keyring::Entry;
use serde::{Deserialize, Serialize};

const SERVICE_NAME: &str = "com.squarecirclelabs.orcavoice";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecretStatus {
    pub groq: bool,
    pub openai: bool,
    pub env_groq: bool,
    pub env_openai: bool,
}

fn env_var(provider: SpeechProvider) -> &'static str {
    match provider {
        SpeechProvider::Groq => "GROQ_API_KEY",
        SpeechProvider::OpenAi => "OPENAI_API_KEY",
    }
}

fn account(provider: SpeechProvider) -> String {
    format!("{}-api-key", provider.key_name())
}

fn read_keyring(provider: SpeechProvider) -> Result<Option<String>, AppError> {
    let entry = Entry::new(SERVICE_NAME, &account(provider))
        .map_err(|e| AppError::Secret(format!("Cannot open OS keyring entry: {e}")))?;
    match entry.get_password() {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::Secret(format!("Cannot read OS keyring entry: {e}"))),
    }
}

pub fn get_api_key(provider: SpeechProvider) -> Result<String, AppError> {
    if let Ok(value) = std::env::var(env_var(provider)) {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    read_keyring(provider)?.ok_or_else(|| {
        AppError::Secret(format!(
            "No API key found for {:?}. Set it in Settings or provide {}.",
            provider,
            env_var(provider)
        ))
    })
}

pub fn set_api_key(provider: SpeechProvider, api_key: String) -> Result<(), AppError> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return clear_api_key(provider);
    }

    let entry = Entry::new(SERVICE_NAME, &account(provider))
        .map_err(|e| AppError::Secret(format!("Cannot open OS keyring entry: {e}")))?;
    entry
        .set_password(trimmed)
        .map_err(|e| AppError::Secret(format!("Cannot store API key in OS keyring: {e}")))
}

pub fn clear_api_key(provider: SpeechProvider) -> Result<(), AppError> {
    let entry = Entry::new(SERVICE_NAME, &account(provider))
        .map_err(|e| AppError::Secret(format!("Cannot open OS keyring entry: {e}")))?;
    match entry.delete_credential() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Secret(format!("Cannot remove API key from OS keyring: {e}"))),
    }
}

pub fn secret_status_with_fallback(fallback_groq: &str, fallback_openai: &str) -> SecretStatus {
    let env_groq = std::env::var(env_var(SpeechProvider::Groq))
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let env_openai = std::env::var(env_var(SpeechProvider::OpenAi))
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);

    let keyring_groq = read_keyring(SpeechProvider::Groq).ok().flatten().is_some();
    let keyring_openai = read_keyring(SpeechProvider::OpenAi).ok().flatten().is_some();
    let file_groq = !fallback_groq.trim().is_empty();
    let file_openai = !fallback_openai.trim().is_empty();

    SecretStatus {
        groq: env_groq || keyring_groq || file_groq,
        openai: env_openai || keyring_openai || file_openai,
        env_groq,
        env_openai,
    }
}

pub fn get_api_key_with_fallback(provider: SpeechProvider, file_key: &str) -> Result<String, AppError> {
    if let Ok(value) = std::env::var(env_var(provider)) {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    if let Some(key) = read_keyring(provider)? {
        return Ok(key);
    }
    let trimmed = file_key.trim().to_string();
    if !trimmed.is_empty() {
        return Ok(trimmed);
    }
    Err(AppError::Secret(format!(
        "No API key found for {:?}. Set it in Settings.",
        provider
    )))
}

pub fn secret_status() -> SecretStatus {
    let env_groq = std::env::var(env_var(SpeechProvider::Groq))
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let env_openai = std::env::var(env_var(SpeechProvider::OpenAi))
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);

    SecretStatus {
        groq: env_groq || read_keyring(SpeechProvider::Groq).ok().flatten().is_some(),
        openai: env_openai || read_keyring(SpeechProvider::OpenAi).ok().flatten().is_some(),
        env_groq,
        env_openai,
    }
}
