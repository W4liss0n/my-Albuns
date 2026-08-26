import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, expect, test, vi } from "vitest";

import {
  PROJECT_DIALOG_PRESENTATION_EVENT,
  tauriProjectDialogClient,
} from "./tauriProjectDialogClient";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(listen).mockReset();
});

test("submits semantic actions and receives a validated owned presentation", async () => {
  const listener = vi.fn();
  const unlisten = vi.fn();
  let emit!: (payload: unknown) => void;
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    emit = (payload) => handler({ payload } as never);
    return unlisten;
  });
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "current_project_dialog_presentation"
      ? {
          sessionId: "project-close-3",
          state: { busy: false, kind: "projectCloseConfirmation" },
        }
      : undefined,
  );

  await expect(
    tauriProjectDialogClient.onPresentation(listener),
  ).resolves.toBe(unlisten);
  emit({
    sessionId: "project-close-4",
    state: { busy: true, kind: "projectCloseConfirmation" },
  });
  emit({
    sessionId: "project-close-5",
    state: { busy: "yes", kind: "projectCloseConfirmation" },
  });
  await tauriProjectDialogClient.submit(
    "project-close-3",
    "cancelProjectClose",
  );

  expect(listener).toHaveBeenCalledTimes(2);
  expect(listen).toHaveBeenCalledWith(
    PROJECT_DIALOG_PRESENTATION_EVENT,
    expect.any(Function),
  );
  expect(listener).toHaveBeenNthCalledWith(1, {
    sessionId: "project-close-3",
    state: { busy: false, kind: "projectCloseConfirmation" },
  });
  expect(listener).toHaveBeenNthCalledWith(2, {
    sessionId: "project-close-4",
    state: { busy: true, kind: "projectCloseConfirmation" },
  });
  expect(invoke).toHaveBeenNthCalledWith(
    1,
    "current_project_dialog_presentation",
  );
  expect(invoke).toHaveBeenCalledWith("submit_project_dialog_action", {
    action: "cancelProjectClose",
    sessionId: "project-close-3",
  });
});

test("prefers an owned presentation emitted during initial hydration", async () => {
  const listener = vi.fn();
  const unlisten = vi.fn();
  let emit!: (payload: unknown) => void;
  let resolveCurrent!: (value: unknown) => void;
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    emit = (payload) => handler({ payload } as never);
    return unlisten;
  });
  vi.mocked(invoke).mockImplementation(
    () =>
      new Promise((resolve) => {
        resolveCurrent = resolve;
      }),
  );

  const subscription = tauriProjectDialogClient.onPresentation(listener);
  await Promise.resolve();
  await Promise.resolve();
  emit({
    sessionId: "export-2",
    state: {
      cancelled: false,
      kind: "exportFailure",
      message: "Falha mais recente",
      retryDisabled: false,
    },
  });
  resolveCurrent({
    sessionId: "export-1",
    state: {
      cancelRequested: false,
      cancellable: true,
      kind: "exportProgress",
      progress: { kind: "indeterminate", status: "Estado anterior" },
    },
  });

  await expect(subscription).resolves.toBe(unlisten);
  expect(listener).toHaveBeenCalledOnce();
  expect(listener).toHaveBeenCalledWith({
    sessionId: "export-2",
    state: {
      cancelled: false,
      kind: "exportFailure",
      message: "Falha mais recente",
      retryDisabled: false,
    },
  });
});
