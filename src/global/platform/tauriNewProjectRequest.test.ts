import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, expect, test, vi } from "vitest";
import { onNewProjectRequest } from "./tauriNewProjectRequest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
beforeEach(() => vi.resetAllMocks());

test("replays startup activation, deduplicates racing events, and keeps a later activation", async () => {
  let emit!: (value: unknown) => void;
  const release = vi.fn();
  vi.mocked(listen).mockImplementation(async (_, handler) => {
    emit = (payload) => handler({ payload } as never);
    emit(2);
    return release;
  });
  vi.mocked(invoke).mockResolvedValue(1);
  const callback = vi.fn();
  expect(await onNewProjectRequest(callback)).toBe(release);
  expect(callback).toHaveBeenCalledOnce();
  for (const value of [0, 1, 2, "3", null, 2.5]) emit(value);
  expect(callback).toHaveBeenCalledOnce();
  emit(3);
  expect(callback).toHaveBeenCalledTimes(2);
});

test("an activation is still available after an abandoned StrictMode subscription", async () => {
  vi.mocked(listen).mockResolvedValue(vi.fn());
  vi.mocked(invoke).mockResolvedValue(1);
  const abandoned = vi.fn();
  (await onNewProjectRequest(abandoned))();
  const mounted = vi.fn();
  await onNewProjectRequest(mounted);
  expect(mounted).toHaveBeenCalledOnce();
});
