import { useCallback, useEffect, useMemo, useState, type CSSProperties } from "react";
import {
  cancelRecording,
  clearApiKey,
  disableAutostart,
  enableAutostart,
  getSecretStatus,
  getSettings,
  hideOverlay,
  isAutostartEnabled,
  isRecording,
  onHotkeyToggle,
  saveSettings,
  setApiKey,
  showCompactOverlay,
  showSettingsOverlay,
  startOverlayDrag,
  startRecording,
  stopAndTranscribe,
} from "./lib/tauri";
import type { AppSettings, DictationMode, SecretStatus, SpeechProvider, TranscriptionOperation } from "./lib/types";
import "./styles.css";

const emptySecrets: SecretStatus = {
  groq: false,
  openai: false,
  env_groq: false,
  env_openai: false,
};

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
  const [apiKeyDrafts, setApiKeyDrafts] = useState<Record<SpeechProvider, string>>({ groq: "", "open-ai": "" });
  const [recording, setRecording] = useState(false);
  const [busy, setBusy] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [status, setStatus] = useState("Ready");
  const [error, setError] = useState("");
  const [lastOperation, setLastOperation] = useState<TranscriptionOperation | undefined>();
  const [autostartOn, setAutostartOn] = useState(false);

  const activeProvider = settings?.active_provider ?? "groq";
  const activeProviderSettings = activeProvider === "groq" ? settings?.groq : settings?.openai;
  const providerName = activeProvider === "groq" ? "Groq" : "OpenAI";
  const hasActiveKey = activeProvider === "groq" ? secrets.groq || secrets.env_groq : secrets.openai || secrets.env_openai;

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
    setRecording(activeRecording);
    setAutostartOn(autostart);
    setStatus(activeRecording ? "Listening" : "Ready");
  }, []);

  useEffect(() => {
    refresh().catch((err: unknown) => setError(formatError(err)));
  }, [refresh]);

  const toggleRecording = useCallback(async () => {
    if (!settings) return;
    setError("");
    setLastOperation(undefined);
    setSettingsOpen(false);
    await showCompactOverlay();

    try {
      const backendRecording = await isRecording();
      if (backendRecording) {
        playTone(420, 90);
        await showCompactOverlay();
        setBusy(true);
        setRecording(false);
        setStatus("Creating text");
        const operation = await stopAndTranscribe();
        setError("");
        setLastOperation(operation);
        setStatus(operation.pasted ? "Pasted" : "Copied");
        window.setTimeout(() => {
          void hideOverlay();
          setError("");
          setStatus("Ready");
          setLastOperation(undefined);
        }, 900);
      } else {
        if (!hasActiveKey) {
          setError(`${providerName} API key is missing.`);
          setSettingsOpen(true);
          await showSettingsOverlay();
          return;
        }
        await startRecording();
        playTone(660, 70);
        await showCompactOverlay();
        setRecording(true);
        setStatus("Listening");
      }
    } catch (err) {
      setError(formatError(err));
      setStatus("Error");
      await showCompactOverlay();
      await cancelRecording();
      setRecording(false);
    } finally {
      setBusy(false);
    }
  }, [hasActiveKey, providerName, settings]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onHotkeyToggle(() => {
      void toggleRecording();
    })
      .then((dispose) => {
        unlisten = dispose;
      })
      .catch((err: unknown) => setError(formatError(err)));
    return () => unlisten?.();
  }, [toggleRecording]);

  async function toggleSettings() {
    const next = !settingsOpen;
    setSettingsOpen(next);
    setError("");
    if (next) {
      await showSettingsOverlay();
    } else {
      await showCompactOverlay();
    }
  }

  async function persistSettings(nextSettings = settings) {
    if (!nextSettings) return;
    setError("");
    try {
      const saved = await saveSettings(nextSettings);
      setSettings(saved);
      setStatus("Saved");
    } catch (err) {
      setError(formatError(err));
    }
  }

  async function saveProviderKey(provider: SpeechProvider) {
    setError("");
    try {
      const savedSecrets = await setApiKey(provider, apiKeyDrafts[provider]);
      setSecrets(savedSecrets);
      setApiKeyDrafts((current) => ({ ...current, [provider]: "" }));
      setStatus("Key saved");
    } catch (err) {
      setError(formatError(err));
    }
  }

  async function removeProviderKey(provider: SpeechProvider) {
    setError("");
    try {
      const savedSecrets = await clearApiKey(provider);
      setSecrets(savedSecrets);
      setStatus("Key cleared");
    } catch (err) {
      setError(formatError(err));
    }
  }

  async function cancelAndHide() {
    await cancelRecording();
    setRecording(false);
    setBusy(false);
    setError("");
    setStatus("Ready");
    setLastOperation(undefined);
    setSettingsOpen(false);
    await hideOverlay();
  }

  async function toggleAutostart() {
    try {
      if (autostartOn) {
        await disableAutostart();
        setAutostartOn(false);
      } else {
        await enableAutostart();
        setAutostartOn(true);
      }
    } catch {
      // best-effort
    }
  }

  function updateSettings(patch: Partial<AppSettings>) {
    if (!settings) return;
    setSettings({ ...settings, ...patch });
  }

  async function updateAndPersistSettings(patch: Partial<AppSettings>) {
    if (!settings) return;
    const nextSettings = { ...settings, ...patch };
    setSettings(nextSettings);
    await persistSettings(nextSettings);
  }

  if (!settings) {
    return <main className="overlay-root compact"><div className="toolbar loading">OrcaVoice</div></main>;
  }

  return (
    <main
      className={`overlay-root ${settingsOpen ? "expanded" : "compact"}`}
      style={{ "--border": settings.bubble_outline } as CSSProperties}
    >
      <section className={`toolbar ${recording ? "is-recording" : ""} ${busy ? "is-busy" : ""} ${error ? "is-error" : ""}`}>
        <button className="grab-handle" title="Drag OrcaVoice" onPointerDown={() => void startOverlayDrag()}>
          <span />
        </button>
        <button className="lang-chip" title="Language">{activeProviderSettings?.language || "en"}</button>
        <button className="record-button" disabled={busy} onClick={() => void toggleRecording()} title={recording ? "Stop and create text" : "Start recording"}>
          <span className={recording ? "stop-glyph" : "mic-glyph"} />
        </button>
        <button className="mode-dot" title={`Mode: ${modeLabels[settings.mode]}`} onClick={() => void updateAndPersistSettings({ mode: nextMode(settings.mode) })}>
          <span>{modeInitial(settings.mode)}</span>
        </button>
        <button className="provider-chip" title={`Provider: ${providerName}`} onClick={() => void updateAndPersistSettings({ active_provider: activeProvider === "groq" ? "open-ai" : "groq" })}>
          {activeProvider === "groq" ? "G" : "O"}
        </button>
        <button className="status-orb" title={compactStatus}>
          <span />
        </button>
        <button className="settings-button" onClick={() => void toggleSettings()} title="Settings">···</button>
        <button className="trash-button" onClick={() => void cancelAndHide()} title="Cancel and hide">
          <span />
        </button>
      </section>

      {settingsOpen ? (
        <section className="settings-popover">
          <div className="popover-header">
            <strong>OrcaVoice</strong>
            <div className="header-right">
              <span>{compactStatus}</span>
              <button className="close-button" title="Close" onClick={() => void toggleSettings()}>✕</button>
            </div>
          </div>

          {error ? <div className="mini-error">{error}</div> : null}
          {lastOperation ? (
            <div className="mini-result">
              {lastOperation.result.is_command ? (
                <span className="command-badge">Command</span>
              ) : lastOperation.result.enhanced ? (
                <span className="enhanced-badge">Enhanced</span>
              ) : (
                <span className="raw-badge">Raw</span>
              )}
              {" "}
              {lastOperation.result.text}
            </div>
          ) : null}

          <label>
            Provider
            <select
              value={settings.active_provider}
              onChange={(event) => updateSettings({ active_provider: event.target.value as SpeechProvider })}
            >
              <option value="groq">Groq — fast Whisper</option>
              <option value="open-ai">OpenAI — transcribe</option>
            </select>
          </label>

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
            {providerName} API key
            <div className="key-row">
              <input
                type="password"
                placeholder={hasActiveKey ? "••••••••••••••••" : activeProvider === "groq" ? "gsk_..." : "sk-..."}
                value={apiKeyDrafts[activeProvider]}
                onChange={(event) => setApiKeyDrafts((current) => ({ ...current, [activeProvider]: event.target.value }))}
              />
              <button onClick={() => void saveProviderKey(activeProvider)}>Save</button>
            </div>
          </label>

          <label className="check-row">
            <input
              type="checkbox"
              checked={autostartOn}
              onChange={() => void toggleAutostart()}
            />
            Launch OrcaVoice on system startup
          </label>

          <label className="check-row">
            <input
              type="checkbox"
              checked={settings.voice_commands_enabled}
              onChange={() => void updateAndPersistSettings({ voice_commands_enabled: !settings.voice_commands_enabled })}
            />
            Voice commands ("open [app]")
          </label>

          <div className="popover-actions">
            <span className={hasActiveKey ? "key-ok" : "key-missing"}>{hasActiveKey ? "Key saved" : "Key missing"}</span>
            <button onClick={() => void removeProviderKey(activeProvider)}>Clear key</button>
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

function playTone(frequency: number, durationMs: number) {
  try {
    const AudioContextClass = window.AudioContext || (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioContextClass) return;
    const context = new AudioContextClass();
    const oscillator = context.createOscillator();
    const gain = context.createGain();
    oscillator.type = "sine";
    oscillator.frequency.value = frequency;
    gain.gain.setValueAtTime(0.0001, context.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.08, context.currentTime + 0.01);
    gain.gain.exponentialRampToValueAtTime(0.0001, context.currentTime + durationMs / 1000);
    oscillator.connect(gain);
    gain.connect(context.destination);
    oscillator.start();
    oscillator.stop(context.currentTime + durationMs / 1000);
    window.setTimeout(() => void context.close(), durationMs + 40);
  } catch {
    // Audio feedback is best-effort; dictation should never fail because a device blocks sound.
  }
}

function formatError(err: unknown) {
  if (err instanceof Error) return err.message;
  return String(err);
}
