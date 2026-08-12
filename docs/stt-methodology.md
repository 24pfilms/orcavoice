# OrcaVoice — STT Methodology

## Blabby's public claim
Blabby's website says "Powered by OpenAI's Whisper v3 Turbo." Blabby does not publish source code.

## OrcaVision's current stack
OrcaVision uses OpenAI Realtime (`gpt-realtime-2`) for live voice conversation, with `gpt-4o-transcribe` for input transcript events. This is a **live assistant** pattern, not dictation.

## OrcaVoice choice
- **Only provider:** Groq `whisper-large-v3-turbo` at `$0.04/hour transcribed`.
- **Why not Realtime:** Realtime is for two-way voice agents; it is heavier and more expensive for pure dictation.

## Data flow
```text
Hotkey → cpal capture → 16kHz mono WAV → multipart upload → transcript
  → optional Groq LLM polish → clipboard paste
```

## Removed providers
OrcaVoice originally shipped provider switching and a side-by-side benchmark.
Both were removed — Groq was the fastest and cheapest option by a wide margin, and
the alternatives added settings surface, secrets, and UI for no practical gain.

| Provider | Cost/hour | Status |
|---|---|---|
| Groq `whisper-large-v3-turbo` | `$0.04` | **In use.** 10s minimum billing |
| OpenAI `gpt-4o-mini-transcribe` | `$0.18` est. | Removed |
| OpenAI `gpt-4o-transcribe` | `$0.36` est. | Removed |
| OpenAI Realtime | `$1.15+` | Rejected; for live agents |
| Parakeet (`parakeet-tdt-0.6b`, local endpoint) | Free compute | Removed; never implemented in the app |
| Local `whisper.cpp` | Free compute | Not planned |

## Mode polishing
Non-raw dictation modes send the transcript to a Groq chat model (default
`llama-4-scout-17b-16e-instruct`) with a mode-specific system prompt. The call has
a 5-second timeout and falls back to the raw transcript on any failure, so
polishing can never lose a dictation. Estimated cost is ~110 tokens per dictation,
which is negligible next to the STT cost.

## Privacy model
- Audio leaves the machine only to Groq, only when you stop recording.
- In non-raw modes the transcript text is also sent to Groq for polishing.
- API key stored in the OS keyring, `settings.json` fallback, or `GROQ_API_KEY`.
- No mandatory cloud relay.
- Audio retention defaults off.
