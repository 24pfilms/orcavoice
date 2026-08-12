export type SpeechProvider = "groq";
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
  hotkey: string;
  mode: DictationMode;
  custom_mode_instruction: string;
  auto_paste: boolean;
  retain_audio: boolean;
  output_mode: string;
  bubble_outline: string;
  outline_width: number;
  groq_api_key: string;
  enhancement_model: string;
  /** Empty means "system default input". */
  input_device: string;
  /** Re-asserted against the OS login entry on every launch. */
  start_on_login: boolean;
}

export interface SecretStatus {
  groq: boolean;
  env_groq: boolean;
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
  /** Loudest sample, 0-1 of full scale. */
  peak: number;
  device: string;
}

/** Live capture telemetry behind the microphone meter. */
export interface InputLevel {
  recording: boolean;
  /** Peak since the previous poll, 0-1 of full scale. */
  peak: number;
  device: string;
  samples: number;
  elapsed_ms: number;
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
  /** Absent when the transcript could not be written to history. */
  entry: HistoryEntry | null;
  result: TranscriptionResult;
  recording: RecordingSummary;
  pasted: boolean;
}

export interface PlatformInfo {
  os: string;
  clipboard_paste_supported: boolean;
  keyboard_fallback_supported: boolean;
  notes: string[];
}
