# OrcaVoice — Tool Selection Validation

## Selected tools
| Area | Tool | Version | Validation |
|---|---|---|---|
| Desktop shell | Tauri | 2.11.5 | Lower idle footprint than Electron; native tray/hotkey |
| Hotkeys | tauri-plugin-global-shortcut | 2.3.2 | Official; supports Windows/macOS/Linux |
| Clipboard | tauri-plugin-clipboard-manager | 2.3.2 | Official; needed for paste |
| Settings | tauri-plugin-store | 2.4.3 | Non-secret settings only |
| Autostart | tauri-plugin-autostart | 2.5 | Official; registers the HKCU Run entry |
| Single instance | tauri-plugin-single-instance | 2.4.3 | Required: duplicate instances fight over the global hotkey |
| Secrets | keyring | 4.1.2 | OS keyring for API keys |
| Audio | cpal | 0.18.1 | Standard Rust audio capture |
| WAV encoding | hound | 3.5.1 | Simple WAV writer |
| HTTP | reqwest | 0.13.4 | Multipart uploads; `rustls` feature |
| Text insertion | enigo | 0.6.1 | Clipboard paste primary; enigo fallback |
| Frontend | React 19 + TypeScript 6 + Vite 8 | | Modern, fast |

## Rejected alternatives
- **Electron**: heavier, unnecessary for a tray dictation app.
- **OpenAI Realtime**: wrong pattern for dictation; too expensive.
- **OpenAI transcription**: removed; slower and pricier than Groq for no quality gain.
- **Parakeet local endpoint**: removed; existed only as dead settings keys.
- **whisper.cpp**: too much setup for the value.
- **Blabby app.asar code**: unacceptable IP risk.

## API verification notes
- cpal 0.18: `SampleRate` is `u32` (not struct); `Device` uses `.description()?.name()`; callbacks are typed `&[T]`.
- tauri-plugin-global-shortcut 2.3: `register()` accepts `TryInto<ShortcutWrapper>` (string works).
- reqwest 0.13: `rustls-tls` renamed to `rustls`.
- tauri 2.11: `TrayIconBuilder` renders a blank tray entry unless `.icon()` is set
  explicitly; `app.default_window_icon()` is the usual source.
- tauri-plugin-single-instance must be registered **first**, before other plugins.

## Build tooling
Build desktop artifacts with the Tauri CLI (`npm run tauri:build`), never bare
cargo. Tauri's build script sets `dev = !custom-protocol`, so `cargo build
--release` yields a dev binary that loads `devUrl` instead of its embedded
frontend. `src-tauri/build.rs` rejects that combination at compile time and also
emits `cargo:rerun-if-changed` for every file under `dist/`, so frontend-only
changes cannot leave a stale UI embedded in the binary.
