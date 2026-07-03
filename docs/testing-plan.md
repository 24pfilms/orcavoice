# OrcaVoice Phase A — Testing Plan

## 12 Phase A gates

### Gate 1 — Scaffold
- `npm install` completes with pinned dependency versions.
- `npx tsc --noEmit` passes with zero errors.
- `cargo check` compiles the Rust library.
- `cargo test --lib` passes all unit tests.

### Gate 2 — Settings
- Settings save/load independently for Groq and OpenAI.
- Switching `active_provider` from Groq to OpenAI (or back) does not require app restart.
- Settings file persists at `app_data_dir/settings.json`.

### Gate 3 — Hotkey/Tray
- Default hotkey `Ctrl+Shift+Space` toggles recording from any app.
- Tray icon appears with "Show OrcaVoice" and "Quit" menu.
- Tray status transitions: idle → recording → transcribing → done/error.

### Gate 4 — Audio
- 5-second sample records from default microphone.
- Audio is encoded as 16 kHz mono WAV.
- Empty/silent recordings (< 0.5s or RMS < 80) are rejected with clear error.

### Gate 5 — Groq STT
- Same 5-second spoken phrase transcribes through Groq `whisper-large-v3-turbo`.
- Transcript appears in the app and is pasted if `auto_paste` is enabled.

### Gate 6 — OpenAI STT
- Same 5-second spoken phrase transcribes through OpenAI `gpt-4o-mini-transcribe`.
- Switch provider to OpenAI in settings, save, record, and verify transcript.

### Gate 7 — Provider benchmark
- Click "Stop + benchmark providers" after recording.
- UI shows Groq and OpenAI transcripts side-by-side with latency and cost estimates.

### Gate 8 — Paste
- Transcribed text pastes into Notepad when Notepad is the focused window.
- Transcribed text pastes into a browser text field.

### Gate 9 — Modes
- Raw mode returns transcript as-is.
- Grammar mode capitalizes first letter and adds terminal punctuation.
- Email mode wraps text in email template.
- Translate/Custom modes produce formatted output.

### Gate 10 — Errors
- Missing API key → clear "No API key found" error.
- Invalid API key → provider auth error with status code.
- Mic permission denied → "No default microphone" or stream error.
- Hotkey conflict → "Cannot register hotkey" error.
- Network failure → reqwest error message.

### Gate 11 — Privacy/History
- History records transcript text, provider, model, latency, cost, and timestamp.
- No audio is stored when `retain_audio` is false.
- Privacy notice shows current provider name.

### Gate 12 — Final checks
- `npx tsc --noEmit` passes.
- `cargo check` passes.
- `cargo test --lib` passes all 4 tests:
  - `audio::tests::wav_encoding_has_riff_header`
  - `audio::tests::rms_detects_non_silence`
  - `stt::tests::groq_cost_uses_ten_second_minimum`
  - `stt::tests::grammar_mode_capitalizes_and_punctuates`

## Manual test steps
1. Launch `npm run tauri:dev`.
2. Set a Groq API key in Settings → Save key.
3. Open Notepad.
4. Press `Ctrl+Shift+Space`, say "Hello world this is a test", press again.
5. Verify text appears in Notepad.
6. Switch to OpenAI provider, set key, repeat.
7. Run benchmark to compare both providers side-by-side.
