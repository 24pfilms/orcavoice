export type SpeechProvider = "groq" | "open-ai";
export type DictationMode = "raw" | "grammar" | "email" | "translate-english" | "custom";

export interface ProviderSettings {
  model: string;
  language: string;
  prompt: string;
  endpoint: string;
}

export interface AppSettings {
  active_provider: SpeechProvider;
  groq: ProviderSettings;
  openai: ProviderSettings;
  hotkey: string;
  mode: DictationMode;
  custom_mode_instruction: string;
  auto_paste: boolean;
  retain_audio: boolean;
  output_mode: string;
  bubble_outline: string;
  outline_width: number;
  groq_api_key: string;
  openai_api_key: string;
  enhancement_model: string;
}

export interface SecretStatus {
  groq: boolean;
  openai: boolean;
  env_groq: boolean;
  env_openai: boolean;
}

export interface MicrophoneDevice {
  name: string;
  is_default: boolean;
}

export interface RecordingSummary {
  duration_ms: number;
  sample_rate: number;
  samples: number;
  rms: number;
}

export interface TranscriptionResult {
  provider: SpeechProvider;
  model: string;
  text: string;
  raw_text: string;
  enhanced: boolean;
  latency_ms: number;
  estimated_cost_usd: number;
  audio_duration_ms: number;
}

export interface HistoryEntry extends TranscriptionResult {
  id: string;
  created_at: string;
}

export interface TranscriptionOperation {
  entry: HistoryEntry;
  result: TranscriptionResult;
  recording: RecordingSummary;
  pasted: boolean;
}

export interface ProviderBenchmarkResult {
  ok: boolean;
  result: TranscriptionResult | null;
  error: string | null;
}

export interface BenchmarkOperation {
  recording: RecordingSummary;
  benchmark: {
    groq: ProviderBenchmarkResult;
    openai: ProviderBenchmarkResult;
  };
}

export interface PlatformInfo {
  os: string;
  clipboard_paste_supported: boolean;
  keyboard_fallback_supported: boolean;
  notes: string[];
}
