# OrcaVoice — Product Spec

## Overview
OrcaVoice is a system-wide speech-to-text dictation desktop app built on Groq,
with AI dictation modes, clipboard paste, local history, and privacy-first design.

## Scope
- Global hotkey (default: `\`) toggles recording.
- Microphone capture via `cpal`, encoded to 16 kHz mono WAV.
- Transcription via Groq `whisper-large-v3-turbo`.
- Dictation modes: Raw, Grammar, Email, Translate to English, Custom.
- Non-raw modes polish the transcript with a Groq LLM
  (default `llama-4-scout-17b-16e-instruct`, 5s timeout, falls back to raw text).
- Clipboard paste into active app after transcription.
- Local history (200 entries, audio retention off by default).
- System tray with status; single-instance enforcement.
- Fail-loud errors for mic denial, missing API key, provider auth failure,
  silence, and paste failure.
- API key stored in the OS keyring, with `settings.json` fallback and
  `GROQ_API_KEY` environment override.

## Workflows
1. **Dictate:** Press hotkey → speak → press again → transcript appears at cursor.
2. **Settings:** Set API key, configure language/vocabulary/hotkey/mode/enhancement model.
3. **History:** Browse past transcripts, re-paste any entry.

## Provider decision
Groq is the only provider. OpenAI transcription, the side-by-side provider
benchmark, and the local Parakeet endpoint were all removed — see
`stt-methodology.md` for the rationale.

## Out of scope
- Phase B (highlighted-text TTS) — planned only, not implemented.
- Local whisper.cpp.
- Streaming/partial captions.
- Browser extension.

## Settings
Persisted at `app_data_dir/settings.json`. The file is rewritten on load whenever
it differs from the canonical serialized shape, so keys from retired features are
dropped automatically.

- `active_provider`: `groq` (only valid value; unknown values fall back to Groq)
- `groq`: model, language, prompt/vocabulary, endpoint
- `hotkey`: default `\`
- `mode`: `raw` | `grammar` | `email` | `translate-english` | `custom`
- `custom_mode_instruction`: user text for custom mode
- `enhancement_model`: Groq LLM used to polish non-raw modes
- `auto_paste`: boolean (default true)
- `retain_audio`: boolean (default false — no audio is stored regardless)
- `output_mode`, `bubble_outline`, `outline_width`: overlay/output presentation
