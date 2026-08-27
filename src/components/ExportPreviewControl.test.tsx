import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
  ProjectDialogSession,
} from "../application/projectDialogPort";
import type {
  ExportAttempt,
  ExportCancelStatus,
  ExportOutcome,
  ExportPipelinePort,
  ExportProgressEvent,
} from "../application/projectPorts";
import { ExportPreviewControl } from "./ExportPreviewControl";

interface AttemptHarness {
  cancel: ReturnType<typeof vi.fn<() => Promise<ExportCancelStatus>>>;
  emit(event: ExportProgressEvent): void;
  reject(error: unknown): void;
  resolve(outcome: ExportOutcome): void;
}

function createExportHarness() {
  const attempts: AttemptHarness[] = [];
  const startSheet = vi.fn<ExportPipelinePort["startSheet"]>((_selection, onEvent) => {
    let resolve!: (outcome: ExportOutcome) => void;
    let reject!: (error: unknown) => void;
    const completion = new Promise<ExportOutcome>((resolvePromise, rejectPromise) => {
      resolve = resolvePromise;
      reject = rejectPromise;
    });
    const cancel = vi.fn(async (): Promise<ExportCancelStatus> => "requested");
    const attempt: ExportAttempt = { completion, cancel };
    attempts.push({ cancel, emit: onEvent, reject, resolve });
    return attempt;
  });

  return {
    attempts,
    port: { startSheet } satisfies ExportPipelinePort,
    startSheet,
  };
}

function createDialogHarness() {
  let listener: ((action: ProjectDialogAction) => void) | undefined;
  const dismiss = vi.fn(async () => undefined);
  const present = vi.fn<ProjectDialogSession["present"]>(
    async () => undefined,
  );
  const onAction = vi.fn((nextListener: (action: ProjectDialogAction) => void) => {
    listener = nextListener;
  });
  const acquire: ProjectDialogPort["acquire"] = (nextListener) => {
    onAction(nextListener);
    let active = true;
    return {
      dismiss: async () => {
        await dismiss();
        if (!active) return;
        active = false;
        if (listener === nextListener) listener = undefined;
      },
      present,
    };
  };

  return {
    dismiss,
    emit(action: ProjectDialogAction) {
      act(() => listener?.(action));
    },
    onAction,
    port: { acquire } satisfies ProjectDialogPort,
    present,
  };
}

function renderControl({
  dialog = createDialogHarness(),
  exportHarness = createExportHarness(),
  onActiveChange,
  projectId = "project-a",
}: {
  dialog?: ReturnType<typeof createDialogHarness>;
  exportHarness?: ReturnType<typeof createExportHarness>;
  onActiveChange?: (active: boolean) => void;
  projectId?: string;
} = {}) {
  const view = render(
    <ExportPreviewControl
      dialogPort={dialog.port}
      exportPipelinePort={exportHarness.port}
      onActiveChange={onActiveChange}
      projectId={projectId}
      selection={{
        projectName: "Projeto de teste",
        sheetId: "sheet-001",
        sheetNumber: 1,
      }}
    />,
  );
  return { dialog, exportHarness, view };
}

afterEach(() => {
  vi.useRealTimers();
});

test("waits for the backend started event before opening the native progress window", async () => {
  const user = userEvent.setup();
  const { dialog, exportHarness } = renderControl();
  await user.click(screen.getByRole("button", { name: "Exportar Lâmina" }));

  expect(exportHarness.startSheet).toHaveBeenCalledWith(
    {
      projectName: "Projeto de teste",
      sheetId: "sheet-001",
      sheetNumber: 1,
    },
    expect.any(Function),
  );
  expect(dialog.present).not.toHaveBeenCalled();

  act(() => {
    exportHarness.attempts[0].emit({ event: "started", cancellable: false });
  });

  expect(dialog.present).toHaveBeenCalledWith({
    cancelRequested: false,
    cancellable: false,
    kind: "exportProgress",
    progress: {
      kind: "indeterminate",
      status: "Iniciando a Exportação",
    },
  });
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

test("projects measured and unmeasured progress through the dialog port", async () => {
  const user = userEvent.setup();
  const { dialog, exportHarness } = renderControl();
  await user.click(screen.getByRole("button", { name: "Exportar Lâmina" }));
  act(() => {
    exportHarness.attempts[0].emit({ event: "started", cancellable: true });
    exportHarness.attempts[0].emit({
      event: "progress",
      stage: "preparing",
      units: { kind: "unmeasured" },
      cancellable: true,
    });
    exportHarness.attempts[0].emit({
      event: "progress",
      stage: "composing",
      units: { kind: "measured", completedUnits: 2, totalUnits: 5 },
      cancellable: true,
    });
  });

  expect(dialog.present).toHaveBeenNthCalledWith(2, {
    cancelRequested: false,
    cancellable: true,
    kind: "exportProgress",
    progress: {
      kind: "indeterminate",
      status: "Preparando a prova",
    },
  });
  expect(dialog.present).toHaveBeenNthCalledWith(3, {
    cancelRequested: false,
    cancellable: true,
    kind: "exportProgress",
    progress: {
      completed: 2,
      kind: "determinate",
      status: "Compondo a prova",
      total: 5,
    },
  });
});

test("handles cancellation actions from the child window and keeps feedback there", async () => {
  const user = userEvent.setup();
  const { dialog, exportHarness } = renderControl();
  await user.click(screen.getByRole("button", { name: "Exportar Lâmina" }));
  act(() => {
    exportHarness.attempts[0].emit({ event: "started", cancellable: true });
  });
  await waitFor(() => expect(dialog.onAction).toHaveBeenCalledOnce());

  dialog.emit("cancelExport");
  dialog.emit("cancelExport");
  expect(exportHarness.attempts[0].cancel).toHaveBeenCalledOnce();
  expect(dialog.present).toHaveBeenLastCalledWith({
    cancelRequested: true,
    cancellable: true,
    kind: "exportProgress",
    progress: expect.objectContaining({ kind: "indeterminate" }),
  });

  await act(async () => {
    exportHarness.attempts[0].resolve({ status: "cancelled" });
    await Promise.resolve();
  });
  expect(dialog.present).toHaveBeenLastCalledWith({
    cancelled: true,
    kind: "exportFailure",
    message: "A Exportação foi cancelada.",
    retryDisabled: false,
  });
});

test("opens the standard failure dialog for a pre-start conflict", async () => {
  const user = userEvent.setup();
  const onActiveChange = vi.fn();
  const { dialog, exportHarness } = renderControl({ onActiveChange });
  await user.click(screen.getByRole("button", { name: "Exportar Lâmina" }));

  await act(async () => {
    exportHarness.attempts[0].reject({
      code: "conflict",
      message: "Outra operação exclusiva já está em andamento.",
    });
    await Promise.resolve();
  });

  expect(dialog.present).toHaveBeenCalledWith({
    cancelled: false,
    kind: "exportFailure",
    message: "Outra operação exclusiva já está em andamento.",
    retryDisabled: false,
  });
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(onActiveChange.mock.calls).toEqual([[true]]);
});

test("recovers after the native progress window cannot be presented", async () => {
  const user = userEvent.setup();
  const onActiveChange = vi.fn();
  const dialog = createDialogHarness();
  dialog.present.mockRejectedValue(
    new Error("Não foi possível abrir a janela de progresso."),
  );
  const { exportHarness } = renderControl({ dialog, onActiveChange });

  await user.click(screen.getByRole("button", { name: "Exportar Lâmina" }));
  act(() => {
    exportHarness.attempts[0].emit({ event: "started", cancellable: true });
  });

  await waitFor(() => {
    expect(exportHarness.attempts[0].cancel).toHaveBeenCalledOnce();
  });
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();

  await act(async () => {
    exportHarness.attempts[0].resolve({ status: "cancelled" });
    await Promise.resolve();
  });

  expect(dialog.present).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "Exportar Lâmina" })).toBeEnabled();
  expect(onActiveChange.mock.calls).toEqual([[true], [false]]);
});

test("replaces native progress with the standard success dialog", async () => {
  const { dialog, exportHarness } = renderControl();
  fireEvent.click(screen.getByRole("button", { name: "Exportar Lâmina" }));
  act(() => {
    exportHarness.attempts[0].emit({ event: "started", cancellable: true });
  });
  await act(async () => {
    exportHarness.attempts[0].resolve({
      status: "completed",
      result: { widthPx: 7_087, heightPx: 3_543 },
    });
    await Promise.resolve();
  });

  expect(dialog.present).toHaveBeenLastCalledWith({
    kind: "exportSuccess",
    message: "A prova foi exportada com sucesso.",
  });
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
});

test("retries and dismisses terminal feedback from semantic child-window actions", async () => {
  const user = userEvent.setup();
  const { dialog, exportHarness } = renderControl();
  await user.click(screen.getByRole("button", { name: "Exportar Lâmina" }));
  act(() => {
    exportHarness.attempts[0].emit({ event: "started", cancellable: true });
  });
  await waitFor(() => expect(dialog.onAction).toHaveBeenCalledOnce());
  await act(async () => {
    exportHarness.attempts[0].reject(new Error("Mídia indisponível"));
    await Promise.resolve();
  });

  expect(dialog.present).toHaveBeenLastCalledWith({
    cancelled: false,
    kind: "exportFailure",
    message: "Mídia indisponível",
    retryDisabled: false,
  });
  dialog.emit("retryExport");
  expect(exportHarness.startSheet).toHaveBeenCalledTimes(2);
  expect(dialog.present).toHaveBeenLastCalledWith(
    expect.objectContaining({ kind: "exportFailure", retryDisabled: true }),
  );

  act(() => {
    exportHarness.attempts[1].emit({ event: "started", cancellable: true });
  });
  await act(async () => {
    exportHarness.attempts[1].reject(new Error("Ainda indisponível"));
    await Promise.resolve();
  });
  dialog.emit("dismissExport");
  expect(dialog.dismiss).toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Exportar Lâmina" })).toBeEnabled();
});

test("retires the attempt and native presentation when the Project changes", async () => {
  const user = userEvent.setup();
  const onActiveChange = vi.fn();
  const dialog = createDialogHarness();
  const exportHarness = createExportHarness();
  const { view } = renderControl({ dialog, exportHarness, onActiveChange });
  await user.click(screen.getByRole("button", { name: "Exportar Lâmina" }));
  act(() => {
    exportHarness.attempts[0].emit({ event: "started", cancellable: true });
  });

  view.rerender(
    <ExportPreviewControl
      dialogPort={dialog.port}
      exportPipelinePort={exportHarness.port}
      onActiveChange={onActiveChange}
      projectId="project-b"
      selection={{
        projectName: "Projeto de teste",
        sheetId: "sheet-001",
        sheetNumber: 1,
      }}
    />,
  );

  expect(exportHarness.attempts[0].cancel).toHaveBeenCalledOnce();
  expect(dialog.dismiss).toHaveBeenCalled();
  expect(onActiveChange.mock.calls).toEqual([[true], [false]]);
});
