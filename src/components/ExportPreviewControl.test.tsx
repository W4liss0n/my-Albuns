import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";

import type {
  ExportAttempt,
  ExportCancelStatus,
  ExportOutcome,
  ExportPort,
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
  const startPreview = vi.fn<ExportPort["startPreview"]>((onEvent) => {
    let resolve!: (outcome: ExportOutcome) => void;
    let reject!: (error: unknown) => void;
    const completion = new Promise<ExportOutcome>(
      (resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
      },
    );
    const cancel = vi.fn(async (): Promise<ExportCancelStatus> => "requested");
    const attempt: ExportAttempt = { completion, cancel };

    attempts.push({
      cancel,
      emit: onEvent,
      reject,
      resolve,
    });

    return attempt;
  });

  return {
    attempts,
    port: { startPreview } satisfies ExportPort,
    startPreview,
  };
}

afterEach(() => {
  vi.useRealTimers();
});

test("waits for the started event before showing progress", async () => {
  const user = userEvent.setup();
  const harness = createExportHarness();

  render(
    <ExportPreviewControl
      exportPort={harness.port}
      projectId="project-a"
    />,
  );

  const exportButton = screen.getByRole("button", {
    name: "Exportar prova",
  });
  await user.click(exportButton);

  expect(exportButton).toBeDisabled();
  expect(harness.startPreview).toHaveBeenCalledOnce();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

  act(() => {
    harness.attempts[0].emit({ event: "started" });
  });

  expect(
    screen.getByRole("dialog", { name: "Exportando" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("progressbar")).toHaveAttribute(
    "aria-valuenow",
    "0",
  );
  expect(
    screen.getByRole("button", { name: "Cancelar exportação" }),
  ).toBeInTheDocument();
});

test("projects progress and removes cancellation when publishing starts", async () => {
  const user = userEvent.setup();
  const harness = createExportHarness();

  render(
    <ExportPreviewControl
      exportPort={harness.port}
      projectId="project-a"
    />,
  );
  await user.click(
    screen.getByRole("button", { name: "Exportar prova" }),
  );

  act(() => {
    harness.attempts[0].emit({ event: "started" });
    harness.attempts[0].emit({
      event: "progress",
      stage: "composing",
      completedUnits: 2,
      totalUnits: 5,
      cancellable: true,
    });
  });

  expect(screen.getByText("Compondo a prova")).toBeInTheDocument();
  expect(screen.getByText("2 de 5")).toBeInTheDocument();
  expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "2");
  expect(
    screen.getByRole("button", { name: "Cancelar exportação" }),
  ).toBeInTheDocument();

  act(() => {
    harness.attempts[0].emit({
      event: "progress",
      stage: "verifying",
      completedUnits: 4,
      totalUnits: 5,
      cancellable: false,
    });
  });

  expect(
    screen.queryByRole("button", { name: "Cancelar exportação" }),
  ).not.toBeInTheDocument();

  act(() => {
    harness.attempts[0].emit({
      event: "progress",
      stage: "publishing",
      completedUnits: 5,
      totalUnits: 5,
      cancellable: true,
    });
  });

  expect(screen.getByText("Publicando a prova")).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Cancelar exportação" }),
  ).not.toBeInTheDocument();
});

test("requests cancellation once and waits for completion before showing feedback", async () => {
  const user = userEvent.setup();
  const harness = createExportHarness();

  render(
    <ExportPreviewControl
      exportPort={harness.port}
      projectId="project-a"
    />,
  );
  await user.click(
    screen.getByRole("button", { name: "Exportar prova" }),
  );
  act(() => {
    harness.attempts[0].emit({ event: "started" });
    harness.attempts[0].emit({
      event: "progress",
      stage: "composing",
      completedUnits: 2,
      totalUnits: 5,
      cancellable: true,
    });
  });

  const cancelButton = screen.getByRole("button", {
    name: "Cancelar exportação",
  });
  await user.click(cancelButton);
  await user.click(cancelButton);

  expect(harness.attempts[0].cancel).toHaveBeenCalledOnce();
  expect(
    screen.getByRole("dialog", { name: "Exportando" }),
  ).toBeInTheDocument();
  expect(screen.getByText("Cancelando…")).toBeInTheDocument();

  await act(async () => {
    harness.attempts[0].resolve({ status: "cancelled" });
    await Promise.resolve();
  });

  expect(
    screen.queryByRole("dialog", { name: "Exportando" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("dialog", { name: "Exportação cancelada" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Tentar novamente" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Fechar" })).toBeInTheDocument();
});

test("closes progress and confirms a completed Export briefly", async () => {
  vi.useFakeTimers();
  const harness = createExportHarness();

  render(
    <ExportPreviewControl
      exportPort={harness.port}
      projectId="project-a"
    />,
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Exportar prova" }),
  );
  act(() => {
    harness.attempts[0].emit({ event: "started" });
  });

  await act(async () => {
    harness.attempts[0].resolve({
      status: "completed",
      result: {
        outputPath: "C:/exports/Album_001.jpg",
        widthPx: 7_087,
        heightPx: 3_543,
      },
    });
    await Promise.resolve();
  });

  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.getByRole("status")).toHaveTextContent(
    "Exportação concluída",
  );
  expect(
    screen.getByRole("button", { name: "Exportar prova" }),
  ).toBeEnabled();

  act(() => {
    vi.advanceTimersByTime(4_000);
  });

  expect(screen.queryByRole("status")).not.toBeInTheDocument();
});

test("offers retry and close in feedback after an Export failure", async () => {
  const user = userEvent.setup();
  const harness = createExportHarness();

  render(
    <ExportPreviewControl
      exportPort={harness.port}
      projectId="project-a"
    />,
  );
  await user.click(
    screen.getByRole("button", { name: "Exportar prova" }),
  );
  act(() => {
    harness.attempts[0].emit({ event: "started" });
  });

  await act(async () => {
    harness.attempts[0].reject({
      code: "conflict",
      message: "Outra operação exclusiva já está em andamento.",
    });
    await Promise.resolve();
  });

  expect(
    screen.queryByRole("dialog", { name: "Exportando" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("dialog", { name: "Exportação não concluída" }),
  ).toHaveTextContent("Outra operação exclusiva já está em andamento.");

  await user.click(
    screen.getByRole("button", { name: "Tentar novamente" }),
  );

  expect(harness.startPreview).toHaveBeenCalledTimes(2);
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Exportar prova" }),
  ).toBeDisabled();

  act(() => {
    harness.attempts[1].emit({ event: "started" });
  });
  await act(async () => {
    harness.attempts[1].reject(new Error("still unavailable"));
    await Promise.resolve();
  });
  await user.click(screen.getByRole("button", { name: "Fechar" }));

  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Exportar prova" }),
  ).toBeEnabled();
});

test("retires an active attempt when the Project changes or the control unmounts", async () => {
  const user = userEvent.setup();
  const harness = createExportHarness();
  const onActiveChange = vi.fn();
  const view = render(
    <ExportPreviewControl
      exportPort={harness.port}
      onActiveChange={onActiveChange}
      projectId="project-a"
    />,
  );

  await user.click(
    screen.getByRole("button", { name: "Exportar prova" }),
  );
  act(() => {
    harness.attempts[0].emit({ event: "started" });
  });
  expect(onActiveChange.mock.calls).toEqual([[true]]);

  view.rerender(
    <ExportPreviewControl
      exportPort={harness.port}
      onActiveChange={onActiveChange}
      projectId="project-b"
    />,
  );

  expect(harness.attempts[0].cancel).toHaveBeenCalledOnce();
  expect(onActiveChange.mock.calls).toEqual([[true], [false]]);
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

  act(() => {
    harness.attempts[0].emit({
      event: "progress",
      stage: "publishing",
      completedUnits: 5,
      totalUnits: 5,
      cancellable: false,
    });
  });
  await act(async () => {
    harness.attempts[0].resolve({
      status: "completed",
      result: {
        outputPath: "C:/exports/stale.jpg",
        widthPx: 100,
        heightPx: 50,
      },
    });
    await Promise.resolve();
  });

  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.queryByRole("status")).not.toBeInTheDocument();

  await user.click(
    screen.getByRole("button", { name: "Exportar prova" }),
  );
  expect(onActiveChange.mock.calls).toEqual([[true], [false], [true]]);

  view.unmount();

  expect(harness.attempts[1].cancel).toHaveBeenCalledOnce();
  expect(onActiveChange.mock.calls).toEqual([
    [true],
    [false],
    [true],
    [false],
  ]);

  await act(async () => {
    harness.attempts[1].resolve({ status: "cancelled" });
    await Promise.resolve();
  });
  expect(onActiveChange).toHaveBeenCalledTimes(4);
});

test("honors external disabling and balances active notifications at the terminal result", async () => {
  const harness = createExportHarness();
  const onActiveChange = vi.fn();
  const view = render(
    <ExportPreviewControl
      disabled
      exportPort={harness.port}
      onActiveChange={onActiveChange}
      projectId="project-a"
    />,
  );

  const disabledButton = screen.getByRole("button", {
    name: "Exportar prova",
  });
  expect(disabledButton).toBeDisabled();
  fireEvent.click(disabledButton);
  expect(harness.startPreview).not.toHaveBeenCalled();

  view.rerender(
    <ExportPreviewControl
      exportPort={harness.port}
      onActiveChange={onActiveChange}
      projectId="project-a"
    />,
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Exportar prova" }),
  );

  expect(onActiveChange.mock.calls).toEqual([[true]]);

  await act(async () => {
    harness.attempts[0].resolve({ status: "cancelled" });
    await Promise.resolve();
  });

  expect(onActiveChange.mock.calls).toEqual([[true], [false]]);
});

test("moves focus into progress and restores it after completion", async () => {
  const user = userEvent.setup();
  const harness = createExportHarness();

  render(
    <ExportPreviewControl
      exportPort={harness.port}
      projectId="project-a"
    />,
  );
  const exportButton = screen.getByRole("button", {
    name: "Exportar prova",
  });
  await user.click(exportButton);

  act(() => {
    harness.attempts[0].emit({ event: "started" });
  });

  await waitFor(() => {
    expect(
      screen.getByRole("dialog", { name: "Exportando" }),
    ).toHaveFocus();
  });

  await act(async () => {
    harness.attempts[0].resolve({
      status: "completed",
      result: {
        outputPath: "C:/exports/Album_001.jpg",
        widthPx: 7_087,
        heightPx: 3_543,
      },
    });
    await Promise.resolve();
  });

  await waitFor(() => {
    expect(exportButton).toHaveFocus();
  });
});
