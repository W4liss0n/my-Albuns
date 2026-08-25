import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, expect, test, vi } from "vitest";

import { tauriProjectDialogPort } from "./tauriProjectDialogPort";

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

test("presents and dismisses one native project dialog through the owner window", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  const state = {
    busy: false,
    kind: "projectCloseConfirmation",
  } as const;

  await tauriProjectDialogPort.present(state);
  await tauriProjectDialogPort.dismiss();

  expect(invoke).toHaveBeenNthCalledWith(1, "present_project_dialog", {
    state,
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "dismiss_project_dialog");
});

test("serializes dialog mutations so later state cannot overtake window creation", async () => {
  let releaseFirst!: () => void;
  vi.mocked(invoke)
    .mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          releaseFirst = resolve;
        }),
    )
    .mockResolvedValue(undefined);
  const firstState = {
    cancelRequested: false,
    cancellable: true,
    kind: "exportProgress",
    progress: { kind: "indeterminate", status: "Preparando" },
  } as const;
  const secondState = {
    cancelled: false,
    kind: "exportFailure",
    message: "Falhou",
    retryDisabled: false,
  } as const;

  const first = tauriProjectDialogPort.present(firstState);
  const second = tauriProjectDialogPort.present(secondState);
  await Promise.resolve();

  expect(invoke).toHaveBeenCalledOnce();
  releaseFirst();
  await Promise.all([first, second]);

  expect(invoke).toHaveBeenNthCalledWith(1, "present_project_dialog", {
    state: firstState,
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "present_project_dialog", {
    state: secondState,
  });
});

test("forwards only valid semantic actions from the native dialog", async () => {
  const listener = vi.fn();
  const unlisten = vi.fn();
  let emit!: (payload: unknown) => void;
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    emit = (payload) => handler({ payload } as never);
    return unlisten;
  });

  await expect(tauriProjectDialogPort.onAction(listener)).resolves.toBe(
    unlisten,
  );
  expect(listen).toHaveBeenCalledWith(
    "myalbuns://project-dialog-action",
    expect.any(Function),
  );

  emit("cancelAlbumInformation");
  emit("confirmAlbumInformation");
  emit("dismissProjectOperationFailure");
  emit("retryExport");
  emit("unknownAction");

  expect(listener).toHaveBeenCalledTimes(4);
  expect(listener).toHaveBeenNthCalledWith(1, "cancelAlbumInformation");
  expect(listener).toHaveBeenNthCalledWith(2, "confirmAlbumInformation");
  expect(listener).toHaveBeenNthCalledWith(
    3,
    "dismissProjectOperationFailure",
  );
  expect(listener).toHaveBeenNthCalledWith(4, "retryExport");
});
