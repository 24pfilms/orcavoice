import type { AppSettings, MicrophoneDevice, SecretStatus, SpeechProvider } from "../lib/types";

interface SettingsPanelProps {
  settings: AppSettings;
  secrets: SecretStatus;
  microphones: MicrophoneDevice[];
  saving: boolean;
  apiKeyDrafts: Record<SpeechProvider, string>;
  onSettingsChange: (settings: AppSettings) => void;
  onSave: () => void;
  onApiKeyDraftChange: (provider: SpeechProvider, value: string) => void;
  onSaveApiKey: (provider: SpeechProvider) => void;
  onClearApiKey: (provider: SpeechProvider) => void;
}

export function SettingsPanel({
  settings,
  secrets,
  microphones,
  saving,
  apiKeyDrafts,
  onSettingsChange,
  onSave,
  onApiKeyDraftChange,
  onSaveApiKey,
  onClearApiKey,
}: SettingsPanelProps) {
  const active = settings.active_provider;
  const providerSettings = active === "groq" ? settings.groq : settings.openai;

  function updateProvider(field: keyof typeof providerSettings, value: string) {
    onSettingsChange({
      ...settings,
      [active === "groq" ? "groq" : "openai"]: {
        ...providerSettings,
        [field]: value,
      },
    });
  }

  return (
    <section className="card">
      <div className="section-header">
        <div>
          <p className="eyebrow">Provider switch</p>
          <h2>Speech settings</h2>
        </div>
        <button disabled={saving} onClick={onSave}>Save settings</button>
      </div>

      <label>
        Speech provider
        <select
          value={settings.active_provider}
          onChange={(event) => onSettingsChange({ ...settings, active_provider: event.target.value as SpeechProvider })}
        >
          <option value="groq">Groq — Whisper Large v3 Turbo</option>
          <option value="open-ai">OpenAI — gpt-4o transcribe</option>
        </select>
      </label>

      <div className="provider-card">
        <div className="section-header compact">
          <strong>{active === "groq" ? "Groq" : "OpenAI"} API key</strong>
          <span className={secretAvailable(secrets, active) ? "ok" : "warn"}>
            {secretAvailable(secrets, active) ? "Configured" : "Missing"}
          </span>
        </div>
        <div className="inline-fields">
          <input
            type="password"
            placeholder={active === "groq" ? "gsk_..." : "sk-..."}
            value={apiKeyDrafts[active]}
            onChange={(event) => onApiKeyDraftChange(active, event.target.value)}
          />
          <button onClick={() => onSaveApiKey(active)}>Save key</button>
          <button className="ghost" onClick={() => onClearApiKey(active)}>Clear</button>
        </div>
        <p className="hint">Keys are stored in the OS keyring, not in settings.json. Environment variables still work.</p>
      </div>

      <div className="two-col">
        <label>
          Model
          <input value={providerSettings.model} onChange={(event) => updateProvider("model", event.target.value)} />
        </label>
        <label>
          Language
          <input value={providerSettings.language} onChange={(event) => updateProvider("language", event.target.value)} />
        </label>
      </div>

      <label>
        Endpoint
        <input value={providerSettings.endpoint} onChange={(event) => updateProvider("endpoint", event.target.value)} />
      </label>

      <label>
        Custom vocabulary / prompt
        <textarea value={providerSettings.prompt} onChange={(event) => updateProvider("prompt", event.target.value)} />
      </label>

      <div className="two-col">
        <label>
          Hotkey
          <input value={settings.hotkey} onChange={(event) => onSettingsChange({ ...settings, hotkey: event.target.value })} />
        </label>
        <label>
          Microphones
          <select disabled>
            {microphones.length ? microphones.map((mic) => <option key={mic.name}>{mic.is_default ? "Default — " : ""}{mic.name}</option>) : <option>No microphones listed</option>}
          </select>
        </label>
      </div>

      <label className="check-row">
        <input
          type="checkbox"
          checked={settings.auto_paste}
          onChange={(event) => onSettingsChange({ ...settings, auto_paste: event.target.checked })}
        />
        Paste transcript into active app after transcription
      </label>
      <label className="check-row">
        <input
          type="checkbox"
          checked={settings.retain_audio}
          onChange={(event) => onSettingsChange({ ...settings, retain_audio: event.target.checked })}
        />
        Retain audio after transcription (Phase A stores no audio even when enabled)
      </label>
    </section>
  );
}

function secretAvailable(status: SecretStatus, provider: SpeechProvider) {
  return provider === "groq" ? status.groq : status.openai;
}
