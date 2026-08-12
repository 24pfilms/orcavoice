import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import type {
  AppSettings,
  HistoryEntry,
  InputLevel,
  MicrophoneDevice,
  PlatformInfo,
  SecretStatus,
  SpeechProvider,
  TranscriptionOperation,
} from "./types";

export const HOTKEY_EVENT = "orcavoice://hotkey-toggle";

export function getSettings() {
  return invoke<AppSettings>("get_settings");
}

export function saveSettings(settings: AppSettings) {
  return invoke<AppSettings>("save_settings", { newSettings: settings });
}

export function getSecretStatus() {
  return invoke<SecretStatus>("get_secret_status");
}

export function setApiKey(provider: SpeechProvider, apiKey: string) {
  return invoke<SecretStatus>("set_api_key", { provider, apiKey });
}

export function clearApiKey(provider: SpeechProvider) {
  return invoke<SecretStatus>("clear_api_key", { provider });
}

export function listMicrophones() {
  return invoke<MicrophoneDevice[]>("list_microphones");
}

export function getInputLevel() {
  return invoke<InputLevel>("get_input_level");
}

/** Windows only: opens Settings > Privacy & security > Microphone. */
export function openMicrophoneSettings() {
  return invoke<void>("open_microphone_settings");
}

export function isRecording() {
  return invoke<boolean>("is_recording");
}

export function duckAudio() {
  return invoke<void>("duck_audio");
}

export function unduckAudio() {
  return invoke<void>("unduck_audio");
}

export function restoreAudio() {
  return invoke<void>("restore_audio");
}

export function playFeedbackTone(kind: "start" | "stop") {
  return invoke<void>("play_feedback_tone", { kind });
}

export function startRecording() {
  return invoke<void>("start_recording");
}

export function cancelRecording() {
  return invoke<void>("cancel_recording");
}

export function stopAndTranscribe() {
  return invoke<TranscriptionOperation>("stop_and_transcribe");
}

export function pasteText(text: string) {
  return invoke<void>("paste_text", { text });
}

export function getHistory() {
  return invoke<HistoryEntry[]>("get_history");
}

export function clearHistory() {
  return invoke<void>("clear_history");
}

export function getPlatformInfo() {
  return invoke<PlatformInfo>("get_platform_info");
}

export function showCompactOverlay() {
  return invoke<void>("show_compact_overlay");
}

export function showSettingsOverlay() {
  return invoke<void>("show_settings_overlay");
}

export function hideOverlay() {
  return invoke<void>("hide_overlay");
}

export function startOverlayDrag() {
  return invoke<void>("start_overlay_drag");
}

export function onHotkeyToggle(handler: () => void) {
  return listen(HOTKEY_EVENT, handler);
}

export { enable as enableAutostart, disable as disableAutostart, isEnabled as isAutostartEnabled };
