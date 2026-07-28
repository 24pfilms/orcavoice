# OrcaVoice — Phase B: Highlighted-Text Text-to-Speech

**Status:** PLANNED ONLY — no code for this exists. Do not implement until the
user explicitly commands it.

> **Open decision:** this plan predates the move to a Groq-only stack. OrcaVoice
> no longer has an OpenAI key or provider, so the TTS provider below must be
> re-chosen before any implementation starts.

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
Unresolved. The original plan assumed OpenAI `gpt-4o-mini-tts`, which is no longer
available to the app. Windows system TTS (free, lower quality, no network) is the
only option that works with the current dependency set.

## UX
- Tray menu button: **Read selected text**.
- Optional hotkey: default unassigned until user chooses one.
- Floating mini-player: Stop, Pause/Resume, voice selector, speed selector.

## Privacy
- Highlighted text sent only to the configured TTS provider on user action.
- UI must name the actual provider in the label before any text is sent.
- Do not store highlighted text by default.
