// @vitest-environment node
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, expect, test, vi } from "vitest";

import { closeSettings, onSettingsSection } from "./tauriSettingsWindow";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockResolvedValue(undefined);
  vi.mocked(listen).mockReset();
});

test("forwards only the Settings sections the window can show", async () => {
  const listener = vi.fn();
  const unlisten = vi.fn();
  let emit!: (payload: unknown) => void;
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    emit = (payload) => handler({ payload } as never);
    return unlisten;
  });

  await expect(onSettingsSection(listener)).resolves.toBe(unlisten);
  expect(listen).toHaveBeenCalledExactlyOnceWith(
    "myalbuns://settings-section",
    expect.any(Function),
  );

  for (const payload of [
    "advanced", "", "Photoshop", null, undefined, 0,
    ["photoshop"], { section: "photoshop" },
  ]) {
    emit(payload);
  }
  expect(listener).not.toHaveBeenCalled();

  emit("photoshop");
  emit("performance");
  expect(listener.mock.calls).toEqual([["photoshop"], ["performance"]]);
});

test("closes the Settings window through its native command", () => {
  closeSettings();

  expect(invoke).toHaveBeenCalledExactlyOnceWith("close_application_settings");
});
