# OrcaVoice

A minimal, always-available speech-to-text dictation overlay for Windows, macOS, and Linux.

Press a key. Speak. Text appears wherever your cursor is.

**Version 0.2.3** · Last updated 12 August 2026

![OrcaVoice overlay](docs/orcavoice-overlay.png)

## How it works

OrcaVoice lives hidden in your system tray. Press your trigger key (default: `\`) and a tiny floating bubble appears. Speak naturally. Press the key again and your words are transcribed and pasted into whatever app has focus.

**Toolbar controls:** drag · language · mic · mode · provider · status · settings · cancel

## Features

- **Invisible until needed** — hidden in the tray, summoned by a single key
- **Groq Whisper Large v3 Turbo** for sub-second transcription
- **One-key toggle** — press `\` to start, press again to stop and paste
- **Audio feedback** — native start/stop WAV sound so you know it's listening
- **System audio ducking** — mutes other app audio while recording, then restores it on stop
- **Visual feedback** — pulsing status dot and border glow during recording
- **Movable** — drag the bubble anywhere with the grab handle
- **Customizable outline** — pick your own bubble accent color
- **Microphone picker + live level meter** — choose your input and prove it works before dictating
- **Auto-start on boot** — on by default, re-asserted at every launch so an upgrade cannot silently drop it
- **Single instance** — a second launch surfaces the running app instead of fighting it for the hotkey
- **API keys stored securely** — OS keyring + settings.json fallback
- **Five dictation modes** — Raw, Grammar, Email, Translate, Custom (all polished by Groq's LLM)

## Setup

### Prerequisites

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://rustup.rs/) 1.77+
- System microphone access

### Install dependencies

```bash
npm install
```

### Run in development

```bash
npm run tauri:dev
```

### Build for production

```bash
npm run tauri:build              # installers + exe
npm run tauri:build -- --no-bundle   # exe only, faster
```

### Run the tests

```bash
npm run test:ui        # vitest — overlay lifecycle (jsdom, mocked Tauri IPC)
npm run test:ui:watch  # same, in watch mode
npm test               # build + vitest + cargo test (needs cargo on PATH)
```

`src/App.test.tsx` drives the real component through a fake Tauri backend
(`src/test/tauri-backend-mock.ts`) that mocks `@tauri-apps/api/core`'s `invoke`.
It locks in the overlay lifecycle: the bubble hides after a **failed**
transcription as well as a successful one, a new recording cancels a pending
hide, and a hotkey press during an in-flight transcription is ignored rather
than restarting the recorder.

> **Never build with bare `cargo build --release`.**
>
> Tauri selects between the embedded frontend and the dev server with
> `dev = !custom-protocol`. A plain cargo release build omits that feature, so it
> produces a *dev* binary that loads `http://localhost:1420` instead of its own
> bundled UI. It then renders whatever a stray dev server is serving — or a blank
> error page once that server stops.
>
> `src-tauri/build.rs` now fails the build with an explanatory panic if this is
> ever attempted. If you must use cargo directly, pass
> `--features tauri/custom-protocol`.

## Getting an API key

OrcaVoice uses Groq for both transcription and mode polishing, so it needs one key.

1. Go to [console.groq.com](https://console.groq.com/keys)
2. Create an API key (starts with `gsk_`)
3. Open OrcaVoice Settings (`···` button)
4. Paste your key and click **Save**

The key is stored in the OS keyring, with `settings.json` as a fallback. You can
also supply it via the `GROQ_API_KEY` environment variable, which takes priority.

## Using OrcaVoice

1. Press `\` (or your configured trigger key) — the bubble appears and the start sound plays
2. Speak naturally — other app audio is muted while recording
3. Press `\` again — the stop sound plays, audio is restored, and text is transcribed/pasted

### Sound effects and audio ducking

This part was deliberately built around the working Windows behavior:

- **SFX playback:** `src-tauri/src/feedback.rs`
- **SFX assets:** `src-tauri/resources/sfx-start.wav` and `src-tauri/resources/sfx-stop.wav`
- **Current behavior:** start feedback uses the end/done WAV pitched up 15%; stop feedback uses the original end/done WAV
- **Playback API:** Windows `PlaySoundW` (non-blocking `SND_ASYNC`) with an embedded WAV copied to the temp directory, so feedback never adds latency to start/stop
- **Ducking:** `src-tauri/src/ducking.rs` mutes other process audio sessions after the start SFX plays
- **Recovery:** app launch, stop, cancel, and error paths call audio recovery so sessions do not stay muted

To change the sound effect, replace `src-tauri/resources/sfx-stop.wav` with a short PCM WAV file, then regenerate `src-tauri/resources/sfx-start.wav` as the 15% pitch-up variant. Recommended format: mono, 44.1 kHz, 16-bit PCM, under 500 ms.

Keep this order for reliable behavior:

```text
Start: play SFX → start recording → duck other audio
Stop: restore other audio → play SFX → transcribe/paste
```

Avoid replacing this with WebAudio or `Beep()`:

- WebAudio can be silent from the global-hotkey path.
- `Beep()` is audible but harsh/distorted and has no useful volume control.
- Ducking before the start SFX can mute the SFX itself.

### Settings popover

Click `···` on the bubble to access:
- **Microphone** — pick an input device, or follow the Windows default
- **Test microphone** — a live level meter that proves audio is reaching OrcaVoice (auto-stops after 15s)
- **Mode** — Raw, Grammar correction, Email formatting, Translate to English, Custom
- **Language** — transcription language, or auto-detect
- **Custom vocabulary** — bias the transcript toward names and jargon you use
- **Enhancement model** — Groq LLM used to polish non-raw modes
- **Trigger key** — change the global hotkey
- **Outline color** — customize the bubble's accent border
- **Auto-start** — toggle launch on system startup
- **API key** — paste, save, or clear your Groq key

### Dictation modes

`Raw` returns the transcript untouched. Every other mode sends the transcript to a
Groq LLM (default `llama-4-scout-17b-16e-instruct`) with a mode-specific system
prompt. Polishing has a 5-second timeout and **falls back to the raw transcript**
rather than failing the dictation.

## Troubleshooting

### The overlay is blank, black, or shows an old UI

The binary was built with bare `cargo build --release`, so it is a dev build
loading `http://localhost:1420` instead of its embedded frontend. Rebuild with
`npm run tauri:build`. The build now refuses to produce this binary.

### The app starts but nothing appears

OrcaVoice starts hidden by design — look for the tray icon and press `\`. If the
tray icon is missing entirely, the app failed before `setup()` finished; run it
from a terminal to read the error.

### The overlay stays on screen after the second key press

**First, confirm you are running a build that contains the fix.** `npm test`
rebuilds `dist/` but never the installed binary, so a frontend fix does not
reach the app you actually use until you rebuild and reinstall:

```bash
ls -la "$LOCALAPPDATA/OrcaVoice/orcavoice.exe"   # is this older than your change?
npm run tauri:build
# then run target/release/bundle/nsis/OrcaVoice_<version>_x64-setup.exe
```

Three separate defects produced this symptom:

1. **No hide on the error path.** Only the success branch called
   `hideOverlay()`, so a silent/short recording or a Groq failure left the
   bubble pinned open with no visible message. `src/App.tsx` now routes success
   *and* failure through one cancellable `scheduleHide` timer.
2. **Key-repeat force-showing the window.** Windows repeats `WM_HOTKEY` while
   the trigger key is held, and `global-hotkey` emits a `Pressed` event per
   repeat. `hotkey.rs` called `window.show()` on every one — an "on" with no
   "off". It is now edge-triggered: one physical press is one toggle.
3. **A wedged in-flight latch.** The re-entrancy guard was set before an
   `await` outside its `try/finally`, so a single rejected IPC call left it
   stuck and silently swallowed every later press.

`src/App.test.tsx` and `src-tauri/src/hotkey.rs` tests fail if any of these
regress.

### "Recording looks silent" when you know you were speaking

Until 0.2.3 this was usually a **false alarm**: the silence check compared the
recording's *average* level (RMS) against a threshold of 80. Normal speech
contains pauses, so a perfectly good dictation averages well below that and was
rejected locally — it never reached Groq.

The check is now based on **peak amplitude** (~-54 dBFS, below a typical mic's
noise floor), so quiet-but-real audio is accepted and only true dead air is
refused. The error now also names the device and the measured level.

If it still reports silence, audio genuinely is not arriving:

1. Open Settings → **Test microphone** and speak. A flat meter means no signal.
2. Use the **Open Windows microphone settings** link on the error. Windows feeds
   blocked apps an all-zero stream rather than failing, so a privacy block looks
   exactly like a dead mic.
3. Pick the right device in the **Microphone** dropdown — the default input is
   often a webcam or monitor mic, not the one you are talking into.

### The settings panel is cut off at the bottom

Fixed in 0.2.3. The window was capped at 540px tall and grew *downward* from a
bubble sitting near the taskbar, so the lower rows (including the Groq API key)
fell off-screen with no way to reach them. The panel is now taller, clamped to
the monitor work area, moved back on-screen when it expands, and it scrolls
internally with a sticky header — so no row can be unreachable.

### The hotkey does nothing

Another process already owns `\`. Check for leftover OrcaVoice processes:

```bash
tasklist | grep -i orcavoice     # Windows
```

Duplicate instances used to cause this; the single-instance plugin now prevents
it, but a stale binary from an older build can still squat the key.

### Every dictation pastes the text but then flashes red

The transcript is fine; something *after* the paste failed. The known cause was
a corrupt `history.json` — a crash mid-write leaves a correct-length file full
of NUL bytes — which made `stop_and_transcribe` return an error even though the
paste had already happened. The overlay then showed its error state and waited
out the error timer, which read as "it records, goes grey, goes red, then
closes".

This can no longer fail a dictation:

- `append_history` failures are logged and reported as `entry: null`; the
  dictation still succeeds.
- An unreadable `history.json` or `settings.json` is moved aside to
  `<name>.corrupt-<timestamp>` and the app starts clean instead of failing on
  the same bytes forever.
- Both files are written atomically (temp + fsync + rename), so an unclean
  shutdown cannot produce a half-written file in the first place.

Quarantined files stay in `%APPDATA%/com.squarecirclelabs.orcavoice/`.

### Settings look wrong after an upgrade

`settings.json` is rewritten on load whenever it differs from the canonical
shape, so retired keys are dropped automatically. Delete it to reset:
`%APPDATA%/com.squarecirclelabs.orcavoice/settings.json`.

## Tech stack

| Layer | Technology |
|-------|-----------|
| Desktop shell | Tauri v2 |
| Backend | Rust (cpal, reqwest, enigo, keyring) |
| Frontend | React + TypeScript + Vite |
| STT | Groq Whisper Large v3 Turbo |
| Mode polishing | Groq LLM (Llama 4 Scout / Llama 3.3 70B) |
| Clipboard paste | Enigo keyboard simulation |

## Privacy

Audio is captured locally and sent **only** when you stop recording, directly to Groq. No intermediate servers. No audio retention. Transcription history is stored locally in your app data directory.

In any mode other than Raw, the resulting *text* is also sent to Groq for polishing.

## Changelog

### 0.2.3 — 12 August 2026

- **Fixed:** "Recording looks silent" rejected real dictation. The gate compared
  *average* RMS against 80; speech with normal pauses falls below that, so valid
  audio was discarded before it ever reached Groq. It now gates on **peak
  amplitude** (~-54 dBFS) and names the device and measured level in the error.
- **Fixed:** the settings panel was clipped at the bottom, hiding the Groq API
  key row. The window was capped at 540px and grew downward from a bubble near
  the taskbar. It is now taller, clamped to the monitor work area, pulled back
  on-screen when expanding, restored to its previous spot on collapse, and
  scrollable with a sticky header.
- **Added:** microphone picker (`input_device`), which falls back to the system
  default if the chosen device disappears.
- **Added:** **Test microphone** with a live level meter, plus a one-click link
  to Windows microphone privacy settings on silence errors. The test auto-stops
  after 15s and is torn down whenever the panel closes or a dictation starts, so
  it can never hold the capture device open.
- Capture now accepts I8/I16/I32/F32 microphones, downmixes to mono and
  resamples to 16 kHz for Groq.
- **Auto-start is on by default** and re-asserted from `start_on_login` at every
  launch, so a reinstall cannot silently drop it. Turning it off persists.
  Dev builds skip this, so `target\debug` is never registered for login.

### 0.2.2 — 28 July 2026

- **Fixed:** every dictation pasted its text and then flashed red. `history.json`
  had been corrupted by an unclean shutdown (a correct-length file of NUL
  bytes), and history is appended *after* the paste, so the whole command
  returned an error even though the dictation had succeeded.
- History writes are now best-effort: a failure is logged and reported as
  `entry: null` instead of failing the dictation.
- `history.json` and `settings.json` are written atomically (temp + fsync +
  rename) and quarantined to `<name>.corrupt-<timestamp>` if unreadable.
- The overlay no longer drops to its neutral grey styling between transcribing
  and hiding; the sequence is now record → transcribe → success → hide.

### 0.2.1 — 28 July 2026

- **Fixed:** the overlay stayed pinned open after the second key press. Three
  separate defects contributed: the error path never hid the bubble, Windows
  key-repeat re-showed it on every `WM_HOTKEY`, and a re-entrancy latch could
  wedge and swallow all later presses.
- Global hotkey is edge-triggered: one physical press is exactly one toggle.
- Added the frontend test suite (`npm run test:ui`).

### 0.2.0

- Groq-only provider, dictation modes, audio feedback and ducking, tray icon,
  autostart, single-instance enforcement.

## License

MIT — SquareCircle Labs
