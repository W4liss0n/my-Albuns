// @vitest-environment node
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";

import { tauriCacheSettingsPort } from "./tauriCacheSettingsPort";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invalidResponse =
  "O serviço de prévias temporárias retornou uma resposta inválida.";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

test("reads the Cache status through its native command and keeps only the contract fields", async () => {
  vi.mocked(invoke).mockResolvedValue({
    occupiedBytes: 4_096,
    releasableBytes: 1_024,
    clearAllScheduled: false,
    cacheRoot: "C:/Users/Ana/AppData/Local/MyAlbuns/cache",
  });

  await expect(tauriCacheSettingsPort.status()).resolves.toEqual({
    occupiedBytes: 4_096,
    releasableBytes: 1_024,
    clearAllScheduled: false,
  });
  expect(invoke).toHaveBeenCalledExactlyOnceWith("cache_service_status");
});

test("accepts a Cache that is entirely releasable or empty", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({
    occupiedBytes: 2_048,
    releasableBytes: 2_048,
    clearAllScheduled: true,
  });
  await expect(tauriCacheSettingsPort.status()).resolves.toEqual({
    occupiedBytes: 2_048,
    releasableBytes: 2_048,
    clearAllScheduled: true,
  });

  vi.mocked(invoke).mockResolvedValueOnce({
    occupiedBytes: 0,
    releasableBytes: 0,
    clearAllScheduled: false,
  });
  await expect(tauriCacheSettingsPort.status()).resolves.toEqual({
    occupiedBytes: 0,
    releasableBytes: 0,
    clearAllScheduled: false,
  });
});

const validStatus = {
  occupiedBytes: 4_096,
  releasableBytes: 1_024,
  clearAllScheduled: false,
};

test.each<[string, unknown]>([
  ["no response", undefined],
  ["a null response", null],
  ["a text response", "4096"],
  ["a missing occupied size", { ...validStatus, occupiedBytes: undefined }],
  ["a negative occupied size", { ...validStatus, occupiedBytes: -1 }],
  ["a fractional occupied size", { ...validStatus, occupiedBytes: 4_096.5 }],
  ["an occupied size beyond the safe integers", { ...validStatus, occupiedBytes: Number.MAX_SAFE_INTEGER + 1 }],
  ["a textual occupied size", { ...validStatus, occupiedBytes: "4096" }],
  ["a missing releasable size", { ...validStatus, releasableBytes: undefined }],
  ["a negative releasable size", { ...validStatus, releasableBytes: -1 }],
  ["a fractional releasable size", { ...validStatus, releasableBytes: 0.5 }],
  ["more releasable than occupied bytes", { ...validStatus, releasableBytes: 4_097 }],
  ["a missing clear-all flag", { ...validStatus, clearAllScheduled: undefined }],
  ["a textual clear-all flag", { ...validStatus, clearAllScheduled: "false" }],
])("rejects a Cache status with %s", async (_name, response) => {
  vi.mocked(invoke).mockResolvedValue(response);

  await expect(tauriCacheSettingsPort.status()).rejects.toThrow(
    invalidResponse,
  );
});

test("frees the Cache of closed Projects and reports only the freed size", async () => {
  vi.mocked(invoke).mockResolvedValue({ freedBytes: 512, removedFiles: 3 });

  await expect(tauriCacheSettingsPort.freeClosedProjects()).resolves.toEqual({
    freedBytes: 512,
  });
  expect(invoke).toHaveBeenCalledExactlyOnceWith("free_closed_project_cache");
});

test.each<[string, unknown]>([
  ["no response", undefined],
  ["a null response", null],
  ["a bare number", 512],
  ["a missing freed size", {}],
  ["a negative freed size", { freedBytes: -1 }],
  ["a fractional freed size", { freedBytes: 0.5 }],
  ["a freed size beyond the safe integers", { freedBytes: Number.MAX_SAFE_INTEGER + 1 }],
  ["a textual freed size", { freedBytes: "512" }],
])("rejects a closed-Project cleanup with %s", async (_name, response) => {
  vi.mocked(invoke).mockResolvedValue(response);

  await expect(tauriCacheSettingsPort.freeClosedProjects()).rejects.toThrow(
    invalidResponse,
  );
});

test("distinguishes a Cache cleared now from one scheduled for the next start", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({
    kind: "cleared",
    result: { freedBytes: 0, removedFiles: 0 },
    elapsedMs: 12,
  });
  await expect(tauriCacheSettingsPort.clearAll()).resolves.toEqual({
    kind: "cleared",
    result: { freedBytes: 0 },
  });

  vi.mocked(invoke).mockResolvedValueOnce({
    kind: "scheduled",
    result: { freedBytes: 4_096 },
  });
  await expect(tauriCacheSettingsPort.clearAll()).resolves.toEqual({
    kind: "scheduled",
  });

  expect(invoke).toHaveBeenCalledTimes(2);
  expect(invoke).toHaveBeenNthCalledWith(1, "clear_all_cache");
  expect(invoke).toHaveBeenNthCalledWith(2, "clear_all_cache");
});

test.each<[string, unknown]>([
  ["no response", undefined],
  ["a null response", null],
  ["a bare outcome name", "cleared"],
  ["a missing outcome", { result: { freedBytes: 0 } }],
  ["an unknown outcome", { kind: "skipped", result: { freedBytes: 0 } }],
  ["a cleared outcome without its result", { kind: "cleared" }],
  ["a cleared outcome with a flattened result", { kind: "cleared", freedBytes: 0 }],
  ["a cleared outcome with a negative freed size", { kind: "cleared", result: { freedBytes: -1 } }],
  ["a cleared outcome with a textual freed size", { kind: "cleared", result: { freedBytes: "0" } }],
])("rejects a clear-all response with %s", async (_name, response) => {
  vi.mocked(invoke).mockResolvedValue(response);

  await expect(tauriCacheSettingsPort.clearAll()).rejects.toThrow(
    invalidResponse,
  );
});

test.each(["status", "freeClosedProjects", "clearAll"] as const)(
  "leaves a native %s failure to the caller instead of reporting an invalid response",
  async (operation) => {
    const failure = { code: "cache_unavailable" };
    vi.mocked(invoke).mockRejectedValue(failure);

    await expect(tauriCacheSettingsPort[operation]()).rejects.toBe(failure);
  },
);
