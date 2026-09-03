import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import {
  cancelRecording,
  clearApiKey,
  disableAutostart,
  duckAudio,
  enableAutostart,
  getInputLevel,
  getSecretStatus,
  getSettings,
  hideOverlay,
  isAutostartEnabled,
  isRecording,
  listMicrophones,
  onHotkeyDown,
  onHotkeyUp,
  openMicrophoneSettings,
  playFeedbackTone,
  restoreAudio,
  saveSettings,
  setApiKey,
  showCompactOverlay,
  showSettingsOverlay,
  startOverlayDrag,
  startRecording,
  stopAndTranscribe,
  unduckAudio,
} from "./lib/tauri";
import type {
  ActivationMode,
  AppSettings,
  DictationMode,
  MicrophoneDevice,
  SecretStatus,
  SpeechProvider,
  TranscriptionOperation,
} from "./lib/types";
import "./styles.css";

const emptySecrets: SecretStatus = {
  groq: false,
  env_groq: false,
};

const PROVIDER: SpeechProvider = "groq";
const PROVIDER_NAME = "Groq";

// The overlay lingers just long enough to read the outcome, then always hides.
export const HIDE_AFTER_SUCCESS_MS = 300;
export const HIDE_AFTER_ERROR_MS = 2600;

// Mic check: poll fast enough to look live, and stop on its own so a forgotten
// test never holds the capture device open.
const MIC_TEST_POLL_MS = 120;
export const MIC_TEST_MAX_MS = 15_000;

const modeLabels: Record<DictationMode, string> = {
  raw: "Raw",
  grammar: "Grammar",
  email: "Email",
  "translate-english": "Translate",
  custom: "Custom",
};

export default function App() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [secrets, setSecrets] = useState<SecretStatus>(emptySecrets);
  const [apiKeyDraft, setApiKeyDraft] = useState("");
  const [recording, setRecording] = useState(false);
  const [busy, setBusy] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [status, setStatus] = useState("Ready");
  const [error, setError] = useState("");
  const [lastOperation, setLastOperation] = useState<TranscriptionOperation | undefined>();
  const [autostartOn, setAutostartOn] = useState(false);
  const [lastError, setLastError] = useState("");
  const [microphones, setMicrophones] = useState<MicrophoneDevice[]>([]);
  const [micTest, setMicTest] = useState({ active: false, peak: 0, device: "" });
  const hideTimer = useRef<number | undefined>(undefined);
  const micTestTimer = useRef<number | undefined>(undefined);
  // Mirrors `micTest.active` for callbacks that must not close over stale state.
  const micTestActive = useRef(false);
  const operationInFlight = useRef(false);
  const startupInFlight = useRef(false);
  const pendingFinish = useRef(false);
  const recordingRef = useRef(false);
  const hotkeyHeld = useRef(false);
  const cancelGeneration = useRef(0);

  const activeProviderSettings = settings?.groq;
  const hasActiveKey = secrets.groq || secrets.env_groq;
  const defaultMicrophoneName = microphones.find((microphone) => microphone.is_default)?.name ?? "";

  // Between "transcription landed" and "overlay hidden" the bar used to carry no
  // state class at all, so it dropped back to the neutral grey styling for a
  // beat before disappearing. Hold an explicit success state instead.
  const done = Boolean(lastOperation) && !error && !busy && !recording;

  const compactStatus = useMemo(() => {
    if (error) return "Error";
    if (busy) return "Creating text";
    if (recording) return "Listening";
    return status;
  }, [busy, error, recording, status]);

  const refresh = useCallback(async () => {
    const [loadedSettings, loadedSecrets, activeRecording, autostart] = await Promise.all([
      getSettings(),
      getSecretStatus(),
      isRecording(),
      isAutostartEnabled().catch(() => false),
    ]);
    setSettings(loadedSettings);
    setSecrets(loadedSecrets);
    recordingRef.current = activeRecording;
    setRecording(activeRecording);
    setAutostartOn(autostart);
    setStatus(activeRecording ? "Listening" : "Ready");
  }, []);

  useEffect(() => {
    void restoreAudio();
    refresh().catch((err: unknown) => setError(formatError(err)));
  }, [refresh]);

  const cancelPendingHide = useCallback(() => {
    if (hideTimer.current === undefined) return;
    window.clearTimeout(hideTimer.current);
    hideTimer.current = undefined;
  }, []);

  // Every terminal state — success *and* failure — has to put the overlay away.
  // Without a hide on the error path the bubble stays pinned on screen and the
  // trigger key looks like a switch that only ever turns on.
  const scheduleHide = useCallback(
    (delayMs: number) => {
      cancelPendingHide();
      hideTimer.current = window.setTimeout(() => {
        hideTimer.current = undefined;
        setError("");
        setStatus("Ready");
        setLastOperation(undefined);
        void hideOverlay();
      }, delayMs);
    },
    [cancelPendingHide],
  );

  useEffect(() => cancelPendingHide, [cancelPendingHide]);

  // A mic check holds the capture device open, so it must never outlive the
  // panel it lives in - nor collide with a real dictation.
  const stopMicTest = useCallback(async () => {
    if (micTestTimer.current !== undefined) {
      window.clearInterval(micTestTimer.current);
      micTestTimer.current = undefined;
    }
    if (!micTestActive.current) return;
    micTestActive.current = false;
    setMicTest({ active: false, peak: 0, device: "" });
    try {
      await cancelRecording();
    } catch (err) {
      console.error("OrcaVoice mic check teardown failed:", formatError(err));
    }
  }, []);

  useEffect(
    () => () => {
      if (micTestTimer.current !== undefined) window.clearInterval(micTestTimer.current);
    },
    [],
  );

  const recoverFromRecordingFailure = useCallback(
    async (err: unknown) => {
      const message = formatError(err);
      console.error("OrcaVoice dictation failed:", message);
      setError(message);
      setLastError(message);
      setStatus("Error");
      recordingRef.current = false;
      setRecording(false);
      scheduleHide(HIDE_AFTER_ERROR_MS);
      const recovery = await Promise.allSettled([cancelRecording(), unduckAudio()]);
      recovery.forEach((result) => {
        if (result.status === "rejected") {
          console.error("OrcaVoice recovery failed:", formatError(result.reason));
        }
      });
    },
    [scheduleHide],
  );

  const finishRecording = useCallback(async () => {
    if (startupInFlight.current) {
      pendingFinish.current = true;
      return;
    }
    if (operationInFlight.current || !recordingRef.current) return;
    operationInFlight.current = true;
    const generation = cancelGeneration.current;

    try {
      cancelPendingHide();
      await unduckAudio();
      await playFeedbackTone("stop");
      await showCompactOverlay();
      setBusy(true);
      recordingRef.current = false;
      setRecording(false);
      setStatus("Creating text");
      const operation = await stopAndTranscribe();
      if (generation !== cancelGeneration.current) return;
      setError("");
      setLastError("");
      setLastOperation(operation);
      setStatus(operation.action ? "Replaced" : operation.pasted ? "Pasted" : "Created");
      scheduleHide(HIDE_AFTER_SUCCESS_MS);
    } catch (err) {
      if (generation === cancelGeneration.current) {
        await recoverFromRecordingFailure(err);
      }
    } finally {
      setBusy(false);
      operationInFlight.current = false;
    }
  }, [cancelPendingHide, recoverFromRecordingFailure, scheduleHide]);

  const beginRecording = useCallback(async () => {
    if (!settings || operationInFlight.current || startupInFlight.current || recordingRef.current) return;
    operationInFlight.current = true;
    startupInFlight.current = true;
    const generation = cancelGeneration.current;

    try {
      cancelPendingHide();
      await stopMicTest();
      setError("");
      setLastOperation(undefined);
      setSettingsOpen(false);
      await showCompactOverlay();
      if (!hasActiveKey) {
        setError(`${PROVIDER_NAME} API key is missing.`);
        setSettingsOpen(true);
        await showSettingsOverlay();
        return;
      }
      await playFeedbackTone("start");
      await startRecording();
      if (generation !== cancelGeneration.current) {
        await cancelRecording();
        return;
      }
      recordingRef.current = true;
      setRecording(true);
      await duckAudio();
      await showCompactOverlay();
      if (generation !== cancelGeneration.current) {
        recordingRef.current = false;
        setRecording(false);
        await Promise.allSettled([cancelRecording(), unduckAudio()]);
        return;
      }
      setStatus("Listening");
    } catch (err) {
      if (generation === cancelGeneration.current) {
        await recoverFromRecordingFailure(err);
      }
    } finally {
      startupInFlight.current = false;
      operationInFlight.current = false;
      if (pendingFinish.current) {
        pendingFinish.current = false;
        if (generation === cancelGeneration.current && recordingRef.current) void finishRecording();
      }
    }
  }, [
    cancelPendingHide,
    finishRecording,
    hasActiveKey,
    recoverFromRecordingFailure,
    settings,
    stopMicTest,
  ]);

  const toggleRecording = useCallback(() => {
    if (startupInFlight.current || recordingRef.current) {
      void finishRecording();
    } else {
      void beginRecording();
    }
  }, [beginRecording, finishRecording]);

  useEffect(() => {
    const activationMode = settings?.activation_mode;
    if (!activationMode) return;

    let disposed = false;
    const disposers: Array<() => void> = [];
    const attach = (listener: Promise<() => void>) =>
      listener.then((dispose) => {
        if (disposed) dispose();
        else disposers.push(dispose);
      });

    void Promise.all([
      attach(
        onHotkeyDown(() => {
          if (hotkeyHeld.current) return;
          hotkeyHeld.current = true;
          if (activationMode === "push-to-talk") void beginRecording();
          else toggleRecording();
        }),
      ),
      attach(
        onHotkeyUp(() => {
          if (!hotkeyHeld.current) return;
          hotkeyHeld.current = false;
          if (activationMode === "push-to-talk") void finishRecording();
        }),
      ),
    ]).catch((err: unknown) => setError(formatError(err)));

    return () => {
      disposed = true;
      hotkeyHeld.current = false;
      disposers.forEach((dispose) => dispose());
    };
  }, [beginRecording, finishRecording, settings?.activation_mode, toggleRecording]);

  async function toggleSettings() {
    // A pending auto-hide must not yank the settings window away mid-edit.
    cancelPendingHide();
    const next = !settingsOpen;
    setSettingsOpen(next);
    setError("");
    if (next) {
      listMicrophones()
        .then(setMicrophones)
        .catch((err: unknown) => console.error("OrcaVoice cannot list microphones:", formatError(err)));
      await showSettingsOverlay();
    } else {
      await stopMicTest();
      await showCompactOverlay();
    }
  }

  /** A live test holds the old device open, so restart it on the new one. */
  async function switchMicrophone(name: string) {
    const wasTesting = micTestActive.current;
    await stopMicTest();
    await persistWithoutClosing({ input_device: name });
    if (wasTesting) await startMicTest();
  }

  /** Proves whether audio actually reaches OrcaVoice, before a real dictation. */
  async function startMicTest() {
    if (recording || busy || micTestActive.current) return;
    setError("");
    try {
      await startRecording();
    } catch (err) {
      setError(formatError(err));
      return;
    }
    micTestActive.current = true;
    setMicTest({ active: true, peak: 0, device: "" });
    const startedAt = Date.now();
    micTestTimer.current = window.setInterval(() => {
      if (Date.now() - startedAt > MIC_TEST_MAX_MS) {
        void stopMicTest();
        return;
      }
      getInputLevel()
        .then((level) => {
          if (!micTestActive.current) return;
          setMicTest((previous) => ({
            active: true,
            device: level.device || previous.device,
            // Decay the previous peak so the bar falls back smoothly instead of
            // flickering between polls.
            peak: Math.max(level.peak, previous.peak * 0.55),
          }));
        })
        .catch(() => {
          /* a poll that loses the race with teardown is not an error */
        });
    }, MIC_TEST_POLL_MS);
  }

  async function persistSettings(nextSettings = settings) {
    if (!nextSettings) return;
    setError("");
    try {
      const saved = await saveSettings(nextSettings);
      setSettings(saved);
      setStatus("Saved");
      setSettingsOpen(false);
      // Collapsing leaves no UI to stop the meter, so release the mic with it.
      await stopMicTest();
      await showCompactOverlay();
    } catch (err) {
      setError(formatError(err));
    }
  }

  /**
   * Save immediately but keep the panel open, for choices the user makes *while*
   * configuring - picking a microphone then testing it must not collapse the UI.
   */
  async function persistWithoutClosing(patch: Partial<AppSettings>) {
    if (!settings) return;
    const nextSettings = { ...settings, ...patch };
    setSettings(nextSettings);
    setError("");
    try {
      setSettings(await saveSettings(nextSettings));
    } catch (err) {
      setError(formatError(err));
    }
  }

  async function saveProviderKey() {
    setError("");
    try {
      const savedSecrets = await setApiKey(PROVIDER, apiKeyDraft);
      setSecrets(savedSecrets);
      setApiKeyDraft("");
      setStatus("Key saved");
    } catch (err) {
      setError(formatError(err));
    }
  }

  async function removeProviderKey() {
    setError("");
    try {
      const savedSecrets = await clearApiKey(PROVIDER);
      setSecrets(savedSecrets);
      setStatus("Key cleared");
    } catch (err) {
      setError(formatError(err));
    }
  }

  async function cancelAndHide() {
    cancelPendingHide();
    cancelGeneration.current += 1;
    pendingFinish.current = false;
    hotkeyHeld.current = false;
    await stopMicTest();
    await cancelRecording();
    recordingRef.current = false;
    await unduckAudio();
    setRecording(false);
    setBusy(false);
    setError("");
    setStatus("Ready");
    setLastError("");
    setLastOperation(undefined);
    setSettingsOpen(false);
    await hideOverlay();
  }

  async function toggleAutostart() {
    const next = !autostartOn;
    try {
      if (next) {
        await enableAutostart();
      } else {
        await disableAutostart();
      }
      setAutostartOn(next);
    } catch (err) {
      setError(formatError(err));
      return;
    }
    // Persist the choice too: startup re-asserts the OS entry from this flag,
    // so without it an opt-out would be undone on the next launch.
    await persistWithoutClosing({ start_on_login: next });
  }

  function updateSettings(patch: Partial<AppSettings>) {
    if (!settings) return;
    setSettings({ ...settings, ...patch });
  }

  function updateProviderSettings(field: "language" | "prompt", value: string) {
    if (!settings) return;
    setSettings({ ...settings, groq: { ...settings.groq, [field]: value } });
  }

  async function updateAndPersistSettings(patch: Partial<AppSettings>) {
    if (!settings) return;
    const nextSettings = { ...settings, ...patch };
    setSettings(nextSettings);
    await persistSettings(nextSettings);
  }

  if (!settings) {
    return <main className="overlay-root compact"><div className="pill-group loading-pill">Actions Preview</div></main>;
  }

  return (
    <main
      className={`overlay-root ${settingsOpen ? "expanded" : "compact"}`}
      style={{ "--border": settings.bubble_outline, "--outline-width": `${settings.outline_width}px` } as CSSProperties}
    >
      <section className={`toolbar-bar ${recording ? "is-recording" : ""} ${busy ? "is-busy" : ""} ${done ? "is-done" : ""} ${error ? "is-error" : ""}`}>
        {/* Pill 1: Drag + Language */}
        <div className="pill-group">
          <button className="icon-btn grab-handle" title="Drag OrcaVoice Actions Preview" onPointerDown={() => void startOverlayDrag()}>
            <svg width="11" height="11" viewBox="0 0 14 14" fill="none">
              <rect x="2" y="1" width="3.2" height="12" rx="1" fill="currentColor" />
              <rect x="8.8" y="1" width="3.2" height="12" rx="1" fill="currentColor" />
            </svg>
          </button>
          <span className="lang-label" title="Language">{(activeProviderSettings?.language === "auto" || !activeProviderSettings?.language) ? "AUTO" : activeProviderSettings.language.toUpperCase()}</span>
        </div>

        {/* Record button — standalone */}
        <button className="record-btn" disabled={busy} onClick={() => void toggleRecording()} title={recording ? "Stop and create text" : "Start recording"}>
          {recording ? (
            <svg width="11" height="11" viewBox="0 0 14 14" fill="none">
              <rect x="2" y="1" width="3.2" height="12" rx="1" fill="currentColor" />
              <rect x="8.8" y="1" width="3.2" height="12" rx="1" fill="currentColor" />
            </svg>
          ) : (
            <svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.3}>
              <rect x="6" y="2" width="4" height="8" rx="2" />
              <path d="M4 8a4 4 0 0 0 8 0" />
              <line x1="8" y1="12" x2="8" y2="14" />
              <line x1="5.5" y1="14" x2="10.5" y2="14" />
            </svg>
          )}
        </button>

        {/* Pill 2: Mode + Provider + Status */}
        <div className="pill-group">
          <button className="mode-circle" title={`Mode: ${modeLabels[settings.mode]}`} onClick={() => void updateAndPersistSettings({ mode: nextMode(settings.mode) })}>
            {modeInitial(settings.mode)}
          </button>
          <span className="provider-circle is-static" title={`Provider: ${PROVIDER_NAME}`}>G</span>
          <span className="status-dot" title={error || compactStatus} />
        </div>

        {/* Pill 3: Settings + Cancel */}
        <div className="pill-group">
          <button className="icon-btn" onClick={() => void toggleSettings()} title="Settings">
            <svg width="15" height="4" viewBox="0 0 15 4">
              <circle cx="2" cy="2" r="1.6" fill="currentColor" />
              <circle cx="7.5" cy="2" r="1.6" fill="currentColor" />
              <circle cx="13" cy="2" r="1.6" fill="currentColor" />
            </svg>
          </button>
          <button className="icon-btn" onClick={() => void cancelAndHide()} title="Cancel and hide">
            <svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.3}>
              <path d="M3 4.5h10M6.3 4.5V3a1 1 0 0 1 1-1h1.4a1 1 0 0 1 1 1v1.5M4.3 4.5l.6 8.6a1 1 0 0 0 1 .9h4.2a1 1 0 0 0 1-.9l.6-8.6" />
            </svg>
          </button>
        </div>
      </section>

      {settingsOpen ? (
        <section className="settings-popover">
          <div className="popover-header">
            <strong>OrcaVoice Actions Preview</strong>
            <div className="header-right">
              <span>{compactStatus}</span>
              <button className="close-button" title="Close" onClick={() => void toggleSettings()}>✕</button>
            </div>
          </div>

          {error || lastError ? (
            <div className="mini-error">
              {error || lastError}
              {isSilenceError(error || lastError) ? (
                <button className="link-button" onClick={() => void openMicrophoneSettings()}>
                  Open Windows microphone settings
                </button>
              ) : null}
            </div>
          ) : null}
          {lastOperation ? (
            <div className="mini-result">
              {lastOperation.action ? <span className="enhanced-badge">Action</span> : lastOperation.result.enhanced ? <span className="enhanced-badge">Enhanced</span> : <span className="raw-badge">Raw</span>}
              {" "}
              {lastOperation.result.text}
            </div>
          ) : null}

          <label>
            Microphone
            <select
              value={settings.input_device}
              onChange={(event) => void switchMicrophone(event.target.value)}
            >
              <option value="">
                System default{defaultMicrophoneName ? ` (${defaultMicrophoneName})` : ""}
              </option>
              {microphones.map((microphone) => (
                <option key={microphone.name} value={microphone.name}>
                  {microphone.name}
                </option>
              ))}
            </select>
          </label>

          <div className="mic-check">
            <button className="mic-check-button" disabled={recording || busy} onClick={() => void (micTest.active ? stopMicTest() : startMicTest())}>
              {micTest.active ? "Stop test" : "Test microphone"}
            </button>
            <div
              className="level-meter"
              role="meter"
              aria-label="Microphone level"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round(micTest.peak * 100)}
            >
              <span className="level-fill" style={{ width: `${Math.min(100, Math.round(micTest.peak * 140))}%` }} />
            </div>
            <span className="level-hint">
              {micTest.active ? (micTest.peak > 0.02 ? "Hearing you" : "No signal — speak up") : "Speak to check input"}
            </span>
          </div>

          <label>
            Mode
            <select value={settings.mode} onChange={(event) => updateSettings({ mode: event.target.value as DictationMode })}>
              <option value="raw">Raw dictation</option>
              <option value="grammar">Correct grammar</option>
              <option value="email">Email</option>
              <option value="translate-english">Translate to English</option>
              <option value="custom">Custom</option>
            </select>
          </label>

          <label>
            Activation
            <select
              aria-label="Activation"
              value={settings.activation_mode}
              onChange={(event) => updateSettings({ activation_mode: event.target.value as ActivationMode })}
            >
              <option value="toggle">Press to start or stop</option>
              <option value="push-to-talk">Hold to record</option>
            </select>
          </label>

          <label className="check-row">
            <input
              type="checkbox"
              checked={settings.selection_actions_enabled}
              onChange={(event) => updateSettings({ selection_actions_enabled: event.target.checked })}
            />
            <span className="check-copy">
              Transform selected text
              <small>Windows only. When enabled, selected text is sent to Groq with your spoken instruction.</small>
            </span>
          </label>

          <label>
              Language
              <select
                value={activeProviderSettings?.language || "en"}
                onChange={(event) => updateProviderSettings("language", event.target.value)}
              >
                <option value="auto">Auto-detect</option>
                <option value="en">English</option>
                <option value="es">Spanish</option>
                <option value="fr">French</option>
                <option value="de">German</option>
                <option value="it">Italian</option>
                <option value="pt">Portuguese</option>
                <option value="nl">Dutch</option>
                <option value="ru">Russian</option>
                <option value="ja">Japanese</option>
                <option value="ko">Korean</option>
                <option value="zh">Chinese</option>
                <option value="hi">Hindi</option>
                <option value="ar">Arabic</option>
                <option value="tr">Turkish</option>
                <option value="pl">Polish</option>
                <option value="uk">Ukrainian</option>
              </select>
          </label>

          <label>
            Custom vocabulary
            <input
              type="text"
              placeholder="names, jargon, acronyms (comma-separated)"
              value={activeProviderSettings?.prompt || ""}
              onChange={(event) => updateProviderSettings("prompt", event.target.value)}
            />
          </label>

          <div className="two-mini-fields">
            <label>
              Trigger key
              <input value={settings.hotkey} onChange={(event) => updateSettings({ hotkey: event.target.value })} />
            </label>
            <label>
              Outline
              <input type="color" value={settings.bubble_outline} onChange={(event) => updateSettings({ bubble_outline: event.target.value })} />
            </label>
          </div>

          <label>
            Outline width ({settings.outline_width}px)
            <input
              type="range"
              min={0}
              max={4}
              step={1}
              value={settings.outline_width}
              onChange={(event) => updateAndPersistSettings({ outline_width: Number(event.target.value) })}
            />
          </label>

          <label>
            Enhancement model
            <select
              value={settings.enhancement_model}
              onChange={(event) => updateSettings({ enhancement_model: event.target.value })}
            >
              <option value="llama-4-scout-17b-16e-instruct">Scout 17B — fast</option>
              <option value="llama-3.3-70b-versatile">70B Versatile — quality</option>
            </select>
          </label>

          <label>
            {PROVIDER_NAME} API key
            <div className="key-row">
              <input
                type="password"
                placeholder={hasActiveKey ? "••••••••••••••••" : "gsk_..."}
                value={apiKeyDraft}
                onChange={(event) => setApiKeyDraft(event.target.value)}
              />
              <button onClick={() => void saveProviderKey()}>Save</button>
            </div>
          </label>

          <label className="check-row">
            <input
              type="checkbox"
              checked={autostartOn}
              onChange={() => void toggleAutostart()}
            />
            Launch Actions Preview on system startup
          </label>

          <div className="popover-actions">
            <span className={hasActiveKey ? "key-ok" : "key-missing"}>{hasActiveKey ? "Key saved" : "Key missing"}</span>
            <button onClick={() => void removeProviderKey()}>Clear key</button>
            <button className="primary-mini" onClick={() => void persistSettings()}>Save settings</button>
          </div>
        </section>
      ) : null}
    </main>
  );
}

function nextMode(mode: DictationMode): DictationMode {
  const modes: DictationMode[] = ["raw", "grammar", "email", "translate-english", "custom"];
  return modes[(modes.indexOf(mode) + 1) % modes.length];
}

function modeInitial(mode: DictationMode) {
  return modeLabels[mode].slice(0, 1);
}

function formatError(err: unknown) {
  if (err instanceof Error) return err.message;
  return String(err);
}

/** Only dead-air failures are fixable from the Windows privacy page. */
function isSilenceError(message: string) {
  return /silent|no audio/i.test(message);
}
