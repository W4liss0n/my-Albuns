import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, expect, test, vi } from "vitest";

import { createTauriProjectDialogPort } from "./tauriProjectDialogPort";

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

function nativeActionHarness() {
  const unlisten = vi.fn();
  let emit!: (payload: unknown) => void;
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    emit = (payload) => handler({ payload } as never);
    return unlisten;
  });
  return {
    emit: (payload: unknown) => emit(payload),
    unlisten,
  };
}

function invokedSessionId(callIndex: number) {
  const args = vi.mocked(invoke).mock.calls[callIndex]?.[1];
  if (
    !args ||
    typeof args !== "object" ||
    !("sessionId" in args) ||
    typeof args.sessionId !== "string"
  ) {
    throw new Error("the native dialog mutation must carry a session id");
  }
  return args.sessionId;
}

test("scopes native presentation, updates and actions to one dialog session", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  const native = nativeActionHarness();
  const port = createTauriProjectDialogPort();
  const listener = vi.fn();
  const session = port.acquire(listener);
  const confirmation = {
    busy: false,
    kind: "projectCloseConfirmation",
  } as const;
  const busyConfirmation = { ...confirmation, busy: true };

  await session.present(confirmation);
  const sessionId = invokedSessionId(0);
  await session.present(busyConfirmation);

  expect(sessionId).toEqual(expect.any(String));
  expect(invoke).toHaveBeenNthCalledWith(1, "present_project_dialog", {
    sessionId,
    state: confirmation,
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "present_project_dialog", {
    sessionId,
    state: busyConfirmation,
  });

  native.emit({ action: "saveAndClose", sessionId });
  native.emit({ action: "discardAndClose", sessionId: "stale-owner" });
  native.emit({ action: "unknownAction", sessionId });

  expect(listener).toHaveBeenCalledOnce();
  expect(listener).toHaveBeenCalledWith("saveAndClose");
});

test("queues a second owner until the current owner dismisses", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  nativeActionHarness();
  const port = createTauriProjectDialogPort();
  const first = port.acquire(vi.fn());
  const second = port.acquire(vi.fn());
  const firstState = {
    busy: false,
    details: [],
    kind: "albumInformationConfirmation",
  } as const;
  const secondState = {
    kind: "projectOperationFailure",
    message: "Falhou",
  } as const;

  await first.present(firstState);
  const firstId = invokedSessionId(0);
  const queuedPresentation = second.present(secondState);
  await Promise.resolve();

  expect(invoke).toHaveBeenCalledOnce();

  await first.dismiss();
  await queuedPresentation;
  const secondId = invokedSessionId(2);

  expect(secondId).not.toBe(firstId);
  expect(invoke).toHaveBeenNthCalledWith(2, "dismiss_project_dialog", {
    sessionId: firstId,
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "present_project_dialog", {
    sessionId: secondId,
    state: secondState,
  });
});

test("materializes an update that arrives while the native window is being created", async () => {
  let releaseCreation!: () => void;
  vi.mocked(invoke)
    .mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          releaseCreation = resolve;
        }),
    )
    .mockResolvedValue(undefined);
  nativeActionHarness();
  const port = createTauriProjectDialogPort();
  const session = port.acquire(vi.fn());
  const initial = {
    cancelRequested: false,
    cancellable: true,
    kind: "exportProgress",
    progress: { kind: "indeterminate", status: "Preparando" },
  } as const;
  const completed = {
    kind: "exportSuccess",
    message: "Concluída",
  } as const;

  const opening = session.present(initial);
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledOnce());
  const updating = session.present(completed);
  releaseCreation();
  await Promise.all([opening, updating]);

  const sessionId = invokedSessionId(0);
  expect(invoke).toHaveBeenNthCalledWith(2, "present_project_dialog", {
    sessionId,
    state: completed,
  });
});

test("an obsolete owner cannot update or dismiss the next session", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  const native = nativeActionHarness();
  const port = createTauriProjectDialogPort();
  const staleListener = vi.fn();
  const currentListener = vi.fn();
  const stale = port.acquire(staleListener);
  const current = port.acquire(currentListener);

  await stale.present({ busy: false, kind: "projectCloseConfirmation" });
  const staleId = invokedSessionId(0);
  await stale.dismiss();
  await current.present({
    cancelRequested: false,
    cancellable: true,
    kind: "exportProgress",
    progress: { kind: "indeterminate", status: "Preparando" },
  });
  const currentId = invokedSessionId(2);
  const callsBeforeStaleMutations = vi.mocked(invoke).mock.calls.length;

  await stale.present({ busy: true, kind: "projectCloseConfirmation" });
  await stale.dismiss();
  native.emit({ action: "cancelProjectClose", sessionId: staleId });
  native.emit({ action: "cancelExport", sessionId: currentId });

  expect(invoke).toHaveBeenCalledTimes(callsBeforeStaleMutations);
  expect(staleListener).not.toHaveBeenCalled();
  expect(currentListener).toHaveBeenCalledWith("cancelExport");
});

test("removes a queued owner without disturbing the active session", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  nativeActionHarness();
  const port = createTauriProjectDialogPort();
  const active = port.acquire(vi.fn());
  const queued = port.acquire(vi.fn());

  await active.present({ busy: false, kind: "projectCloseConfirmation" });
  const queuedPresentation = queued.present({
    kind: "projectOperationFailure",
    message: "Falhou",
  });
  await Promise.resolve();
  await queued.dismiss();
  await queuedPresentation;

  expect(invoke).toHaveBeenCalledOnce();
  await active.present({ busy: true, kind: "projectCloseConfirmation" });
  expect(invoke).toHaveBeenCalledTimes(2);
});

test("a session dismissed before its first projection is permanently obsolete", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  nativeActionHarness();
  const port = createTauriProjectDialogPort();
  const obsolete = port.acquire(vi.fn());

  await obsolete.dismiss();
  await obsolete.present({
    kind: "projectOperationFailure",
    message: "Não deve aparecer",
  });

  expect(invoke).not.toHaveBeenCalled();
  expect(listen).not.toHaveBeenCalled();
});

test("a failed native dismissal rejects queued and future owners instead of orphaning them", async () => {
  const dismissalFailure = new Error("native dismiss failed");
  vi.mocked(invoke)
    .mockResolvedValueOnce(undefined)
    .mockRejectedValueOnce(dismissalFailure);
  nativeActionHarness();
  const port = createTauriProjectDialogPort();
  const active = port.acquire(vi.fn());
  const queued = port.acquire(vi.fn());

  await active.present({ busy: false, kind: "projectCloseConfirmation" });
  const queuedPresentation = queued.present({
    kind: "projectOperationFailure",
    message: "Falha enfileirada",
  });

  await expect(active.dismiss()).rejects.toBe(dismissalFailure);
  await expect(queuedPresentation).rejects.toBe(dismissalFailure);
  await expect(
    port.acquire(vi.fn()).present({
      kind: "projectOperationFailure",
      message: "Falha posterior",
    }),
  ).rejects.toBe(dismissalFailure);
  expect(invoke).toHaveBeenCalledTimes(2);
});

test("routes an action after the first projection appears while a newer update is pending", async () => {
  let releaseCreation!: () => void;
  let releaseUpdate!: () => void;
  vi.mocked(invoke)
    .mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          releaseCreation = resolve;
        }),
    )
    .mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          releaseUpdate = resolve;
        }),
    );
  const native = nativeActionHarness();
  const listener = vi.fn();
  const session = createTauriProjectDialogPort().acquire(listener);

  const opening = session.present({
    cancelRequested: false,
    cancellable: true,
    kind: "exportProgress",
    progress: { kind: "indeterminate", status: "Preparando" },
  });
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledOnce());
  const updating = session.present({
    kind: "exportSuccess",
    message: "Concluída",
  });
  releaseCreation();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));

  native.emit({ action: "dismissExport", sessionId: invokedSessionId(0) });
  expect(listener).toHaveBeenCalledWith("dismissExport");

  releaseUpdate();
  await Promise.all([opening, updating]);
});

test("releases a failed initial owner before activating its successor", async () => {
  const presentationFailure = new Error("native presentation failed");
  vi.mocked(invoke)
    .mockRejectedValueOnce(presentationFailure)
    .mockResolvedValue(undefined);
  nativeActionHarness();
  const port = createTauriProjectDialogPort();
  const failed = port.acquire(vi.fn());
  const successor = port.acquire(vi.fn());

  const failedPresentation = failed.present({
    busy: false,
    kind: "projectCloseConfirmation",
  });
  const successorPresentation = successor.present({
    kind: "projectOperationFailure",
    message: "O owner seguinte continua utilizável",
  });

  await expect(failedPresentation).rejects.toBe(presentationFailure);
  await successorPresentation;

  const failedSessionId = invokedSessionId(0);
  const successorSessionId = invokedSessionId(2);
  expect(invoke).toHaveBeenNthCalledWith(2, "dismiss_project_dialog", {
    sessionId: failedSessionId,
  });
  expect(successorSessionId).not.toBe(failedSessionId);
});

test("releases an active owner whose update fails before activating its successor", async () => {
  const updateFailure = new Error("native update failed");
  vi.mocked(invoke)
    .mockResolvedValueOnce(undefined)
    .mockRejectedValueOnce(updateFailure)
    .mockResolvedValue(undefined);
  nativeActionHarness();
  const port = createTauriProjectDialogPort();
  const active = port.acquire(vi.fn());
  const successor = port.acquire(vi.fn());

  await active.present({ busy: false, kind: "projectCloseConfirmation" });
  const successorPresentation = successor.present({
    kind: "projectOperationFailure",
    message: "O owner seguinte continua utilizável",
  });
  const failedUpdate = active.present({
    busy: true,
    kind: "projectCloseConfirmation",
  });

  await expect(failedUpdate).rejects.toBe(updateFailure);
  await successorPresentation;

  const activeSessionId = invokedSessionId(0);
  const successorSessionId = invokedSessionId(3);
  expect(invoke).toHaveBeenNthCalledWith(3, "dismiss_project_dialog", {
    sessionId: activeSessionId,
  });
  expect(successorSessionId).not.toBe(activeSessionId);
});

test("fails closed when cleanup after a rejected update also fails", async () => {
  const updateFailure = new Error("native update failed");
  const cleanupFailure = new Error("native cleanup failed");
  vi.mocked(invoke)
    .mockResolvedValueOnce(undefined)
    .mockRejectedValueOnce(updateFailure)
    .mockRejectedValueOnce(cleanupFailure);
  nativeActionHarness();
  const port = createTauriProjectDialogPort();
  const active = port.acquire(vi.fn());
  const queued = port.acquire(vi.fn());

  await active.present({ busy: false, kind: "projectCloseConfirmation" });
  const queuedPresentation = queued.present({
    kind: "projectOperationFailure",
    message: "Não pode ficar órfão",
  });
  const failedUpdate = active.present({
    busy: true,
    kind: "projectCloseConfirmation",
  });

  await expect(failedUpdate).rejects.toBe(cleanupFailure);
  await expect(queuedPresentation).rejects.toBe(cleanupFailure);
  await expect(
    port.acquire(vi.fn()).present({
      kind: "projectOperationFailure",
      message: "Falha posterior",
    }),
  ).rejects.toBe(cleanupFailure);
});
