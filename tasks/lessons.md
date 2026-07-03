# Lessons

[2026-07-03] [OrcaVoice] Mistook Blabby's trigger as forward slash from dictation wording → Verify local config/keycodes first; keycode 220 maps to backslash (`\\`), not forward slash (`/`).
[2026-07-03] [OrcaVoice] Over-engineered sound/ducking with WebAudio, Beep, and raw COM volume changes before matching Blabby's working timing → For global-hotkey SFX, use native WAV playback; play start SFX before ducking; always restore audio sessions on stop/cancel/startup.
