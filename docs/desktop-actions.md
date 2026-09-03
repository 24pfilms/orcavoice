# Desktop actions

Desktop actions let OrcaVoice transform text in the application where a recording started. The feature is **Windows-only and off by default**.

## Controls

- **Toggle** (default): press the trigger key once to start and once to stop.
- **Push-to-talk**: hold the trigger key to record and release it to stop. If release arrives while recording is still starting, OrcaVoice queues the stop.
- **Transform selected text**: opt in under Settings. The setting states that selected text is sent to Groq.

The record button remains a start/stop toggle in either activation mode.

## Dictation versus an action

At recording start, OrcaVoice remembers the active window. When **Transform selected text** is enabled, it also asks Windows UI Automation for the focused control's selected text.

- With readable, non-empty selected text, speech is the instruction. For example, select a paragraph, hold the trigger, say “make this concise,” and release. Groq receives the selected text and spoken instruction as separate data and returns only replacement text. The normal Grammar, Email, Translate, or Custom mode is not applied again.
- Without readable selected text, OrcaVoice follows the configured dictation mode exactly as before.
- Controls that do not expose a UI Automation text selection degrade to ordinary dictation; selection capture failure does not crash or block dictation.

## Privacy boundary

Selected text leaves the machine **only when the opt-in is enabled and a readable non-empty selection exists**. It is sent directly through OrcaVoice's existing Groq endpoint and API-key path.

OrcaVoice does not log or persist the source selection. History contains the spoken instruction and returned replacement, not the selected source. A selection over **16,000 Unicode characters** is rejected before network submission; OrcaVoice does not send or insert a partial selection.

Selected text is untrusted data in a fixed no-tools prompt. OrcaVoice asks the model to obey only the spoken instruction. Model output is treated only as text for bounded, chunked insertion; it cannot invoke a shell, access files, open URLs, or call tools.

## Target and clipboard safety

Before output, OrcaVoice validates the original window, asks Windows to foreground it, and verifies that it really became foreground. If the window vanished or focus cannot be verified, the operation fails without typing into another app.

Windows output uses chunked Unicode `SendInput`; it never opens the clipboard. Existing text, images, and copied-file clipboard formats remain untouched. Newlines and UTF-16 surrogate pairs are emitted explicitly, and partial insertion is reported as an error rather than retried against an unverified window.

macOS and Linux retain the existing clipboard-plus-paste path. They do not capture desktop selections.

## Known limits

- UI Automation selection support depends on the target control. Standard Notepad fields, browser textareas, and VS Code editors are expected targets; custom controls may fall back to dictation.
- Windows integrity isolation can reject `SendInput` into an application running as administrator when OrcaVoice is not elevated. OrcaVoice reports the failure and does not use an elevated helper.
- A target that closes, cannot be foregrounded, or loses foreground verification causes a fail-closed output error.
- Network timeout or Groq transformation failure leaves the selected source unchanged.

## Verification checklist

Before deleting local build artifacts, verify on Windows:

1. Toggle dictation and push-to-talk work in Notepad, a browser textarea, and VS Code.
2. A selected-text instruction replaces only the selection in all three; no selection still dictates normally.
3. Text, image, and copied-file clipboard contents are byte-for-byte unchanged after Windows output.
4. Switching windows during transcription restores the original target or fails without typing elsewhere.
5. Unsupported controls, oversized selections, elevated or vanished targets, network timeout, and rapid press/release leave no stale recording or selection.

Also recheck the tray, settings, autostart, microphone meter, audio ducking, history, and overlay hide timers.

## Recovery and rollback

The active tree is reproducible from its lockfiles:

```bash
npm ci
npm run tauri:build
```

`OrcaVoiceUp` is the untouched `master` rollback copy at commit `59c4ea0d78195680c0258e9593218c79f8363273`. Do not run builds, cleanup, formatters, or package installation in that backup.

## Design research attribution

[OpenWhispr](https://github.com/OpenWhispr/openwhispr) was reviewed as MIT-licensed design research for Windows desktop behavior. OrcaVoice copies no OpenWhispr source and bundles no OpenWhispr helper or executable, so no imported license file is required.
