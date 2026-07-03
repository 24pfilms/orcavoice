# OrcaVoice — Phase B: Highlighted-Text Text-to-Speech

**Status:** PLANNED ONLY. Do not implement until the user explicitly commands it after Phase A is working perfectly.

## Goal
Add a **Read Selection** button/hotkey that speaks highlighted text from any app.

## Methodology
```text
User highlights text in any app
  → user clicks OrcaVoice "Read Selection" or presses a hotkey
  → app preserves current clipboard
  → app simulates Ctrl+C to copy selected text
  → app reads clipboard text
  → app restores previous clipboard when safe
  → app streams/generates speech via OpenAI gpt-4o-mini-tts
  → app plays audio with pause/stop controls
```

## TTS provider
- Default: OpenAI `gpt-4o-mini-tts` with voice `marin` or `cedar`.
- Fallback: Windows system TTS (free, lower quality).

## UX
- Tray menu button: **Read selected text**.
- Optional hotkey: default unassigned until user chooses one.
- Floating mini-player: Stop, Pause/Resume, voice selector, speed selector.

## Privacy
- Highlighted text sent only to the configured TTS provider on user action.
- UI must label: "Selected text will be sent to OpenAI for speech generation."
- Do not store highlighted text by default.
