import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App, { HIDE_AFTER_ERROR_MS, HIDE_AFTER_SUCCESS_MS } from "./App";
import {
  createFakeBackend,
  deferred,
  transcriptionOperation,
  transcriptionOperationWithoutHistory,
} from "./test/tauri-backend-mock";
import type { TranscriptionOperation } from "./lib/types";

const backend = createFakeBackend();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>) => backend.invoke(command, args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, handler: () => void) => backend.listen(event, handler),
}));

vi.mock("@tauri-apps/plugin-autostart", () => ({
  enable: () => Promise.resolve(),
  disable: () => Promise.resolve(),
  isEnabled: () => Promise.resolve(false),
}));

/**
 * Drain the microtask queue so the long `await` chain inside `toggleRecording`
 * runs to completion. Fake timers do not stall microtasks, so this is enough.
 */
async function flushMicrotasks(ticks = 60) {
  for (let index = 0; index < ticks; index += 1) {
    await Promise.resolve();
  }
}

async function renderOverlay() {
  render(<App />);
  await act(async () => {
    await flushMicrotasks();
  });
  // Settings resolved, so the real toolbar (not the loading pill) is mounted.
  expect(screen.getByTitle("Settings")).toBeInTheDocument();
}

/** Fire the global hotkey exactly as the Rust side emits it. */
async function pressHotkey() {
  await act(async () => {
    for (const handler of [...backend.hotkeyHandlers]) {
      handler();
    }
    await flushMicrotasks();
  });
}

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
    await flushMicrotasks();
  });
}

function statusDot() {
  const dot = document.querySelector(".status-dot");
  if (!dot) throw new Error("status dot is not rendered");
  return dot;
}

function toolbar() {
  const bar = document.querySelector(".toolbar-bar");
  if (!bar) throw new Error("toolbar is not rendered");
  return bar;
}

describe("OrcaVoice overlay lifecycle", () => {
  beforeEach(() => {
    backend.reset();
    vi.useFakeTimers();
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("registers exactly one hotkey listener", async () => {
    await renderOverlay();
    expect(backend.hotkeyHandlers.size).toBe(1);
  });

  it("hides the overlay after a failed transcription instead of staying pinned open", async () => {
    backend.transcribe = () => Promise.reject(new Error("Recording looks silent."));
    await renderOverlay();

    await pressHotkey();
    expect(backend.countOf("start_recording")).toBe(1);
    expect(toolbar().className).toContain("is-recording");

    await pressHotkey();
    // The failure is surfaced, the recorder is torn down, audio is restored...
    expect(backend.countOf("stop_and_transcribe")).toBe(1);
    expect(backend.countOf("cancel_recording")).toBe(1);
    expect(backend.countOf("unduck_audio")).toBeGreaterThanOrEqual(1);
    expect(toolbar().className).toContain("is-error");
    expect(toolbar().className).not.toContain("is-recording");
    expect(statusDot()).toHaveAttribute("title", "Recording looks silent.");
    expect(console.error).toHaveBeenCalled();

    // ...and critically, the overlay is still on screen but scheduled to hide.
    expect(backend.countOf("hide_overlay")).toBe(0);

    await advance(HIDE_AFTER_ERROR_MS - 1);
    expect(backend.countOf("hide_overlay")).toBe(0);

    await advance(1);
    expect(backend.countOf("hide_overlay")).toBe(1);
    expect(toolbar().className).not.toContain("is-error");
    expect(statusDot()).toHaveAttribute("title", "Ready");
  });

  it("hides the overlay after a successful transcription", async () => {
    await renderOverlay();

    await pressHotkey();
    await pressHotkey();

    expect(backend.countOf("hide_overlay")).toBe(0);
    await advance(HIDE_AFTER_SUCCESS_MS);
    expect(backend.countOf("hide_overlay")).toBe(1);
  });

  it("never shows the error state during a successful dictation", async () => {
    await renderOverlay();
    await pressHotkey();
    expect(toolbar().className).toContain("is-recording");

    await pressHotkey();
    // Transcription landed: success styling, and critically NOT is-error.
    expect(toolbar().className).toContain("is-done");
    expect(toolbar().className).not.toContain("is-error");
    expect(toolbar().className).not.toContain("is-recording");

    await advance(HIDE_AFTER_SUCCESS_MS);
    expect(backend.countOf("hide_overlay")).toBe(1);
    expect(toolbar().className).not.toContain("is-error");
  });

  it("never drops to the neutral state between transcribing and hiding", async () => {
    const pending = deferred<TranscriptionOperation>();
    backend.transcribe = () => pending.promise;
    await renderOverlay();

    await pressHotkey();
    await pressHotkey();
    // Transcribing: must read as active, not as an idle/inactive bubble.
    expect(toolbar().className).toContain("is-busy");

    await act(async () => {
      pending.resolve(transcriptionOperation());
      await flushMicrotasks();
    });

    // The gap between busy ending and the hide firing must still be styled.
    expect(toolbar().className).toContain("is-done");
    await advance(HIDE_AFTER_SUCCESS_MS - 1);
    expect(toolbar().className).toContain("is-done");

    await advance(1);
    expect(backend.countOf("hide_overlay")).toBe(1);
  });

  it("treats a dictation whose history write failed as a success", async () => {
    // Regression: a corrupt history.json made stop_and_transcribe reject *after*
    // the text had already been pasted, so every dictation ended in red.
    backend.transcribe = () => Promise.resolve(transcriptionOperationWithoutHistory());
    await renderOverlay();

    await pressHotkey();
    await pressHotkey();

    expect(toolbar().className).toContain("is-done");
    expect(toolbar().className).not.toContain("is-error");
    await advance(HIDE_AFTER_SUCCESS_MS);
    expect(backend.countOf("hide_overlay")).toBe(1);
  });

  it("cancels the pending hide when a new recording starts during the error window", async () => {
    backend.transcribe = () => Promise.reject(new Error("Recording is too short."));
    await renderOverlay();

    await pressHotkey();
    await pressHotkey();
    expect(toolbar().className).toContain("is-error");

    // Trigger a fresh dictation while the error hide is still pending.
    backend.transcribe = () => Promise.resolve(transcriptionOperation());
    await advance(HIDE_AFTER_ERROR_MS - 500);
    expect(backend.countOf("hide_overlay")).toBe(0);

    await pressHotkey();
    expect(backend.countOf("start_recording")).toBe(2);
    expect(toolbar().className).toContain("is-recording");

    // The stale timer must not fire and yank the overlay out mid-recording.
    await advance(HIDE_AFTER_ERROR_MS * 2);
    expect(backend.countOf("hide_overlay")).toBe(0);
    expect(toolbar().className).toContain("is-recording");
  });

  it("stays responsive when the show-overlay IPC call itself rejects", async () => {
    await renderOverlay();

    // A rejecting show_compact_overlay used to strand the in-flight latch,
    // killing the toggle permanently and pinning the overlay on screen.
    let failShow = true;
    const realInvoke = backend.invoke.getMockImplementation()!;
    backend.invoke.mockImplementation(async (command, args) => {
      if (command === "show_compact_overlay" && failShow) {
        throw new Error("Cannot find main window");
      }
      return realInvoke(command, args);
    });

    await pressHotkey();
    expect(toolbar().className).toContain("is-error");
    expect(backend.countOf("start_recording")).toBe(0);

    // The failure must still put the overlay away rather than stranding it.
    await advance(HIDE_AFTER_ERROR_MS);
    expect(backend.countOf("hide_overlay")).toBe(1);

    // ...and the toggle must still work once the window is healthy again.
    failShow = false;
    await pressHotkey();
    expect(backend.countOf("start_recording")).toBe(1);
    expect(toolbar().className).toContain("is-recording");
  });

  it("still hides when audio recovery fails during error handling", async () => {
    await renderOverlay();
    const realInvoke = backend.invoke.getMockImplementation()!;
    backend.invoke.mockImplementation(async (command, args) => {
      if (command === "unduck_audio") throw new Error("COM volume call failed");
      return realInvoke(command, args);
    });
    backend.transcribe = () => Promise.reject(new Error("Groq request failed"));

    await pressHotkey();
    await pressHotkey();

    await advance(HIDE_AFTER_ERROR_MS);
    expect(backend.countOf("hide_overlay")).toBe(1);
  });

  it("ignores a hotkey press while a transcription is still in flight", async () => {
    const pending = deferred<TranscriptionOperation>();
    backend.transcribe = () => pending.promise;
    await renderOverlay();

    await pressHotkey();
    await pressHotkey();
    expect(backend.countOf("stop_and_transcribe")).toBe(1);
    expect(toolbar().className).toContain("is-busy");

    // The backend already released the recorder, so an unguarded press would
    // read `is_recording === false` and start a brand new recording.
    await pressHotkey();
    await pressHotkey();
    expect(backend.countOf("stop_and_transcribe")).toBe(1);
    expect(backend.countOf("start_recording")).toBe(1);
    expect(backend.countOf("hide_overlay")).toBe(0);

    await act(async () => {
      pending.resolve(transcriptionOperation());
      await flushMicrotasks();
    });
    expect(toolbar().className).not.toContain("is-busy");

    await advance(HIDE_AFTER_SUCCESS_MS);
    expect(backend.countOf("hide_overlay")).toBe(1);

    // Once the flight lands, the overlay accepts input again.
    await pressHotkey();
    expect(backend.countOf("start_recording")).toBe(2);
  });
});
