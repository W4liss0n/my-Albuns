import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { parsePhotoshopStatus, tauriPhotoshopPort, tauriPhotoshopSettingsPort } from "./tauriPhotoshopPort";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => vi.mocked(invoke).mockReset());
const status = { revision: 3, installations: [{ id: "a", name: "Photoshop", version: "27.10", path: "C:\\Adobe\\Photoshop.exe" }], selectedInstallationId: "a" };

test.each([
  null, { ...status, revision: -1 }, { ...status, revision: Number.MAX_SAFE_INTEGER + 1 },
  { ...status, selectedInstallationId: "foreign" }, { ...status, installations: [...status.installations, ...status.installations] },
  { ...status, installations: [{ ...status.installations[0], path: 42 }] },
])("rejects malformed installation snapshots: %j", (value) => {
  expect(() => parsePhotoshopStatus(value)).toThrow();
});

test("sends contextual IDs to the backend and preserves actionable native errors", async () => {
  vi.mocked(invoke).mockRejectedValueOnce({ code: "original_unavailable", message: "Original indisponível." });
  await expect(tauriPhotoshopPort.openPhoto({ kind: "frames", frameIds: ["frame-1"] }))
    .rejects.toMatchObject({ code: "original_unavailable", message: "Original indisponível." });
  expect(invoke).toHaveBeenCalledExactlyOnceWith("open_in_photoshop", { target: { kind: "frames", frameIds: ["frame-1"] } });
});

test("manual selection accepts cancellation and validates a successful native result", async () => {
  vi.mocked(invoke).mockResolvedValueOnce(null).mockResolvedValueOnce(status);
  expect(await tauriPhotoshopSettingsPort.locate()).toBeNull();
  expect(await tauriPhotoshopSettingsPort.locate()).toEqual(status);
});
