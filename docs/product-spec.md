# OrcaVoice Phase A — Product Spec

## Overview
OrcaVoice is a system-wide speech-to-text dictation desktop app with Groq/OpenAI provider switching, AI modes, clipboard paste, local history, and privacy-first design.

## Phase A scope
- Global hotkey (default: `Ctrl+Shift+Space`) toggles recording.
- Microphone capture via `cpal`, encoded to 16 kHz mono WAV.
- Transcription via Groq `whisper-large-v3-turbo` (default) or OpenAI `gpt-4o-mini-transcribe` / `gpt-4o-transcribe`.
- Simple provider switch in Settings — no restart required.
- Optional AI modes: Raw, Grammar, Email, Translate to English, Custom.
- Clipboard paste into active app after transcription.
- Side-by-side provider benchmark.
- Local history (200 entries, audio retention off by default).
- System tray with status.
- Fail-loud errors for mic denial, missing API key, provider auth failure, silence, and paste failure.
- API keys stored in OS keyring (`keyring` crate) or environment variables.

## Workflows
1. **Dictate:** Press hotkey → speak → press again → transcript appears at cursor.
2. **Benchmark:** Press hotkey → speak → click "Stop + benchmark" → see Groq vs OpenAI side-by-side.
3. **Settings:** Switch provider, set API keys, configure model/language/prompt/hotkey/mode.
4. **History:** Browse past transcripts, re-paste any entry.

## Out of scope for Phase A
- Phase B (highlighted-text TTS) — planned only, not implemented.
- Local whisper.cpp.
- Streaming/partial captions.
- Browser extension.
- LLM-based polish (modes are conservative local formatting only).

## Settings
- `active_provider`: `groq` | `open-ai`
- Per-provider: model, language, prompt/vocabulary, endpoint
- `hotkey`: default `CommandOrControl+Shift+Space`
- `mode`: `raw` | `grammar` | `email` | `translate-english` | `custom`
- `custom_mode_instruction`: user text for custom mode
- `auto_paste`: boolean (default true)
- `retain_audio`: boolean (default false — Phase A stores no audio regardless)
