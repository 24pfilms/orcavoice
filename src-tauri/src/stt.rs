use crate::audio::CapturedAudio;
use crate::error::AppError;
use crate::secrets;
use crate::settings::{AppSettings, DictationMode, SpeechProvider};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TranscriptionResult {
    pub provider: SpeechProvider,
    pub model: String,
    pub text: String,
    pub raw_text: String,
    pub enhanced: bool,
    pub is_command: bool,
    pub latency_ms: u128,
    pub estimated_cost_usd: f64,
    pub audio_duration_ms: u128,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderBenchmarkResult {
    pub ok: bool,
    pub result: Option<TranscriptionResult>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BenchmarkResult {
    pub groq: ProviderBenchmarkResult,
    pub openai: ProviderBenchmarkResult,
}

#[derive(Debug, Deserialize)]
struct ProviderJsonResponse {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

const GROQ_CHAT_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";
const LLM_TIMEOUT_SECS: u64 = 5;

pub async fn transcribe_active_provider(
    settings: &AppSettings,
    audio: &CapturedAudio,
) -> Result<TranscriptionResult, AppError> {
    transcribe_with_provider(settings.active_provider, settings, audio).await
}

pub async fn benchmark_providers(
    settings: &AppSettings,
    audio: &CapturedAudio,
) -> BenchmarkResult {
    let groq = to_benchmark_result(
        transcribe_with_provider(SpeechProvider::Groq, settings, audio).await,
    );
    let openai = to_benchmark_result(
        transcribe_with_provider(SpeechProvider::OpenAi, settings, audio).await,
    );
    BenchmarkResult { groq, openai }
}

fn to_benchmark_result(result: Result<TranscriptionResult, AppError>) -> ProviderBenchmarkResult {
    match result {
        Ok(result) => ProviderBenchmarkResult {
            ok: true,
            result: Some(result),
            error: None,
        },
        Err(error) => ProviderBenchmarkResult {
            ok: false,
            result: None,
            error: Some(error.to_string()),
        },
    }
}

pub async fn transcribe_with_provider(
    provider: SpeechProvider,
    settings: &AppSettings,
    audio: &CapturedAudio,
) -> Result<TranscriptionResult, AppError> {
    let provider_settings = match provider {
        SpeechProvider::Groq => &settings.groq,
        SpeechProvider::OpenAi => &settings.openai,
    };
    let file_key = match provider {
        SpeechProvider::Groq => &settings.groq_api_key,
        SpeechProvider::OpenAi => &settings.openai_api_key,
    };
    let api_key = secrets::get_api_key_with_fallback(provider, file_key)?;
    let started = Instant::now();

    let file_part = Part::bytes(audio.wav_bytes.clone())
        .file_name("orcavoice.wav")
        .mime_str("audio/wav")
        .map_err(|e| AppError::Provider(format!("Cannot create audio upload part: {e}")))?;

    let mut form = Form::new()
        .part("file", file_part)
        .text("model", provider_settings.model.clone())
        .text("response_format", "json".to_string());

    if !provider_settings.language.trim().is_empty() && provider_settings.language.trim() != "auto" {
        form = form.text("language", provider_settings.language.trim().to_string());
    }
    if !provider_settings.prompt.trim().is_empty() {
        form = form.text("prompt", provider_settings.prompt.trim().to_string());
    }

    let client = reqwest::Client::new();
    let response = client
        .post(provider_settings.endpoint.clone())
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| AppError::Network(format!("Cannot read STT response: {e}")))?;

    if !status.is_success() {
        return Err(AppError::Provider(format!(
            "{} transcription failed ({status}): {}",
            provider_label(provider),
            body.chars().take(500).collect::<String>()
        )));
    }

    let text = parse_transcript_text(&body)?;

    if settings.voice_commands_enabled {
        if let Some(app_name) = crate::commands::detect_open(&text) {
            if crate::commands::open_application(&app_name).is_ok() {
                return Ok(TranscriptionResult {
                    provider,
                    model: provider_settings.model.clone(),
                    text: format!("Opening {app_name}\u{2026}"),
                    raw_text: text,
                    enhanced: false,
                    is_command: true,
                    latency_ms: started.elapsed().as_millis(),
                    estimated_cost_usd: estimate_cost(
                        provider,
                        audio.summary.duration_ms,
                        false,
                    ),
                    audio_duration_ms: audio.summary.duration_ms,
                });
            }
        }
    }

    let (polished, enhanced) = apply_mode(settings, &text).await;
    Ok(TranscriptionResult {
        provider,
        model: provider_settings.model.clone(),
        text: polished,
        raw_text: text,
        enhanced,
        is_command: false,
        latency_ms: started.elapsed().as_millis(),
        estimated_cost_usd: estimate_cost(provider, audio.summary.duration_ms, enhanced),
        audio_duration_ms: audio.summary.duration_ms,
    })
}

fn parse_transcript_text(body: &str) -> Result<String, AppError> {
    if let Ok(value) = serde_json::from_str::<ProviderJsonResponse>(body) {
        if let Some(text) = value.text {
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }
    }

    let trimmed = body.trim().trim_matches('"').to_string();
    if trimmed.is_empty() {
        return Err(AppError::Provider(
            "Transcription provider returned an empty transcript.".to_string(),
        ));
    }
    Ok(trimmed)
}

fn estimate_cost(provider: SpeechProvider, duration_ms: u128, enhanced: bool) -> f64 {
    let minutes = (duration_ms as f64 / 60_000.0).max(10.0 / 60.0);
    let stt_cost = match provider {
        SpeechProvider::Groq => minutes / 60.0 * 0.04,
        SpeechProvider::OpenAi => minutes * 0.003,
    };
    if enhanced {
        // Llama 4 Scout on Groq: ~80 input tokens + ~30 output tokens per dictation
        let llm_cost = 80.0 * 0.11 / 1_000_000.0 + 30.0 * 0.34 / 1_000_000.0;
        stt_cost + llm_cost
    } else {
        stt_cost
    }
}

fn provider_label(provider: SpeechProvider) -> &'static str {
    match provider {
        SpeechProvider::Groq => "Groq",
        SpeechProvider::OpenAi => "OpenAI",
    }
}

fn mode_system_prompt(mode: DictationMode, custom_instruction: &str) -> Option<String> {
    match mode {
        DictationMode::Raw => None,
        DictationMode::Grammar => Some(
            "Fix grammar, punctuation, and capitalization. Remove filler words \
             (um, uh, like, you know). Preserve meaning and tone. Output only the \
             corrected text \u{2014} no explanations, no preamble."
                .to_string(),
        ),
        DictationMode::Email => Some(
            "Transform this transcript into a professional email with greeting and \
             sign-off. Fix grammar and formatting. Keep it concise. Output only the \
             email text."
                .to_string(),
        ),
        DictationMode::TranslateEnglish => Some(
            "Translate to English. Capture slang, nuance, and cultural context. \
             Output only the translation \u{2014} no explanations."
                .to_string(),
        ),
        DictationMode::Custom => {
            let trimmed = custom_instruction.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
    }
}

async fn apply_mode(settings: &AppSettings, raw_text: &str) -> (String, bool) {
    let Some(system_prompt) =
        mode_system_prompt(settings.mode, &settings.custom_mode_instruction)
    else {
        return (raw_text.to_string(), false);
    };

    // Enhancement always uses the Groq API key, regardless of STT provider.
    let groq_key = match secrets::get_api_key_with_fallback(
        SpeechProvider::Groq,
        &settings.groq_api_key,
    ) {
        Ok(key) => key,
        Err(_) => return (raw_text.to_string(), false),
    };

    match groq_enhance(&groq_key, &settings.enhancement_model, &system_prompt, raw_text).await {
        Ok(enhanced) if !enhanced.trim().is_empty() => (enhanced, true),
        _ => (raw_text.to_string(), false),
    }
}

async fn groq_enhance(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    transcript: &str,
) -> Result<String, AppError> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": transcript },
        ],
        "temperature": 0.3,
        "max_completion_tokens": 2048,
    });

    let response = tokio::time::timeout(
        Duration::from_secs(LLM_TIMEOUT_SECS),
        client
            .post(GROQ_CHAT_ENDPOINT)
            .bearer_auth(api_key)
            .json(&body)
            .send(),
    )
    .await
    .map_err(|_| {
        AppError::Provider("LLM enhancement timed out after 5 seconds".to_string())
    })?
    .map_err(AppError::from)?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|e| AppError::Network(format!("Cannot read LLM response: {e}")))?;

    if !status.is_success() {
        return Err(AppError::Provider(format!(
            "Groq LLM enhancement failed ({status}): {}",
            response_text.chars().take(500).collect::<String>()
        )));
    }

    let parsed: ChatCompletionResponse = serde_json::from_str(&response_text)?;
    parsed
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.content)
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| AppError::Provider("Groq LLM returned an empty response".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groq_cost_uses_ten_second_minimum() {
        let cost = estimate_cost(SpeechProvider::Groq, 5_000, false);
        assert!(cost > 0.0);
        assert!((cost - (10.0 / 3600.0 * 0.04)).abs() < 0.000001);
    }

    #[test]
    fn enhanced_cost_adds_llm_tokens() {
        let base = estimate_cost(SpeechProvider::Groq, 5_000, false);
        let enhanced = estimate_cost(SpeechProvider::Groq, 5_000, true);
        assert!(enhanced > base);
    }

    #[test]
    fn raw_mode_has_no_system_prompt() {
        assert!(mode_system_prompt(DictationMode::Raw, "").is_none());
    }

    #[test]
    fn grammar_mode_has_system_prompt() {
        assert!(mode_system_prompt(DictationMode::Grammar, "").is_some());
    }

    #[test]
    fn custom_mode_without_instruction_has_no_prompt() {
        assert!(mode_system_prompt(DictationMode::Custom, "").is_none());
    }

    #[test]
    fn custom_mode_with_instruction_has_prompt() {
        assert!(mode_system_prompt(DictationMode::Custom, "Make it formal").is_some());
    }
}
