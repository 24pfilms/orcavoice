import type { AppSettings, DictationMode } from "../lib/types";

interface ModeEditorProps {
  settings: AppSettings;
  onSettingsChange: (settings: AppSettings) => void;
}

export function ModeEditor({ settings, onSettingsChange }: ModeEditorProps) {
  return (
    <section className="card">
      <p className="eyebrow">AI modes</p>
      <h2>Output mode</h2>
      <label>
        Mode
        <select
          value={settings.mode}
          onChange={(event) => onSettingsChange({ ...settings, mode: event.target.value as DictationMode })}
        >
          <option value="raw">Raw dictation</option>
          <option value="grammar">Grammar cleanup</option>
          <option value="email">Email draft</option>
          <option value="translate-english">Translate to English</option>
          <option value="custom">Custom instruction</option>
        </select>
      </label>
      <label>
        Custom instruction
        <textarea
          value={settings.custom_mode_instruction}
          onChange={(event) => onSettingsChange({ ...settings, custom_mode_instruction: event.target.value })}
          placeholder="Example: Turn this into concise bullet points."
        />
      </label>
      <p className="hint">Phase A keeps raw mode safest. Other modes are conservative local formatting until a polish LLM is configured.</p>
    </section>
  );
}
