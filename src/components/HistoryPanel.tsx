import type { HistoryEntry } from "../lib/types";

interface HistoryPanelProps {
  history: HistoryEntry[];
  onPaste: (text: string) => void;
  onClear: () => void;
}

export function HistoryPanel({ history, onPaste, onClear }: HistoryPanelProps) {
  return (
    <section className="card history-card">
      <div className="section-header">
        <div>
          <p className="eyebrow">Local only</p>
          <h2>History</h2>
        </div>
        <button className="ghost" onClick={onClear} disabled={!history.length}>Clear history</button>
      </div>
      {!history.length ? <p className="hint">No transcripts yet.</p> : null}
      <div className="history-list">
        {history.map((entry) => (
          <article key={entry.id} className="history-entry">
            <div className="result-meta">
              <span>{new Date(entry.created_at).toLocaleString()}</span>
              <span>{entry.provider === "groq" ? "Groq" : "OpenAI"}</span>
              <span>{entry.model}</span>
              <span>{entry.latency_ms} ms</span>
            </div>
            <p>{entry.text}</p>
            <button className="ghost" onClick={() => onPaste(entry.text)}>Paste again</button>
          </article>
        ))}
      </div>
    </section>
  );
}
