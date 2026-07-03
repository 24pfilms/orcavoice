import { useCallback, useEffect, useMemo, useState, type CSSProperties } from "react";
import {
  cancelRecording,
  clearApiKey,
  disableAutostart,
  duckAudio,
  enableAutostart,
  getSecretStatus,
  getSettings,
  hideOverlay,
  isAutostartEnabled,
  isRecording,
  onHotkeyToggle,
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
    void restoreAudio();
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
        await unduckAudio();
        await playFeedbackTone("stop");
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
        await playFeedbackTone("start");
        await startRecording();
        await duckAudio();
        await showCompactOverlay();
        setRecording(true);
        setStatus("Listening");
      }
    } catch (err) {
      setError(formatError(err));
      setStatus("Error");
      await showCompactOverlay();
      await cancelRecording();
      await unduckAudio();
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
      setSettingsOpen(false);
      await showCompactOverlay();
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
    await unduckAudio();
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

  function updateProviderSettings(field: "language" | "prompt", value: string) {
    if (!settings) return;
    const key = settings.active_provider === "groq" ? "groq" : "openai";
    const updated = {
      ...settings,
      [key]: { ...settings[key], [field]: value },
    };
    setSettings(updated);
  }

  async function updateAndPersistSettings(patch: Partial<AppSettings>) {
    if (!settings) return;
    const nextSettings = { ...settings, ...patch };
    setSettings(nextSettings);
    await persistSettings(nextSettings);
  }

  if (!settings) {
    return <main className="overlay-root compact"><div className="pill-group loading-pill">OrcaVoice</div></main>;
  }

  return (
    <main
      className={`overlay-root ${settingsOpen ? "expanded" : "compact"}`}
      style={{ "--border": settings.bubble_outline, "--outline-width": `${settings.outline_width}px` } as CSSProperties}
    >
      <section className={`toolbar-bar ${recording ? "is-recording" : ""} ${busy ? "is-busy" : ""} ${error ? "is-error" : ""}`}>
        {/* Pill 1: Drag + Language */}
        <div className="pill-group">
          <button className="icon-btn grab-handle" title="Drag OrcaVoice" onPointerDown={() => void startOverlayDrag()}>
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
          <button className="provider-circle" title={`Provider: ${providerName}`} onClick={() => void updateAndPersistSettings({ active_provider: activeProvider === "groq" ? "open-ai" : "groq" })}>
            {activeProvider === "groq" ? "G" : "O"}
          </button>
          <span className="status-dot" title={compactStatus} />
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
            <strong>OrcaVoice</strong>
            <div className="header-right">
              <span>{compactStatus}</span>
              <button className="close-button" title="Close" onClick={() => void toggleSettings()}>✕</button>
            </div>
          </div>

          {error ? <div className="mini-error">{error}</div> : null}
          {lastOperation ? (
            <div className="mini-result">
              {lastOperation.result.enhanced ? <span className="enhanced-badge">Enhanced</span> : <span className="raw-badge">Raw</span>}
              {" "}
              {lastOperation.result.text}
            </div>
          ) : null}

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
              Provider
              <select
                value={settings.active_provider}
                onChange={(event) => updateSettings({ active_provider: event.target.value as SpeechProvider })}
              >
                <option value="groq">Groq — fast</option>
                <option value="open-ai">OpenAI — quality</option>
              </select>
            </label>
          </div>

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

function formatError(err: unknown) {
  if (err instanceof Error) return err.message;
  return String(err);
}
