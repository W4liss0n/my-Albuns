import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, expect, test, vi } from "vitest";

import { tauriProjectDialogClient } from "./tauriProjectDialogClient";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(listen).mockReset();
});

test("submits semantic actions and receives validated owner state", async () => {
  const listener = vi.fn();
  const unlisten = vi.fn();
  let emit!: (payload: unknown) => void;
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    emit = (payload) => handler({ payload } as never);
    return unlisten;
  });
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "current_project_dialog_state"
      ? { busy: false, kind: "projectCloseConfirmation" }
      : undefined,
  );

  await expect(tauriProjectDialogClient.onState(listener)).resolves.toBe(
    unlisten,
  );
  emit({ busy: true, kind: "projectCloseConfirmation" });
  emit({ busy: "yes", kind: "projectCloseConfirmation" });
  await tauriProjectDialogClient.submit(
    "project-close-3",
    "cancelProjectClose",
  );

  expect(listener).toHaveBeenCalledTimes(2);
  expect(listener).toHaveBeenNthCalledWith(1, {
    busy: false,
    kind: "projectCloseConfirmation",
  });
  expect(listener).toHaveBeenNthCalledWith(2, {
    busy: true,
    kind: "projectCloseConfirmation",
  });
  expect(invoke).toHaveBeenNthCalledWith(1, "current_project_dialog_state");
  expect(invoke).toHaveBeenCalledWith("submit_project_dialog_action", {
    action: "cancelProjectClose",
    sessionId: "project-close-3",
  });
});

test("prefers state emitted while the initial state is being hydrated", async () => {
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

  const subscription = tauriProjectDialogClient.onState(listener);
  await Promise.resolve();
  await Promise.resolve();
  emit({
    cancelled: false,
    kind: "exportFailure",
    message: "Falha mais recente",
    retryDisabled: false,
  });
  resolveCurrent({
    cancelRequested: false,
    cancellable: true,
    kind: "exportProgress",
    progress: { kind: "indeterminate", status: "Estado anterior" },
  });

  await expect(subscription).resolves.toBe(unlisten);
  expect(listener).toHaveBeenCalledOnce();
  expect(listener).toHaveBeenCalledWith({
    cancelled: false,
    kind: "exportFailure",
    message: "Falha mais recente",
    retryDisabled: false,
  });
});
