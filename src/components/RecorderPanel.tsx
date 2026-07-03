import type { BenchmarkOperation, TranscriptionOperation } from "../lib/types";

interface RecorderPanelProps {
  recording: boolean;
  busy: boolean;
  status: string;
  error: string;
  lastOperation?: TranscriptionOperation;
  benchmark?: BenchmarkOperation;
  onToggle: () => void;
  onBenchmark: () => void;
}

export function RecorderPanel({
  recording,
  busy,
  status,
  error,
  lastOperation,
  benchmark,
  onToggle,
  onBenchmark,
}: RecorderPanelProps) {
  return (
    <section className="card hero-card">
      <div className="hero-brand">
        <div className="hero-logo">orca</div>
        <div>
          <p className="eyebrow">Speech to Text</p>
          <h1>OrcaVoice</h1>
        </div>
      </div>

      <p className="lede">Speak naturally. Transcribe with Groq or OpenAI. Paste into any app.</p>

      <div className="recorder-actions">
        <button className={recording ? "danger" : "primary"} disabled={busy} onClick={onToggle}>
          {recording ? "Stop + transcribe" : "Start recording"}
        </button>
        <button disabled={busy || !recording} onClick={onBenchmark}>
          Stop + benchmark providers
        </button>
      </div>

      <div className={`status-pill ${recording ? "recording" : busy ? "busy" : ""}`}>{status}</div>
      {error ? <div className="error-box">{error}</div> : null}

      {lastOperation ? (
        <div className="result-box">
          <div className="result-meta">
            <span>{lastOperation.result.provider === "groq" ? "Groq" : "OpenAI"}</span>
            <span>{lastOperation.result.model}</span>
            <span>{lastOperation.result.latency_ms} ms</span>
            <span>${lastOperation.result.estimated_cost_usd.toFixed(6)}</span>
            <span>{lastOperation.pasted ? "Pasted" : "Copied"}</span>
          </div>
          <pre>{lastOperation.result.text}</pre>
        </div>
      ) : null}

      {benchmark ? (
        <div className="benchmark-grid">
          <BenchmarkCell label="Groq" value={benchmark.benchmark.groq} />
          <BenchmarkCell label="OpenAI" value={benchmark.benchmark.openai} />
        </div>
      ) : null}
    </section>
  );
}

function BenchmarkCell({ label, value }: { label: string; value: BenchmarkOperation["benchmark"]["groq"] }) {
  if (!value.ok || !value.result) {
    return (
      <div className="benchmark-cell failed">
        <strong>{label}</strong>
        <p className="hint">{value.error ?? "Provider failed."}</p>
      </div>
    );
  }
  return (
    <div className="benchmark-cell">
      <strong>{label}</strong>
      <div className="result-meta">
        <span>{value.result.model}</span>
        <span>{value.result.latency_ms} ms</span>
        <span>${value.result.estimated_cost_usd.toFixed(6)}</span>
      </div>
      <pre>{value.result.raw_text}</pre>
    </div>
  );
}
