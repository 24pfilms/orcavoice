# OrcaVoice — Tool Selection Validation

## Selected tools
| Area | Tool | Version | Validation |
|---|---|---|---|
| Desktop shell | Tauri | 2.11.5 | Lower idle footprint than Electron; native tray/hotkey |
| Hotkeys | tauri-plugin-global-shortcut | 2.3.2 | Official; supports Windows/macOS/Linux |
| Clipboard | tauri-plugin-clipboard-manager | 2.3.2 | Official; needed for paste |
| Settings | tauri-plugin-store | 2.4.3 | Non-secret settings only |
| Secrets | keyring | 4.1.2 | OS keyring for API keys |
| Audio | cpal | 0.18.1 | Standard Rust audio capture |
| WAV encoding | hound | 3.5.1 | Simple WAV writer |
| HTTP | reqwest | 0.13.4 | Multipart uploads; `rustls` feature |
| Text insertion | enigo | 0.6.1 | Clipboard paste primary; enigo fallback |
| Frontend | React 19 + TypeScript 6 + Vite 8 | | Modern, fast |

## Rejected alternatives
- **Electron**: heavier, unnecessary for a tray dictation app.
- **OpenAI Realtime**: wrong pattern for dictation; too expensive.
- **whisper.cpp first**: too much setup for fastest working Phase A.
- **Blabby app.asar code**: unacceptable IP risk.

## API verification notes
- cpal 0.18: `SampleRate` is `u32` (not struct); `Device` uses `.description()?.name()`; callbacks are typed `&[T]`.
- tauri-plugin-global-shortcut 2.3: `register()` accepts `TryInto<ShortcutWrapper>` (string works).
- reqwest 0.13: `rustls-tls` renamed to `rustls`.
