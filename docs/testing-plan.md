# OrcaVoice — Testing Plan

## Automated gates

### Gate 1 — Scaffold
- `npm install` completes with pinned dependency versions.
- `npx tsc --noEmit` passes with zero errors.
- `cargo check` compiles the Rust library.
- `cargo test --lib` passes all 8 unit tests:
  - `audio::tests::wav_encoding_has_riff_header`
  - `audio::tests::rms_detects_non_silence`
  - `stt::tests::groq_cost_uses_ten_second_minimum`
  - `stt::tests::enhanced_cost_adds_llm_tokens`
  - `stt::tests::raw_mode_has_no_system_prompt`
  - `stt::tests::grammar_mode_has_system_prompt`
  - `stt::tests::custom_mode_with_instruction_has_prompt`
  - `stt::tests::custom_mode_without_instruction_has_no_prompt`

### Gate 2 — Build integrity
This gate exists because a dev-mode release binary previously shipped a stale UI.

- `cargo build --release` **must fail** with the "refusing to build a release
  binary in dev mode" panic from `src-tauri/build.rs`.
- `npm run tauri:build -- --no-bundle` must succeed.
- With **no dev server running**, launch the built exe and confirm the overlay
  renders. A blank or black window means a dev binary was shipped.
- After a frontend-only change, `npm run build` followed by a release build must
  actually relink — compare the exe mtime against `dist/`.

### Gate 3 — Settings
- Settings persist at `app_data_dir/settings.json`.
- A settings file containing retired keys (e.g. `openai`, `parakeet`) is rewritten
  without them on next load.
- Changing the hotkey re-registers it without an app restart.

### Gate 4 — Hotkey/Tray/Instance
- Default hotkey `\` toggles recording from any app.
- Tray icon appears (with a visible icon) and has "Show OrcaVoice" and "Quit".
- Launching a second copy exits immediately and surfaces the running instance,
  leaving exactly one process and no "HotKey already registered" error.

### Gate 5 — Audio
- 5-second sample records from default microphone.
- Audio is encoded as 16 kHz mono WAV.
- Empty/silent recordings (< 0.5s or RMS < 80) are rejected with clear error.

### Gate 6 — Groq STT
- A 5-second spoken phrase transcribes through Groq `whisper-large-v3-turbo`.
- Transcript appears in the app and is pasted if `auto_paste` is enabled.

### Gate 7 — Paste
- Transcribed text pastes into Notepad when Notepad is the focused window.
- Transcribed text pastes into a browser text field.

### Gate 8 — Modes
- Raw mode returns the transcript as-is and makes no LLM call.
- Grammar, Email, Translate, and Custom modes send the transcript to the Groq LLM.
- Custom mode with an empty instruction behaves as Raw.
- If the LLM call fails or exceeds its 5s timeout, the raw transcript is used and
  the dictation still completes.

### Gate 9 — Errors
- Missing API key → clear "No API key found" error.
- Invalid API key → provider auth error with status code.
- Mic permission denied → "No default microphone" or stream error.
- Hotkey conflict → "Cannot register hotkey" error.
- Network failure → reqwest error message.

### Gate 10 — Privacy/History
- History records transcript text, provider, model, latency, cost, and timestamp.
- No audio is stored when `retain_audio` is false.

## Manual test steps
1. Launch `npm run tauri:dev`.
2. Set a Groq API key in Settings → Save key.
3. Open Notepad.
4. Press `\`, say "Hello world this is a test", press again.
5. Verify text appears in Notepad.
6. Switch mode to Grammar and repeat; verify the text is cleaned up.
7. Quit, run `npm run tauri:build -- --no-bundle`, stop every dev server, and
   repeat steps 3–5 against the built exe.
