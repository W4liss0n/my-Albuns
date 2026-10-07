import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { expect, test, vi } from "vitest";

import type { Logger } from "../../application/logging";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
  ProjectDialogSession,
} from "../../application/projectDialogPort";
import {
  ProjectCloseError,
  type ProjectCloseResolution,
  type ProjectWindowPort,
} from "../../application/projectPorts";
import type { ProjectMutationOutcome } from "../../application/projectMutation";
import { representativeProjection } from "../../test/projectFixtures";
import { LoggingProvider } from "../loggingContext";
import { useProjectCloseController } from "./useProjectCloseController";

const confirmation = { busy: false, kind: "projectCloseConfirmation" as const };
const busyConfirmation = { busy: true, kind: "projectCloseConfirmation" as const };
const cancelled: ProjectCloseResolution = {
  kind: "cancelled",
  projection: representativeProjection,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolver, rejecter) => {
    resolve = resolver;
    reject = rejecter;
  });
  return { promise, reject, resolve };
}

function closeHarness(
  initialProps: { formatConversionPending?: boolean; requestBlocked?: boolean } = {},
) {
  const sessions: Array<{
    dismiss: ReturnType<typeof vi.fn<ProjectDialogSession["dismiss"]>>;
    emit(action: ProjectDialogAction): void;
    present: ReturnType<typeof vi.fn<ProjectDialogSession["present"]>>;
  }> = [];
  // Every acquired session shares these, so a test can fail the n-th
  // presentation without knowing which session will carry it.
  const present = vi.fn<ProjectDialogSession["present"]>(async () => undefined);
  const dismiss = vi.fn<ProjectDialogSession["dismiss"]>(async () => undefined);
  const acquire = vi.fn<ProjectDialogPort["acquire"]>((listener) => {
    const session = { dismiss, emit: listener, present };
    sessions.push(session);
    return session;
  });
  let closeRequested: (() => void) | null = null;
  const unsubscribe = vi.fn(() => { closeRequested = null; });
  const windowPort = {
    onCloseRequested: vi.fn<ProjectWindowPort["onCloseRequested"]>(async (listener) => {
      closeRequested = listener;
      return unsubscribe;
    }),
    requestClose: vi.fn<ProjectWindowPort["requestClose"]>(async () => ({
      kind: "confirmationRequired",
    })),
    resolveClose: vi.fn<ProjectWindowPort["resolveClose"]>(async () => ({
      kind: "closed",
    })),
  };
  const waitForPendingMutations = vi.fn(
    async (): Promise<ProjectMutationOutcome | null> => null,
  );
  const onProjectionChange = vi.fn();
  const onError = vi.fn();
  const write = vi.fn<Logger["write"]>();
  const projectDialogPort: ProjectDialogPort = { acquire };
  const view = renderHook(
    (props: { formatConversionPending?: boolean; requestBlocked?: boolean }) =>
      useProjectCloseController({
        ...props,
        onError,
        onProjectionChange,
        projectDialogPort,
        projectWindowPort: windowPort,
        waitForPendingMutations,
      }),
    {
      initialProps,
      wrapper: ({ children }: { children: ReactNode }) => (
        <LoggingProvider logger={{ write }}>{children}</LoggingProvider>
      ),
    },
  );
  return {
    ...view,
    acquire,
    dismiss,
    emit: (action: ProjectDialogAction) => sessions[sessions.length - 1]?.emit(action),
    emitCloseRequested: () => closeRequested?.(),
    /** The close diagnostics written so far, as `[event, reason]`. */
    loggedClose: () => write.mock.calls.map(([event]) => [event.event, event.reason]),
    onError,
    onProjectionChange,
    present,
    unsubscribe,
    waitForPendingMutations,
    windowPort,
    /** Opens the confirmation through a native close request. */
    async openConfirmation() {
      await waitFor(() => expect(windowPort.onCloseRequested).toHaveBeenCalled());
      await act(async () => closeRequested?.());
      expect(present).toHaveBeenCalledOnce();
    },
  };
}

test.each(["native request", "application command"] as const)(
  "a %s on an old myAlbuns Project keeps the format conversion warning through the decision",
  async (origin) => {
    const harness = closeHarness({ formatConversionPending: true });
    await waitFor(() => expect(harness.windowPort.onCloseRequested).toHaveBeenCalled());

    if (origin === "native request") {
      await act(async () => harness.emitCloseRequested());
    } else {
      await act(async () => { await harness.result.current.requestClose(); });
    }

    expect(harness.present).toHaveBeenCalledExactlyOnceWith({
      ...confirmation,
      formatConversion: true,
    });

    await act(async () => harness.emit("saveAndClose"));

    expect(harness.present).toHaveBeenLastCalledWith({
      ...busyConfirmation,
      formatConversion: true,
    });
    expect(harness.windowPort.resolveClose).toHaveBeenCalledExactlyOnceWith("saveAndClose");
  },
);

test.each([false, undefined])(
  "a Project in the current format (%s) closes without the format conversion warning",
  async (formatConversionPending) => {
    const harness = closeHarness({ formatConversionPending });
    await harness.openConfirmation();
    await act(async () => harness.emit("saveAndClose"));

    expect(harness.present.mock.calls).toEqual([[confirmation], [busyConfirmation]]);
    for (const [state] of harness.present.mock.calls) {
      expect(state).not.toHaveProperty("formatConversion");
    }
  },
);

test("the format conversion warning follows the Project state current at the close request", async () => {
  const harness = closeHarness({ formatConversionPending: true });
  await waitFor(() => expect(harness.windowPort.onCloseRequested).toHaveBeenCalled());

  // A save converted the file before the close was requested.
  harness.rerender({ formatConversionPending: false });
  await act(async () => harness.emitCloseRequested());

  expect(harness.present).toHaveBeenCalledExactlyOnceWith(confirmation);
});

test("a second decision arriving while the first is resolving is ignored", async () => {
  const harness = closeHarness();
  const resolution = deferred<ProjectCloseResolution>();
  harness.windowPort.resolveClose.mockReturnValueOnce(resolution.promise);
  await harness.openConfirmation();

  act(() => {
    harness.emit("saveAndClose");
    harness.emit("discardAndClose");
    harness.emit("cancelProjectClose");
  });

  expect(harness.windowPort.resolveClose).toHaveBeenCalledExactlyOnceWith("saveAndClose");
  expect(harness.present).toHaveBeenCalledTimes(2);

  await act(async () => resolution.resolve({ kind: "closed" }));
  expect(harness.windowPort.resolveClose).toHaveBeenCalledOnce();
  expect(harness.result.current.interactionBlocked).toBe(true);
});

test("a cancelled Save and close keeps the window open and re-enables the workspace", async () => {
  const harness = closeHarness();
  const resolution = deferred<ProjectCloseResolution>();
  harness.windowPort.resolveClose.mockReturnValueOnce(resolution.promise);
  await harness.openConfirmation();

  act(() => harness.emit("saveAndClose"));
  expect(harness.result.current.interactionBlocked).toBe(true);
  expect(harness.dismiss).not.toHaveBeenCalled();

  await act(async () => resolution.resolve(cancelled));

  expect(harness.onProjectionChange).toHaveBeenCalledExactlyOnceWith(representativeProjection);
  expect(harness.result.current.interactionBlocked).toBe(false);
  expect(harness.dismiss).toHaveBeenCalledOnce();
  // Only an explicit Cancel counts as the user backing out of the close.
  expect(harness.result.current.explicitCancelRevision).toBe(0);
  expect(harness.onError).not.toHaveBeenCalled();

  // The close can be requested again from the restored idle state.
  await act(async () => harness.emitCloseRequested());
  expect(harness.present).toHaveBeenLastCalledWith(confirmation);
  expect(harness.acquire).toHaveBeenCalledTimes(2);
});

test("an explicit Cancel is reported and never shows the busy confirmation", async () => {
  const harness = closeHarness();
  harness.windowPort.resolveClose.mockResolvedValueOnce(cancelled);
  await harness.openConfirmation();

  await act(async () => harness.emit("cancelProjectClose"));

  expect(harness.windowPort.resolveClose).toHaveBeenCalledExactlyOnceWith("cancel");
  expect(harness.present).toHaveBeenCalledExactlyOnceWith(confirmation);
  expect(harness.result.current.explicitCancelRevision).toBe(1);
  expect(harness.result.current.interactionBlocked).toBe(false);
  expect(harness.dismiss).toHaveBeenCalledOnce();
});

test.each([
  { reason: new ProjectCloseError("close_unavailable", "Fechamento indisponível."), message: "Fechamento indisponível." },
  { reason: "not an error", message: "Não foi possível concluir o fechamento do projeto." },
])("a rejected close request reports $message and returns to idle", async ({ reason, message }) => {
  const harness = closeHarness();
  harness.windowPort.requestClose.mockRejectedValueOnce(reason);

  let outcome: unknown = "pending";
  await act(async () => { outcome = await harness.result.current.requestClose(); });

  expect(outcome).toBeNull();
  expect(harness.onError).toHaveBeenCalledExactlyOnceWith(message);
  expect(harness.acquire).not.toHaveBeenCalled();
  expect(harness.windowPort.resolveClose).not.toHaveBeenCalled();
  expect(harness.result.current.interactionBlocked).toBe(false);

  await act(async () => { outcome = await harness.result.current.requestClose(); });
  expect(outcome).toEqual({ kind: "confirmationRequired" });
  expect(harness.present).toHaveBeenCalledExactlyOnceWith(confirmation);
});

test("a close request answered as closed ends the session without a confirmation", async () => {
  const harness = closeHarness();
  harness.windowPort.requestClose.mockResolvedValueOnce({ kind: "closed" });

  let outcome: unknown;
  await act(async () => { outcome = await harness.result.current.requestClose(); });

  expect(outcome).toEqual({ kind: "closed" });
  expect(harness.acquire).not.toHaveBeenCalled();
  expect(harness.result.current.interactionBlocked).toBe(true);
});

test.each([
  { status: "obsolete" } as const,
  { status: "failed", error: new Error("Mutation failed") } as const,
])("an application close after a $status pending mutation never reaches the window", async (pending) => {
  const harness = closeHarness();
  harness.waitForPendingMutations.mockResolvedValueOnce(pending);

  let outcome: unknown = "pending";
  await act(async () => { outcome = await harness.result.current.requestClose(); });

  expect(outcome).toBeNull();
  expect(harness.windowPort.requestClose).not.toHaveBeenCalled();
  expect(harness.acquire).not.toHaveBeenCalled();
  expect(harness.onError).not.toHaveBeenCalled();
  expect(harness.result.current.interactionBlocked).toBe(false);
  expect(harness.loggedClose()).toContainEqual(["project_close_request_dropped", pending.status]);
});

test.each([
  { status: "obsolete" } as const,
  { status: "failed", error: new Error("Mutation failed") } as const,
])("a native close after a $status pending mutation is released without a confirmation", async (pending) => {
  const harness = closeHarness();
  harness.waitForPendingMutations.mockResolvedValueOnce(pending);
  harness.windowPort.resolveClose.mockResolvedValueOnce(cancelled);
  await waitFor(() => expect(harness.windowPort.onCloseRequested).toHaveBeenCalled());

  await act(async () => harness.emitCloseRequested());

  expect(harness.windowPort.resolveClose).toHaveBeenCalledExactlyOnceWith("cancel");
  expect(harness.acquire).not.toHaveBeenCalled();
  expect(harness.onProjectionChange).toHaveBeenCalledExactlyOnceWith(representativeProjection);
  expect(harness.onError).not.toHaveBeenCalled();
  expect(harness.result.current.interactionBlocked).toBe(false);
  expect(harness.loggedClose()).toEqual([
    ["project_close_native_received", undefined],
    ["project_close_native_released", pending.status],
  ]);
});

test("a native close that cannot be released reports the failure and returns to idle", async () => {
  const harness = closeHarness();
  harness.waitForPendingMutations.mockResolvedValueOnce({ status: "obsolete" });
  harness.windowPort.resolveClose.mockRejectedValueOnce(new Error("Release failed"));
  await waitFor(() => expect(harness.windowPort.onCloseRequested).toHaveBeenCalled());

  await act(async () => harness.emitCloseRequested());

  expect(harness.onError).toHaveBeenCalledExactlyOnceWith("Release failed");
  expect(harness.onProjectionChange).not.toHaveBeenCalled();
  expect(harness.result.current.interactionBlocked).toBe(false);
});

test("a blocked workspace releases a native close and ignores the application command", async () => {
  const harness = closeHarness({ requestBlocked: true });
  harness.windowPort.resolveClose.mockResolvedValueOnce(cancelled);
  await waitFor(() => expect(harness.windowPort.onCloseRequested).toHaveBeenCalled());

  let outcome: unknown = "pending";
  await act(async () => { outcome = await harness.result.current.requestClose(); });
  expect(outcome).toBeNull();
  expect(harness.waitForPendingMutations).not.toHaveBeenCalled();
  expect(harness.windowPort.requestClose).not.toHaveBeenCalled();

  await act(async () => harness.emitCloseRequested());

  expect(harness.windowPort.resolveClose).toHaveBeenCalledExactlyOnceWith("cancel");
  expect(harness.waitForPendingMutations).not.toHaveBeenCalled();
  expect(harness.acquire).not.toHaveBeenCalled();
  expect(harness.result.current.interactionBlocked).toBe(false);
  expect(harness.loggedClose()).toEqual([
    ["project_close_request_ignored", "workspace_blocked"],
    ["project_close_native_received", undefined],
    ["project_close_native_released", "workspace_blocked"],
  ]);
});

test("a confirmation that cannot be presented cancels the close and reports why", async () => {
  const harness = closeHarness();
  harness.present.mockRejectedValueOnce(new Error("Dialog unavailable"));
  harness.windowPort.resolveClose.mockResolvedValueOnce(cancelled);
  await waitFor(() => expect(harness.windowPort.onCloseRequested).toHaveBeenCalled());

  await act(async () => harness.emitCloseRequested());

  expect(harness.windowPort.resolveClose).toHaveBeenCalledExactlyOnceWith("cancel");
  expect(harness.dismiss).toHaveBeenCalledOnce();
  expect(harness.onProjectionChange).toHaveBeenCalledExactlyOnceWith(representativeProjection);
  expect(harness.onError).toHaveBeenCalledExactlyOnceWith("Dialog unavailable");
  expect(harness.result.current.interactionBlocked).toBe(false);
  expect(harness.result.current.explicitCancelRevision).toBe(0);
});

test("a failed confirmation whose cancellation closes the window keeps the workspace blocked", async () => {
  const harness = closeHarness();
  harness.present.mockRejectedValueOnce(new Error("Dialog unavailable"));
  await waitFor(() => expect(harness.windowPort.onCloseRequested).toHaveBeenCalled());

  await act(async () => harness.emitCloseRequested());

  expect(harness.windowPort.resolveClose).toHaveBeenCalledExactlyOnceWith("cancel");
  expect(harness.onProjectionChange).not.toHaveBeenCalled();
  expect(harness.onError).toHaveBeenCalledExactlyOnceWith("Dialog unavailable");
  expect(harness.result.current.interactionBlocked).toBe(true);
});

test("a failed confirmation whose cancellation also fails reports the cancellation failure", async () => {
  const harness = closeHarness();
  harness.present.mockRejectedValueOnce(new Error("Dialog unavailable"));
  harness.windowPort.resolveClose.mockRejectedValueOnce(new Error("Release failed"));
  await waitFor(() => expect(harness.windowPort.onCloseRequested).toHaveBeenCalled());

  await act(async () => harness.emitCloseRequested());

  expect(harness.onError).toHaveBeenCalledExactlyOnceWith("Release failed");
  expect(harness.result.current.interactionBlocked).toBe(true);
});

test("a busy confirmation that cannot be presented still resolves the chosen close", async () => {
  const harness = closeHarness();
  const resolution = deferred<ProjectCloseResolution>();
  harness.windowPort.resolveClose.mockReturnValueOnce(resolution.promise);
  await harness.openConfirmation();
  harness.present.mockRejectedValueOnce(new Error("Dialog lost"));

  await act(async () => harness.emit("discardAndClose"));

  expect(harness.windowPort.resolveClose).toHaveBeenCalledExactlyOnceWith("discardAndClose");
  expect(harness.onError).toHaveBeenCalledExactlyOnceWith("Dialog lost");
  expect(harness.dismiss).toHaveBeenCalledOnce();

  // The lost session is not reused: the failure is shown in a fresh one.
  await act(async () => resolution.reject(new ProjectCloseError("io_failure", "Falha ao salvar.")));

  expect(harness.acquire).toHaveBeenCalledTimes(2);
  expect(harness.present).toHaveBeenLastCalledWith({
    kind: "projectCloseFailure",
    message: "Falha ao salvar.",
  });
  expect(harness.onError).toHaveBeenCalledOnce();
});

test("a conclusive close failure is shown in the open dialog and dismissing it resumes the Project", async () => {
  const harness = closeHarness();
  harness.windowPort.resolveClose.mockRejectedValueOnce(
    new ProjectCloseError("io_failure", "Falha ao salvar."),
  );
  await harness.openConfirmation();

  await act(async () => harness.emit("saveAndClose"));

  expect(harness.acquire).toHaveBeenCalledOnce();
  expect(harness.present).toHaveBeenLastCalledWith({
    kind: "projectCloseFailure",
    message: "Falha ao salvar.",
  });
  expect(harness.result.current.interactionBlocked).toBe(true);
  expect(harness.onError).not.toHaveBeenCalled();

  act(() => harness.emit("dismissProjectCloseFailure"));

  expect(harness.dismiss).toHaveBeenCalledOnce();
  expect(harness.result.current.interactionBlocked).toBe(false);
});

test.each([
  { code: "io_failure", blocked: false } as const,
  { code: "save_state_indeterminate", blocked: true } as const,
])("a $code failure dialog that cannot be presented falls back to the workspace message", async ({ code, blocked }) => {
  const harness = closeHarness();
  harness.windowPort.resolveClose.mockRejectedValueOnce(
    new ProjectCloseError(code, "Falha ao salvar."),
  );
  await harness.openConfirmation();
  // Busy confirmation succeeds; only the failure presentation is lost.
  harness.present
    .mockResolvedValueOnce(undefined)
    .mockRejectedValueOnce(new Error("Dialog lost"));

  await act(async () => harness.emit("saveAndClose"));

  expect(harness.present).toHaveBeenLastCalledWith({
    kind: "projectCloseFailure",
    message: "Falha ao salvar.",
  });
  expect(harness.onError).toHaveBeenCalledExactlyOnceWith("Falha ao salvar.");
  expect(harness.dismiss).toHaveBeenCalledOnce();
  // An indeterminate save never returns the Project to editing.
  expect(harness.result.current.interactionBlocked).toBe(blocked);
});

test("an indeterminate save failure stays terminal after its dialog is dismissed", async () => {
  const harness = closeHarness();
  harness.windowPort.resolveClose.mockRejectedValueOnce(
    new ProjectCloseError("save_state_indeterminate", "Estado indeterminado."),
  );
  await harness.openConfirmation();

  await act(async () => harness.emit("saveAndClose"));
  act(() => harness.emit("dismissProjectCloseFailure"));

  expect(harness.dismiss).toHaveBeenCalledOnce();
  expect(harness.result.current.interactionBlocked).toBe(true);
});

test("a second close request while a decision is pending is ignored", async () => {
  const harness = closeHarness();
  await harness.openConfirmation();

  let outcome: unknown = "pending";
  await act(async () => {
    harness.emitCloseRequested();
    outcome = await harness.result.current.requestClose();
  });

  expect(outcome).toBeNull();
  expect(harness.waitForPendingMutations).toHaveBeenCalledOnce();
  expect(harness.acquire).toHaveBeenCalledOnce();
  expect(harness.present).toHaveBeenCalledExactlyOnceWith(confirmation);
  expect(harness.windowPort.requestClose).not.toHaveBeenCalled();
  expect(harness.windowPort.resolveClose).not.toHaveBeenCalled();
  expect(harness.loggedClose()).toEqual([
    ["project_close_native_received", undefined],
    ["project_close_native_ignored", "deciding"],
    ["project_close_request_ignored", "deciding"],
  ]);
});

test("a native close repeated while pending mutations settle opens one confirmation", async () => {
  const harness = closeHarness();
  const pending = deferred<ProjectMutationOutcome | null>();
  harness.waitForPendingMutations.mockReturnValueOnce(pending.promise);
  await waitFor(() => expect(harness.windowPort.onCloseRequested).toHaveBeenCalled());

  act(() => {
    harness.emitCloseRequested();
    harness.emitCloseRequested();
  });
  expect(harness.result.current.interactionBlocked).toBe(true);
  expect(harness.acquire).not.toHaveBeenCalled();

  await act(async () => pending.resolve(null));

  expect(harness.waitForPendingMutations).toHaveBeenCalledOnce();
  expect(harness.present).toHaveBeenCalledExactlyOnceWith(confirmation);
});

test("unmounting dismisses the open confirmation and stops listening for native closes", async () => {
  const harness = closeHarness();
  await harness.openConfirmation();

  harness.unmount();

  expect(harness.dismiss).toHaveBeenCalledOnce();
  expect(harness.unsubscribe).toHaveBeenCalledOnce();
});

test("a failed native close subscription is reported", async () => {
  const onCloseRequested = vi.fn<ProjectWindowPort["onCloseRequested"]>(
    async () => { throw new Error("Subscription failed"); },
  );
  const onError = vi.fn();
  const projectWindowPort: ProjectWindowPort = {
    onCloseRequested,
    requestClose: async () => ({ kind: "closed" }),
    resolveClose: async () => ({ kind: "closed" }),
  };
  const projectDialogPort: ProjectDialogPort = {
    acquire: () => { throw new Error("Unexpected dialog"); },
  };
  const waitForPendingMutations = async () => null;
  const onProjectionChange = vi.fn();

  renderHook(() => useProjectCloseController({
    onError,
    onProjectionChange,
    projectDialogPort,
    projectWindowPort,
    waitForPendingMutations,
  }));

  await waitFor(() => expect(onError).toHaveBeenCalledExactlyOnceWith("Subscription failed"));
});
