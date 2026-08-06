import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, expect, test, vi } from "vitest";

import { representativeProjection } from "../test/projectFixtures";
import { tauriProjectWindowPort } from "./tauriProjectWindowPort";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(listen).mockReset();
});

test("forwards the native close request through one project-window event", async () => {
  const listener = vi.fn();
  const unlisten = vi.fn();
  let emit!: () => void;
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    emit = () => handler({} as never);
    return unlisten;
  });

  await expect(
    tauriProjectWindowPort.onCloseRequested(listener),
  ).resolves.toBe(unlisten);
  expect(listen).toHaveBeenCalledWith(
    "myalbuns://project-close-confirmation-requested",
    expect.any(Function),
  );

  emit();
  expect(listener).toHaveBeenCalledTimes(1);
});

test("maps application close requests and all decisions to their native commands", async () => {
  vi.mocked(invoke)
    .mockResolvedValueOnce({ kind: "confirmationRequired" })
    .mockResolvedValueOnce({
      kind: "cancelled",
      projection: representativeProjection,
    })
    .mockResolvedValueOnce({ kind: "closed" });

  await expect(tauriProjectWindowPort.requestClose()).resolves.toEqual({
    kind: "confirmationRequired",
  });
  await expect(
    tauriProjectWindowPort.resolveClose("cancel"),
  ).resolves.toEqual({
    kind: "cancelled",
    projection: representativeProjection,
  });
  await expect(
    tauriProjectWindowPort.resolveClose("discardAndClose"),
  ).resolves.toEqual({ kind: "closed" });

  expect(invoke).toHaveBeenNthCalledWith(1, "request_project_close");
  expect(invoke).toHaveBeenNthCalledWith(2, "resolve_project_close", {
    choice: "cancel",
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "resolve_project_close", {
    choice: "discardAndClose",
  });
});

test("rejects malformed close responses instead of guessing native state", async () => {
  vi.mocked(invoke).mockResolvedValue({ kind: "closed", projection: {} });

  await expect(tauriProjectWindowPort.requestClose()).rejects.toMatchObject({
    name: "ProjectCloseError",
    code: "invalid_response",
  });
});

test("localizes a conclusive save-and-close conflict without losing its code", async () => {
  vi.mocked(invoke).mockRejectedValue({
    code: "persisted_baseline_conflict",
  });

  await expect(
    tauriProjectWindowPort.resolveClose("saveAndClose"),
  ).rejects.toMatchObject({
    name: "ProjectCloseError",
    code: "persisted_baseline_conflict",
    message: expect.stringContaining("fora do MyAlbuns"),
  });
});
