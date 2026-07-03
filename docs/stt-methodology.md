# OrcaVoice — STT Methodology

## Blabby's public claim
Blabby's website says "Powered by OpenAI's Whisper v3 Turbo." Blabby does not publish source code.

## OrcaVision's current stack
OrcaVision uses OpenAI Realtime (`gpt-realtime-2`) for live voice conversation, with `gpt-4o-transcribe` for input transcript events. This is a **live assistant** pattern, not dictation.

## OrcaVoice Phase A choice
- **Default:** Groq `whisper-large-v3-turbo` at `$0.04/hour transcribed`.
- **Switch option:** OpenAI `gpt-4o-mini-transcribe` (`$0.18/hr` est.) or `gpt-4o-transcribe` (`$0.36/hr` est.).
- **Why not Realtime:** Realtime is for two-way voice agents; it is heavier and more expensive for pure dictation.

## Data flow
```text
Hotkey → cpal capture → 16kHz mono WAV → multipart upload → transcript → optional mode → clipboard paste
```

## Cost comparison
| Provider | Cost/hour | Notes |
|---|---|---|
| Groq `whisper-large-v3-turbo` | `$0.04` | 10s minimum billing |
| OpenAI `gpt-4o-mini-transcribe` | `$0.18` est. | |
| OpenAI `gpt-4o-transcribe` | `$0.36` est. | |
| OpenAI Realtime | `$1.15+` | Variable; for live agents |
| Local `whisper.cpp` | Free compute | Post-MVP only |

## Privacy model
- Audio leaves the machine only to the user-configured provider.
- API keys stored in OS keyring or environment variables.
- No mandatory cloud relay.
- Audio retention defaults off.
