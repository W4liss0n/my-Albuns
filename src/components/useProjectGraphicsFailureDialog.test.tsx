import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { GraphicsDiagnostic } from "../application/graphics";
import type {
  ProjectDialogAction,
  ProjectDialogPort,
} from "../application/projectDialogPort";
import { useProjectGraphicsFailureDialog } from "./useProjectGraphicsFailureDialog";

const diagnostic: Extract<GraphicsDiagnostic, { supported: false }> = {
  supported: false,
  code: "webgl2_unavailable",
  renderer: "indisponível",
  reason: "O contexto WebGL2 foi perdido.",
  limits: null,
};

function dialogHarness() {
  const sessions: Array<{
    dismiss: ReturnType<typeof vi.fn>;
    listener: (action: ProjectDialogAction) => void;
    present: ReturnType<typeof vi.fn>;
  }> = [];
  const port: ProjectDialogPort = {
    acquire: (listener) => {
      const session = {
        dismiss: vi.fn(async () => undefined),
        listener,
        present: vi.fn(async () => undefined),
      };
      sessions.push(session);
      return session;
    },
  };
  return { port, sessions };
}

test("keeps repeated graphics reports in one owned dialog session", async () => {
  const dialog = dialogHarness();
  const onCloseProject = vi.fn(async () => ({
    kind: "confirmationRequired" as const,
  }));
  const view = renderHook(
    ({ closeCancelRevision, currentDiagnostic }) =>
      useProjectGraphicsFailureDialog({
        closeCancelRevision,
        diagnostic: currentDiagnostic,
        onCloseProject,
        projectDialogPort: dialog.port,
      }),
    {
      initialProps: {
        closeCancelRevision: 0,
        currentDiagnostic: diagnostic,
      },
    },
  );

  await waitFor(() => expect(dialog.sessions).toHaveLength(1));
  expect(dialog.sessions[0]?.present).toHaveBeenCalledWith({
    kind: "graphicsFailure",
    reason: diagnostic.reason,
  });

  view.rerender({
    closeCancelRevision: 0,
    currentDiagnostic: { ...diagnostic },
  });
  expect(dialog.sessions).toHaveLength(1);
  expect(dialog.sessions[0]?.present).toHaveBeenCalledOnce();

  view.rerender({
    closeCancelRevision: 0,
    currentDiagnostic: {
      ...diagnostic,
      reason: "O contexto WebGL2 não pôde ser restaurado.",
    },
  });
  await waitFor(() =>
    expect(dialog.sessions[0]?.present).toHaveBeenCalledTimes(2),
  );
  expect(dialog.sessions[0]?.present).toHaveBeenLastCalledWith({
    kind: "graphicsFailure",
    reason: "O contexto WebGL2 não pôde ser restaurado.",
  });

  act(() =>
    dialog.sessions[0]?.listener("closeProjectAfterGraphicsFailure"),
  );
  await waitFor(() => expect(dialog.sessions[0]?.dismiss).toHaveBeenCalledOnce());
  await waitFor(() => expect(onCloseProject).toHaveBeenCalledOnce());
  view.rerender({
    closeCancelRevision: 1,
    currentDiagnostic: {
      ...diagnostic,
      reason: "O contexto WebGL2 não pôde ser restaurado.",
    },
  });
  await waitFor(() => expect(dialog.sessions).toHaveLength(2));
  expect(dialog.sessions[1]?.present).toHaveBeenCalledWith({
    kind: "graphicsFailure",
    reason: "O contexto WebGL2 não pôde ser restaurado.",
  });

  view.unmount();
  expect(dialog.sessions[1]?.dismiss).toHaveBeenCalledOnce();
});

test("does not create a successor dialog after the Project closes", async () => {
  const dialog = dialogHarness();
  const onCloseProject = vi.fn(async () => ({ kind: "closed" as const }));
  renderHook(() =>
    useProjectGraphicsFailureDialog({
      closeCancelRevision: 0,
      diagnostic,
      onCloseProject,
      projectDialogPort: dialog.port,
    }),
  );

  await waitFor(() => expect(dialog.sessions).toHaveLength(1));
  act(() =>
    dialog.sessions[0]?.listener("closeProjectAfterGraphicsFailure"),
  );
  await waitFor(() => expect(onCloseProject).toHaveBeenCalledOnce());
  expect(dialog.sessions).toHaveLength(1);
});

test("rearms the graphics owner when its first presentation fails and close is cancelled", async () => {
  const dialog = dialogHarness();
  const presentationFailure = new Error("A janela pertencente falhou.");
  const onCloseProject = vi.fn(async () => ({
    kind: "confirmationRequired" as const,
  }));
  dialog.port.acquire = (listener) => {
    const session = {
      dismiss: vi.fn(async () => undefined),
      listener,
      present: vi.fn(async () => undefined),
    };
    if (dialog.sessions.length === 0) {
      session.present.mockRejectedValueOnce(presentationFailure);
    }
    dialog.sessions.push(session);
    return session;
  };

  const view = renderHook(
    ({ closeCancelRevision }) =>
      useProjectGraphicsFailureDialog({
        closeCancelRevision,
        diagnostic,
        onCloseProject,
        projectDialogPort: dialog.port,
      }),
    { initialProps: { closeCancelRevision: 0 } },
  );

  await waitFor(() => expect(onCloseProject).toHaveBeenCalledOnce());
  await act(async () => Promise.resolve());
  view.rerender({ closeCancelRevision: 1 });
  await waitFor(() => expect(dialog.sessions).toHaveLength(2));
  expect(dialog.sessions[0]?.dismiss).toHaveBeenCalledOnce();
  expect(dialog.sessions[1]?.present).toHaveBeenCalledWith({
    kind: "graphicsFailure",
    reason: diagnostic.reason,
  });
});

test("bounds automatic recovery but rearms after every cancelled close terminal", async () => {
  const dialog = dialogHarness();
  const onCloseProject = vi.fn(async () => ({
    kind: "confirmationRequired" as const,
  }));
  dialog.port.acquire = (listener) => {
    const session = {
      dismiss: vi.fn(async () => undefined),
      listener,
      present: vi.fn(async () => {
        throw new Error("A janela pertencente continua indisponível.");
      }),
    };
    dialog.sessions.push(session);
    return session;
  };

  const view = renderHook(
    ({ closeCancelRevision }) =>
      useProjectGraphicsFailureDialog({
        closeCancelRevision,
        diagnostic,
        onCloseProject,
        projectDialogPort: dialog.port,
      }),
    { initialProps: { closeCancelRevision: 0 } },
  );

  await waitFor(() => expect(onCloseProject).toHaveBeenCalledOnce());
  await act(async () => Promise.resolve());
  view.rerender({ closeCancelRevision: 1 });
  await waitFor(() => expect(dialog.sessions).toHaveLength(2));
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(dialog.sessions).toHaveLength(2);
  expect(onCloseProject).toHaveBeenCalledTimes(2);

  view.rerender({ closeCancelRevision: 2 });
  await waitFor(() => expect(dialog.sessions).toHaveLength(3));
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(dialog.sessions).toHaveLength(3);
  expect(onCloseProject).toHaveBeenCalledTimes(3);
});
