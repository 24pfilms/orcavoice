# OrcaVoice — Third-Party References

## Clean-room rule
OrcaVoice is built from scratch. No proprietary Blabby code is copied.

## Open-source systems reviewed for architecture/patterns

| Repo | License | Reuse status | What we learned |
|---|---|---|---|
| [OpenTypeless](https://github.com/tover0314-w/opentypeless) | MIT | Reference only; no code copied | Tauri/Rust voice input architecture, provider abstraction, enigo output |
| [handless](https://github.com/ElwinLiu/handless) | MIT | Reference only | Clipboard/paste handling with Tauri and enigo |
| [AGiXT](https://github.com/Josh-XT/AGiXT) | MIT | Reference only | cpal microphone capture patterns |
| [LocalScribe](https://github.com/MohanTn/LocalScribe) | Unknown | Reference only | Electron + whisper.cpp local dictation architecture |
| [super-stt](https://github.com/jorge-menjivar/super-stt) | GPL-3.0 | Reference only; no code copied | enigo keyboard output patterns |
| [Chirp](https://github.com/sitelift/Chirp) | Source-available/non-commercial | Reference only; no code copied | Clipboard + paste injection |

## Dependency licenses
All OrcaVoice dependencies are MIT or Apache-2.0 licensed:
- `tauri` — MIT/Apache-2.0
- `cpal` — Apache-2.0
- `hound` — Apache-2.0
- `enigo` — MIT
- `keyring` — MIT/Apache-2.0
- `reqwest` — MIT/Apache-2.0
- `serde` / `serde_json` — MIT/Apache-2.0
- `tokio` — MIT
- `hound` — Apache-2.0
