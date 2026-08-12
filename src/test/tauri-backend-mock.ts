import { vi, type Mock } from "vitest";
import type { AppSettings, SecretStatus, TranscriptionOperation } from "../lib/types";

export type InvokeMock = Mock<(command: string, args?: Record<string, unknown>) => Promise<unknown>>;
export type ListenMock = Mock<(event: string, handler: () => void) => Promise<() => void>>;

export interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason: unknown) => void;
}

export function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

export const testSettings: AppSettings = {
  active_provider: "groq",
  groq: {
    model: "whisper-large-v3-turbo",
    language: "en",
    prompt: "",
    endpoint: "https://api.groq.com/openai/v1/audio/transcriptions",
  },
  hotkey: "\\",
  mode: "raw",
  custom_mode_instruction: "",
  auto_paste: true,
  retain_audio: false,
  output_mode: "clipboard-paste",
  bubble_outline: "#ff4057",
  outline_width: 1,
  groq_api_key: "gsk_test",
  enhancement_model: "llama-4-scout-17b-16e-instruct",
  input_device: "",
  start_on_login: true,
};

/**
 * A successful dictation whose history write failed. The Rust command reports
 * this as `entry: null` rather than erroring, because the user already has
 * their pasted text.
 */
export function transcriptionOperationWithoutHistory(text = "hello world"): TranscriptionOperation {
  return { ...transcriptionOperation(text), entry: null };
}

export function transcriptionOperation(text = "hello world"): TranscriptionOperation {
  const result = {
    provider: "groq" as const,
    model: "whisper-large-v3-turbo",
    text,
    raw_text: text,
    enhanced: false,
    latency_ms: 420,
    estimated_cost_usd: 0.0001,
    audio_duration_ms: 1200,
  };
  return {
    entry: { ...result, id: "entry-1", created_at: "2026-07-28T00:00:00Z" },
    result,
    recording: {
      duration_ms: 1200,
      sample_rate: 16_000,
      samples: 19_200,
      rms: 900,
      peak: 0.31,
      device: "Test microphone",
    },
    pasted: true,
  };
}

export interface FakeBackend {
  /** Mirrors the Rust `RecorderState`: true between start and stop. */
  recording: boolean;
  secrets: SecretStatus;
  /** Swap per test to make `stop_and_transcribe` resolve, reject, or hang. */
  transcribe: () => Promise<TranscriptionOperation>;
  invoke: InvokeMock;
  /** Handlers currently registered via `listen`, so a leak is observable. */
  hotkeyHandlers: Set<() => void>;
  listen: ListenMock;
  callsOf: (command: string) => Array<Record<string, unknown> | undefined>;
  countOf: (command: string) => number;
  reset: () => void;
}

export function createFakeBackend(): FakeBackend {
  const backend: FakeBackend = {
    recording: false,
    secrets: { groq: true, env_groq: false },
    transcribe: () => Promise.resolve(transcriptionOperation()),
    invoke: vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>(),
    hotkeyHandlers: new Set<() => void>(),
    listen: vi.fn<(event: string, handler: () => void) => Promise<() => void>>(),
    callsOf: (command) =>
      backend.invoke.mock.calls.filter((call) => call[0] === command).map((call) => call[1]),
    countOf: (command) => backend.callsOf(command).length,
    reset: () => {
      backend.recording = false;
      backend.secrets = { groq: true, env_groq: false };
      backend.transcribe = () => Promise.resolve(transcriptionOperation());
      backend.hotkeyHandlers.clear();
      // Reinstate the implementations: a test that overrides `invoke` to make a
      // command fail would otherwise leak that failure into every later test.
      backend.invoke.mockReset();
      backend.invoke.mockImplementation(defaultInvoke);
      backend.listen.mockReset();
      backend.listen.mockImplementation(defaultListen);
    },
  };

  const defaultInvoke = async (command: string): Promise<unknown> => {
    switch (command) {
      case "get_settings":
      case "save_settings":
        return testSettings;
      case "get_secret_status":
        return backend.secrets;
      case "is_recording":
        return backend.recording;
      case "list_microphones":
        return [{ name: "Test microphone", is_default: true }];
      case "get_input_level":
        return {
          recording: backend.recording,
          peak: backend.recording ? 0.4 : 0,
          device: "Test microphone",
          samples: backend.recording ? 4_800 : 0,
          elapsed_ms: 300,
        };
      case "start_recording":
        backend.recording = true;
        return undefined;
      case "cancel_recording":
        backend.recording = false;
        return undefined;
      case "stop_and_transcribe":
        // The real command takes the recorder lock synchronously before it
        // awaits the network, so the backend is no longer recording the moment
        // this is invoked - even if the transcription later fails.
        backend.recording = false;
        return backend.transcribe();
      case "duck_audio":
      case "unduck_audio":
      case "restore_audio":
      case "play_feedback_tone":
      case "show_compact_overlay":
      case "show_settings_overlay":
      case "hide_overlay":
      case "start_overlay_drag":
      case "open_microphone_settings":
      case "paste_text":
        return undefined;
      default:
        // Fail loudly: an unmocked command means the test drifted from the app.
        throw new Error(`FakeBackend received an unmocked command: ${command}`);
    }
  };

  const defaultListen = async (_event: string, handler: () => void) => {
    backend.hotkeyHandlers.add(handler);
    return () => {
      backend.hotkeyHandlers.delete(handler);
    };
  };

  backend.invoke.mockImplementation(defaultInvoke);
  backend.listen.mockImplementation(defaultListen);

  return backend;
}
