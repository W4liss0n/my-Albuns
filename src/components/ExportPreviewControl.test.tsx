import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
  ProjectDialogSession,
} from "../application/projectDialogPort";
import type { ExportPipelinePort } from "../application/projectPorts";
import { ExportPreviewControl } from "./ExportPreviewControl";
import { MediaExportBlockedError, type ExportMediaPort } from "../application/exportMedia";
import { representativeProjection } from "../test/projectFixtures";
import { ExportConflictsError } from "../application/normalExport";
import { StorageFullError } from "../application/storageRecovery";
import { createExportHarness } from "../test/exportPipelineHarness";

test("normal export keeps its dialog and resumes only after cache cleanup", async () => {
  let finish!: (freed: boolean) => void;
  const harness = createExportHarness();
  (harness.port as ExportPipelinePort).storageRecovery = { status: async () => ({ id: "full", canClearCache: true }),
    clear: vi.fn(() => new Promise<boolean>(resolve => { finish = resolve; })) };
  const { dialog } = renderControl({ exportHarness: harness });
  await startConfigured(dialog);
  await act(async () => harness.attempts[0].reject(new StorageFullError("Libere espaço.")));
  await waitFor(() => expect(dialog.present).toHaveBeenLastCalledWith({ kind: "storageFull", message: "Libere espaço.", canClearCache: true, busy: false }));
  dialog.emit("clearStorageCache");
  expect(harness.startSheet).toHaveBeenCalledTimes(1);
  expect(dialog.dismiss).not.toHaveBeenCalled();
  await act(async () => finish(true));
  expect(harness.startSheet).toHaveBeenCalledTimes(2);
  expect(harness.startSheet).toHaveBeenLastCalledWith(expect.objectContaining({ recoveryId: "full" }), expect.any(Function));
  expect(dialog.dismiss).not.toHaveBeenCalled();
});

test.each([
  ["sheet", "png", ["opening", "middle", "closing"], 3, "lâmina", 31.666666666666668],
  ["page", "png", ["opening", "middle", "closing"], 4, "página", 26.25],
  ["page", "pdf", ["opening", "closing"], 2, "página", 42.5],
] as const)("counts %s units in %s exports, including single-page ends", async (mode, format, sheetIds, total, unit, composingPercent) => {
  const harness = createExportHarness();
  const dialog = createDialogHarness();
  render(<ExportPreviewControl dialogPort={dialog.port} exportPipelinePort={harness.port} projectId="project-a"
    selection={{ projectName: "Album", sheetId: "opening", sheetNumber: 1 }}
    sheets={[{ sheetId: "opening", number: 1, pageCount: 1 }, { sheetId: "middle", number: 2, pageCount: 2 },
      { sheetId: "closing", number: 3, pageCount: 1 }]} />);
  fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
  await waitFor(() => expect(dialog.present).toHaveBeenCalled());
  dialog.emit({ configureExport: { scope: "range", sheetIds: [...sheetIds], mode, format: { kind: format },
    destination: "C:/Exportados", conflictPolicy: "skip" } });
  const expectCount = (count: number) => expect(dialog.present).toHaveBeenLastCalledWith(expect.objectContaining({
    kind: "exportProgress", progress: expect.objectContaining({ status: `${count} ${unit}${count === 1 ? "" : "s"} de ${total}` }),
  }));
  await act(async () => harness.attempts[0].emit({ event: "started", cancellable: true }));
  expectCount(0);
  await act(async () => harness.attempts[0].emit({ event: "progress", stage: "loading_sources", overallPercent: 5,
    units: { kind: "measured", completedUnits: 7, totalUnits: 14 }, cancellable: true }));
  expectCount(0);
  await act(async () => harness.attempts[0].emit({ event: "progress", stage: "composing", overallPercent: composingPercent,
    units: { kind: "measured", completedUnits: 1, totalUnits: total }, cancellable: true }));
  expectCount(1);
  await act(async () => harness.attempts[0].emit({ event: "progress", stage: "encoding_output", overallPercent: 75,
    units: { kind: "measured", completedUnits: total, totalUnits: total }, cancellable: true }));
  expectCount(total);
  await act(async () => harness.attempts[0].emit({ event: "progress", stage: "publishing", overallPercent: 85,
    units: { kind: "measured", completedUnits: 0, totalUnits: format === "pdf" ? 1 : total }, cancellable: false }));
  expectCount(total);
});

test("uses the effective export total after conflicts and retains counts across a storage pause", async () => {
  const harness = createExportHarness();
  (harness.port as ExportPipelinePort).storageRecovery = {
    status: async () => ({ id: "paused", canClearCache: false }), clear: async () => false,
  };
  const dialog = createDialogHarness();
  render(<ExportPreviewControl dialogPort={dialog.port} exportPipelinePort={harness.port} projectId="project-a"
    selection={{ projectName: "Album", sheetId: "first", sheetNumber: 1 }}
    sheets={[{ sheetId: "first", number: 1, pageCount: 2 }, { sheetId: "last", number: 2, pageCount: 2 }]} />);
  fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
  await waitFor(() => expect(dialog.present).toHaveBeenCalled());
  dialog.emit({ configureExport: { scope: "album", sheetIds: ["first", "last"], mode: "page",
    format: { kind: "png" }, destination: "C:/Exportados", conflictPolicy: "skip" } });
  await act(async () => {
    harness.attempts[0].emit({ event: "started", cancellable: true });
    harness.attempts[0].emit({ event: "progress", stage: "preparing", overallPercent: 0,
      units: { kind: "measured", completedUnits: 0, totalUnits: 3 }, cancellable: true });
  });
  expect(dialog.present).toHaveBeenLastCalledWith(expect.objectContaining({ progress: expect.objectContaining({ status: "0 páginas de 3" }) }));
  await act(async () => harness.attempts[0].emit({ event: "progress", stage: "composing", overallPercent: 53.33333333333333,
    units: { kind: "measured", completedUnits: 2, totalUnits: 3 }, cancellable: true }));
  await act(async () => harness.attempts[0].reject(new StorageFullError("Libere espaço.")));
  await waitFor(() => expect(dialog.present).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "storageFull" })));
  dialog.emit("resumeStorage");
  await act(async () => {
    harness.attempts[1].emit({ event: "started", cancellable: true });
    harness.attempts[1].emit({ event: "progress", stage: "preparing", overallPercent: 0,
      units: { kind: "measured", completedUnits: 2, totalUnits: 3 }, cancellable: true });
    harness.attempts[1].emit({ event: "progress", stage: "loading_sources", overallPercent: 0,
      units: { kind: "measured", completedUnits: 0, totalUnits: 1 }, cancellable: true });
  });
  expect(dialog.present).toHaveBeenLastCalledWith(expect.objectContaining({ progress: expect.objectContaining({ status: "2 páginas de 3", completed: 53 }) }));
});

test("retrying after cancelling a resumed export does not reuse its consumed native token", async () => {
  const harness = createExportHarness();
  (harness.port as ExportPipelinePort).storageRecovery = {
    status: async () => ({ id: "full", canClearCache: false }), clear: async () => false,
  };
  const { dialog } = renderControl({ exportHarness: harness });
  await startConfigured(dialog);
  await act(async () => harness.attempts[0].reject(new StorageFullError("Libere espaço.")));
  await waitFor(() => expect(dialog.present).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "storageFull", busy: false })));
  dialog.emit("resumeStorage");
  expect(harness.startSheet.mock.calls[1][0].recoveryId).toBe("full");
  await act(async () => {
    harness.attempts[1].emit({ event: "started", cancellable: true });
    harness.attempts[1].resolve({ status: "cancelled" });
  });
  dialog.emit("retryExport");
  expect(harness.startSheet.mock.calls[2][0].recoveryId).toBeUndefined();
});

test("cancelling the disk-full dialog waits for native preparation disposal before closing", async () => {
  const harness = createExportHarness();
  let finish!: () => void;
  const discard = vi.fn(() => new Promise<void>(resolve => { finish = resolve; }));
  const port = harness.port as ExportPipelinePort;
  port.discardRecovery = discard;
  port.storageRecovery = { status: async () => ({ id: "paused", canClearCache: false }), clear: async () => false };
  const { dialog } = renderControl({ exportHarness: harness });
  await startConfigured(dialog);
  await act(async () => harness.attempts[0].reject(new StorageFullError("Libere espaço.")));
  await waitFor(() => expect(dialog.present).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "storageFull", busy: false })));
  dialog.emit("cancelStorage");
  expect(discard).toHaveBeenCalledWith("paused");
  expect(dialog.dismiss).not.toHaveBeenCalled();
  await act(async () => finish());
  expect(dialog.dismiss).toHaveBeenCalledOnce();
});

test("opens normal export only after the destination is available, without a transient preparation state", async () => {
  const dialog = createDialogHarness();
  const harness = createExportHarness();
  let resolveDestination!: (destination: string) => void;
  harness.port.defaultDestination = () => new Promise(resolve => { resolveDestination = resolve; });
  render(<ExportPreviewControl dialogPort={dialog.port} exportPipelinePort={harness.port} projectId="project-a"
    selection={{ projectName: "Album", sheetId: "first", sheetNumber: 1 }}
    sheets={[{ sheetId: "first", number: 1, pageCount: 2 }]} />);

  fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
  expect(screen.getByRole("button", { name: "Exportar" })).toBeDisabled();
  expect(dialog.present).not.toHaveBeenCalled();

  await act(async () => { resolveDestination("C:/Exportados/Album"); });
  expect(dialog.present).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({
    kind: "exportConfiguration", busy: false, message: "",
    options: expect.objectContaining({ destination: "C:/Exportados/Album" }),
  }));
  expect(harness.startSheet).not.toHaveBeenCalled();
});

test("opens an editable configuration with the destination error instead of a temporary preparation state", async () => {
  const dialog = createDialogHarness();
  const harness = createExportHarness();
  harness.port.defaultDestination = async () => { throw new Error("Destino indisponível"); };
  render(<ExportPreviewControl dialogPort={dialog.port} exportPipelinePort={harness.port} projectId="project-a"
    selection={{ projectName: "Album", sheetId: "first", sheetNumber: 1 }}
    sheets={[{ sheetId: "first", number: 1, pageCount: 2 }]} />);

  fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
  await waitFor(() => expect(dialog.present).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({
    kind: "exportConfiguration", busy: false, message: "Destino indisponível",
    options: expect.objectContaining({ destination: "" }),
  })));
});

test("does not open configuration if the control unmounts while resolving its destination", async () => {
  const dialog = createDialogHarness();
  const harness = createExportHarness();
  const onActiveChange = vi.fn();
  let resolveDestination!: (destination: string) => void;
  harness.port.defaultDestination = () => new Promise(resolve => { resolveDestination = resolve; });
  const view = render(<ExportPreviewControl dialogPort={dialog.port} exportPipelinePort={harness.port} projectId="project-a"
    onActiveChange={onActiveChange} selection={{ projectName: "Album", sheetId: "first", sheetNumber: 1 }}
    sheets={[{ sheetId: "first", number: 1, pageCount: 2 }]} />);

  fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
  view.unmount();
  await act(async () => { resolveDestination("C:/Exportados/Album"); });
  expect(dialog.present).not.toHaveBeenCalled();
  expect(onActiveChange.mock.calls).toEqual([[true], [false]]);
});

test.each([
  ["confirmExportOverwrite", "replace"],
  ["skipExportConflicts", "skip"],
  ["dismissExport", null],
] as const)("normal export retries the same selection after %s", async (action, conflictPolicy) => {
  const dialog = createDialogHarness(); const harness = createExportHarness();
  render(<ExportPreviewControl dialogPort={dialog.port} exportPipelinePort={harness.port} projectId="project-a"
    selection={{ projectName: "Album", sheetId: "second", sheetNumber: 2 }}
    sheets={[{ sheetId: "first", number: 1, pageCount: 1 }, { sheetId: "second", number: 2, pageCount: 2 }]} />);
  fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
  await waitFor(() => expect(dialog.present).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "exportConfiguration", busy: false })));
  expect(harness.startSheet).not.toHaveBeenCalled();
  const options = { scope: "range" as const, sheetIds: ["second"], mode: "page" as const, format: { kind: "png" as const }, destination: "C:/Exportados", conflictPolicy: "ask" as const };
  dialog.emit({ configureExport: options });
  expect(dialog.present).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "exportConfiguration", busy: true, options }));
  expect(harness.startSheet).toHaveBeenLastCalledWith({ projectName: "Album", sheetId: "second", sheetNumber: 2, options }, expect.any(Function));
  await act(async () => { harness.attempts[0].reject(new ExportConflictsError(["Album_002.png", "Album_003.png"])); });
  expect(dialog.present).toHaveBeenLastCalledWith({ kind: "exportConflicts", files: ["Album_002.png", "Album_003.png"] });
  dialog.emit(action);
  if (conflictPolicy === null) {
    expect(dialog.dismiss).toHaveBeenCalledTimes(1);
    expect(harness.startSheet).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Exportar" })).toBeEnabled();
    return;
  }
  expect(harness.startSheet).toHaveBeenLastCalledWith(expect.objectContaining({ options: { ...options, conflictPolicy } }), expect.any(Function));
  dialog.emit(action); expect(harness.startSheet).toHaveBeenCalledTimes(2);
  if (conflictPolicy === "skip") {
    await act(async () => { harness.attempts[1].resolve({ status: "skipped" }); });
    expect(dialog.dismiss).toHaveBeenCalledTimes(1);
    expect(dialog.present.mock.calls.some(([state]) => state.kind === "exportProgress" || state.kind === "exportSuccess")).toBe(false);
    expect(screen.getByRole("button", { name: "Exportar" })).toBeEnabled();
  }
});

function createDialogHarness() {
  let listener: ((action: ProjectDialogAction) => void) | undefined;
  const dismiss = vi.fn<ProjectDialogSession["dismiss"]>(async () => undefined);
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

async function startConfigured(dialog: ReturnType<typeof createDialogHarness>) {
  fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
  await act(async () => { await Promise.resolve(); });
  const state = dialog.present.mock.calls[dialog.present.mock.calls.length - 1]?.[0];
  if (state?.kind !== "exportConfiguration") throw new Error("Export configuration was not opened");
  dialog.emit({ configureExport: state.options });
}

function renderControl({
  dialog = createDialogHarness(),
  exportHarness = createExportHarness(),
  onActiveChange,
  exportMediaPort,
  onProjectionChange,
  projectId = "project-a",
}: {
  dialog?: ReturnType<typeof createDialogHarness>;
  exportHarness?: ReturnType<typeof createExportHarness>;
  onActiveChange?: (active: boolean) => void;
  exportMediaPort?: ExportMediaPort;
  onProjectionChange?: (projection: typeof representativeProjection) => void;
  projectId?: string;
} = {}) {
  const view = render(
    <ExportPreviewControl
      sheets={[{ sheetId: "sheet-001", number: 1, pageCount: 2 }]}
      dialogPort={dialog.port}
      exportPipelinePort={exportHarness.port}
      exportMediaPort={exportMediaPort}
      onProjectionChange={onProjectionChange}
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

test("folder recovery publishes the unsaved projection and resumes once after closing Problems", async () => {
  const problems = [{ mediaId: "photo-1", fileName: "Foto.jpg", state: "absent" as const }];
  let resolve!: (result: Awaited<ReturnType<ExportMediaPort["relink"]>>) => void;
  const relink = vi.fn<ExportMediaPort["relink"]>(() => new Promise(done => { resolve = done; }));
  const inspect = vi.fn<ExportMediaPort["inspect"]>(async () => []);
  const onProjectionChange = vi.fn();
  const { dialog, exportHarness } = renderControl({ exportMediaPort: { relink, inspect }, onProjectionChange });
  await startConfigured(dialog);
  await act(async () => exportHarness.attempts[0].reject(new MediaExportBlockedError(problems)));
  dialog.emit("relinkExportMedia");
  dialog.emit("relinkExportMedia");
  dialog.emit("dismissExport");
  expect(relink).toHaveBeenCalledOnce();
  expect(dialog.dismiss).not.toHaveBeenCalled();
  let finishDismiss!: () => void;
  dialog.dismiss.mockImplementationOnce(() => new Promise<void>(done => { finishDismiss = done; }));
  await act(async () => resolve({ projection: representativeProjection, problems: [], notes: [] }));
  expect(onProjectionChange).toHaveBeenCalledWith(representativeProjection);
  expect(dialog.present).not.toHaveBeenCalledWith(expect.objectContaining({ kind: "exportMediaProblems", problems: [] }));
  expect(dialog.dismiss).toHaveBeenCalledOnce();
  expect(exportHarness.startSheet).toHaveBeenCalledOnce();
  await act(async () => finishDismiss());
  expect(exportHarness.startSheet).toHaveBeenCalledTimes(2);
  expect(inspect).not.toHaveBeenCalled();
  await act(async () => exportHarness.attempts[1].resolve({ status: "cancelled" }));
  expect(dialog.dismiss).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "Exportar" })).toBeEnabled();
  await startConfigured(dialog);
  expect(exportHarness.startSheet).toHaveBeenCalledTimes(3);
});

test("unavailable sources resume only after reinspection clears the last problem", async () => {
  const relink = vi.fn();
  const problems = [{ mediaId: "photo-1", fileName: "Rede.png", state: "unavailable" as const }];
  const inspect = vi.fn<ExportMediaPort["inspect"]>(async () => problems);
  const { dialog, exportHarness } = renderControl({ exportMediaPort: { relink, inspect } });
  await startConfigured(dialog);
  await act(async () => exportHarness.attempts[0].reject(new MediaExportBlockedError(problems)));
  dialog.emit("retryExportMedia");
  await waitFor(() => expect(dialog.present).toHaveBeenLastCalledWith(expect.objectContaining({ problems, busy: false })));
  expect(relink).not.toHaveBeenCalled();
  expect(exportHarness.startSheet).toHaveBeenCalledOnce();
  expect(dialog.dismiss).not.toHaveBeenCalled();
  inspect.mockResolvedValueOnce([]);
  dialog.emit("retryExportMedia");
  await waitFor(() => expect(exportHarness.startSheet).toHaveBeenCalledTimes(2));
  expect(dialog.dismiss).toHaveBeenCalledOnce();
  expect(dialog.present).not.toHaveBeenCalledWith(expect.objectContaining({ kind: "exportMediaProblems", problems: [] }));
  await act(async () => exportHarness.attempts[1].reject(new MediaExportBlockedError(problems)));
  expect(dialog.present).toHaveBeenLastCalledWith(expect.objectContaining({ problems, busy: false }));
  expect(exportHarness.startSheet).toHaveBeenCalledTimes(2);
});

test("partial folder recovery keeps remaining problems and closing cancels the pending export", async () => {
  const problems = [{ mediaId: "photo-1", fileName: "Foto.jpg", state: "absent" as const }];
  const relink = vi.fn<ExportMediaPort["relink"]>(async () => ({ projection: representativeProjection, problems,
    notes: [{ fileName: "Foto.jpg", reason: "Mais de uma correspondência encontrada." }] }));
  const { dialog, exportHarness } = renderControl({ exportMediaPort: { relink, inspect: vi.fn() } });
  await startConfigured(dialog);
  await act(async () => exportHarness.attempts[0].reject(new MediaExportBlockedError(problems)));
  dialog.emit("relinkExportMedia");
  await waitFor(() => expect(dialog.present).toHaveBeenLastCalledWith(expect.objectContaining({ problems, busy: false,
    message: "Foto.jpg: Mais de uma correspondência encontrada." })));
  expect(exportHarness.startSheet).toHaveBeenCalledOnce();
  dialog.emit("dismissExport");
  expect(dialog.dismiss).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "Exportar" })).toBeEnabled();
});

test("retiring the Project during recovery prevents automatic resumption", async () => {
  let resolve!: (result: Awaited<ReturnType<ExportMediaPort["relink"]>>) => void;
  const relink = vi.fn<ExportMediaPort["relink"]>(() => new Promise(done => { resolve = done; }));
  const onProjectionChange = vi.fn();
  const { dialog, exportHarness, view } = renderControl({ exportMediaPort: { relink, inspect: vi.fn() }, onProjectionChange });
  await startConfigured(dialog);
  await act(async () => exportHarness.attempts[0].reject(new MediaExportBlockedError([
    { mediaId: "photo-1", fileName: "Foto.jpg", state: "absent" },
  ])));
  dialog.emit("relinkExportMedia");
  view.unmount();
  await act(async () => resolve({ projection: representativeProjection, problems: [], notes: [] }));
  expect(onProjectionChange).not.toHaveBeenCalled();
  expect(exportHarness.startSheet).toHaveBeenCalledOnce();
});

test("a recovered Original with a failed preview shows the processing problem instead of silently resuming", async () => {
  const notes = [{ fileName: "Foto.jpg", reason: "Não foi possível publicar a prévia do Cache." }];
  const relink = vi.fn<ExportMediaPort["relink"]>(async () => ({ projection: representativeProjection, problems: [], notes }));
  const onProjectionChange = vi.fn();
  const { dialog, exportHarness } = renderControl({ exportMediaPort: { relink, inspect: vi.fn() }, onProjectionChange });
  await startConfigured(dialog);
  await act(async () => exportHarness.attempts[0].reject(new MediaExportBlockedError([
    { mediaId: "photo-1", fileName: "Foto.jpg", state: "absent" },
  ])));
  dialog.emit("relinkExportMedia");
  await waitFor(() => expect(dialog.present).toHaveBeenLastCalledWith({
    kind: "imageProcessingProblems", importedCount: null, operationProblem: null, problems: notes,
  }));
  expect(onProjectionChange).toHaveBeenCalledWith(representativeProjection);
  expect(exportHarness.startSheet).toHaveBeenCalledOnce();
  dialog.emit("dismissImageProcessingProblems");
  expect(dialog.dismiss).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "Exportar" })).toBeEnabled();
});

test("placeholder validation presents Project problems and returns to the Project without retrying", async () => {
  const { dialog, exportHarness } = renderControl();
  await startConfigured(dialog);
  const { LayoutExportBlockedError } = await import("../application/projectPorts");
  const problems = [{ sheetId: "sheet-001", sheetNumber: 1, frameId: "frame-002", frameNumber: 2 }];
  await act(async () => exportHarness.attempts[0].reject(new LayoutExportBlockedError(problems)));
  expect(dialog.present).toHaveBeenLastCalledWith({ kind: "exportProblems", projectName: "Projeto de teste", problems });
  dialog.emit("openExportProject");
  await waitFor(() => expect(dialog.dismiss).toHaveBeenCalledOnce());
  expect(exportHarness.startSheet).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "Exportar" })).toBeEnabled();
});

test("waits for the backend started event before opening the native progress window", async () => {
  const { dialog, exportHarness } = renderControl();
  await startConfigured(dialog);

  expect(exportHarness.startSheet).toHaveBeenCalledWith(
    {
      projectName: "Projeto de teste",
      sheetId: "sheet-001",
      sheetNumber: 1,
      options: expect.objectContaining({ sheetIds: ["sheet-001"] }),
    },
    expect.any(Function),
  );
  expect(dialog.present).not.toHaveBeenCalledWith(expect.objectContaining({ kind: "exportProgress" }));

  act(() => {
    exportHarness.attempts[0].emit({ event: "started", cancellable: false });
  });

  expect(dialog.present).toHaveBeenCalledWith({
    cancelRequested: false,
    cancellable: false,
    kind: "exportProgress",
    progress: {
      kind: "determinate", completed: 0, total: 100,
      status: "0 lâminas de 1",
    },
  });
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

test("displays the native overall percentage independently of stage units", async () => {
  const { dialog, exportHarness } = renderControl();
  await startConfigured(dialog);
  act(() => {
    exportHarness.attempts[0].emit({ event: "started", cancellable: true });
    exportHarness.attempts[0].emit({
      event: "progress",
      stage: "preparing",
      overallPercent: 0,
      units: { kind: "unmeasured" },
      cancellable: true,
    });
    exportHarness.attempts[0].emit({
      event: "progress",
      stage: "composing",
      overallPercent: 37.75,
      units: { kind: "measured", completedUnits: 2, totalUnits: 5 },
      cancellable: true,
    });
  });

  expect(dialog.present).toHaveBeenNthCalledWith(4, {
    cancelRequested: false,
    cancellable: true,
    kind: "exportProgress",
    progress: {
      kind: "determinate", completed: 0, total: 100,
      status: "0 lâminas de 1",
    },
  });
  expect(dialog.present).toHaveBeenNthCalledWith(5, {
    cancelRequested: false,
    cancellable: true,
    kind: "exportProgress",
    progress: {
      completed: 37,
      kind: "determinate",
      status: "2 lâminas de 5",
      total: 100,
    },
  });
});

test("handles cancellation actions from the child window and keeps feedback there", async () => {
  const { dialog, exportHarness } = renderControl();
  await startConfigured(dialog);
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
    progress: expect.objectContaining({ kind: "determinate", status: "0 lâminas de 1" }),
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
  const onActiveChange = vi.fn();
  const { dialog, exportHarness } = renderControl({ onActiveChange });
  await startConfigured(dialog);

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
  const onActiveChange = vi.fn();
  const dialog = createDialogHarness();
  const { exportHarness } = renderControl({ dialog, onActiveChange });

  await startConfigured(dialog);
  dialog.present.mockClear();
  dialog.present.mockRejectedValue(new Error("Não foi possível abrir a janela de progresso."));
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
  expect(screen.getByRole("button", { name: "Exportar" })).toBeEnabled();
  expect(onActiveChange.mock.calls).toEqual([[true], [false]]);
});

test("replaces native progress with the standard success dialog", async () => {
  const { dialog, exportHarness } = renderControl();
  await startConfigured(dialog);
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
    message: "A Exportação foi concluída com sucesso.",
  });
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
});

test("retries and dismisses terminal feedback from semantic child-window actions", async () => {
  const { dialog, exportHarness } = renderControl();
  await startConfigured(dialog);
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
  expect(screen.getByRole("button", { name: "Exportar" })).toBeEnabled();
});

test("retires the attempt and native presentation when the Project changes", async () => {
  const onActiveChange = vi.fn();
  const dialog = createDialogHarness();
  const exportHarness = createExportHarness();
  const { view } = renderControl({ dialog, exportHarness, onActiveChange });
  await startConfigured(dialog);
  act(() => {
    exportHarness.attempts[0].emit({ event: "started", cancellable: true });
  });

  view.rerender(
    <ExportPreviewControl
      sheets={[{ sheetId: "sheet-001", number: 1, pageCount: 2 }]}
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
