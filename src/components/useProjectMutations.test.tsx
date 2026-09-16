import type { ProjectDialogAction, ProjectDialogPort, ProjectDialogState } from "../application/projectDialogPort";
import { createAlbumInformationReview } from "../application/albumInformationReview";
import { createAlbumInformationProjectDraft } from "../application/projectSettingsDraft";
import { emptyLayoutCatalogPort } from "../test/layoutCatalogPorts";
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { ProjectCorePort, ImageProcessingProgress } from "../application/projectPorts";
import { createAlbumDesignProjectDraft } from "../application/projectSettingsDraft";
import type { EditorProjection } from "../domain/project";
import {
  createThreeSheetProjection,
  representativeProjection,
} from "../test/projectFixtures";
import {
  useProjectMutationRunner,
  type ProjectMutationRunner,
} from "./useProjectMutationRunner";
import { useProjectMutations } from "./useProjectMutations";

const unusedDialogPort: ProjectDialogPort = { acquire: () => { throw new Error("Unexpected confirmation"); } };

function deferredProjection() {
  let resolve!: (projection: EditorProjection) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<EditorProjection>((resolver, rejecter) => {
    resolve = resolver;
    reject = rejecter;
  });
  return { promise, reject, resolve };
}

function projectSessionPort(
  apply: ProjectCorePort["apply"],
  undo: ProjectCorePort["undo"],
): ProjectCorePort {
  return {
    load: async () => representativeProjection,
    validateAlbumInformation: async () => ({
      errors: [],
      impact: { sheetWidthPx: 7_087, pageWidthPx: 3_543, heightPx: 3_543 },
    }),
    apply,
    applyWithOutcome: async (intent, publish) => ({
      projection: await apply(intent, publish),
      affectedFrameId: null,
      affectedSheetId: null,
    }),
    importMedia: async () => ({
      kind: "cancelled",
      projection: representativeProjection,
    }),
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    readSliderDoubleClickTime: async () => 500,
    queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewFrameStyle: async () => { throw new Error("Frame style preview is not configured in this fixture."); },
    previewDecorativeDrop: async () => { throw new Error("Decorative preview is not configured in this fixture."); },
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewFrameGeometry: async () => { throw new Error("Frame geometry preview is not configured in this fixture."); },
    resolvePhotoDropTarget: async () => ({ kind: "invalid" }),
    replaceImage: async () => representativeProjection, relink: async () => representativeProjection,
    undo,
    redo: async () => representativeProjection,
    save: async () => ({
      outcome: {
        kind: "alreadyCurrent",
        revision: representativeProjection.state.revision,
      },
      projection: representativeProjection,
    }),
    saveAs: async () => ({
      outcome: { kind: "cancelled" },
      projection: representativeProjection,
    }),
  };
}

test("applies a structural intent with outcome, returns its status, and forwards the affected Sheet", async () => {
  const updatedProjection: EditorProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      revision: representativeProjection.state.revision + 1,
    },
  };
  const port = projectSessionPort(
    async () => updatedProjection,
    async () => representativeProjection,
  );
  const applyWithOutcome = vi
    .spyOn(port, "applyWithOutcome")
    .mockResolvedValue({
      projection: updatedProjection,
      affectedFrameId: null,
      affectedSheetId: "sheet-001",
    });
  const runProjectMutation = {
    run: vi.fn<ProjectMutationRunner["run"]>(async (operation) => ({
      status: "completed",
      projection: await operation(port, null),
    })),
    waitForIdle: async () => null,
  };
  const onProjectionChange = vi.fn();
  const onAffectedSheet = vi.fn();
  const view = renderHook(() =>
    useProjectMutations({ projectDialogPort: unusedDialogPort,
      projection: representativeProjection,
      runProjectMutation,
      onProjectionChange,
      onAffectedFrame: () => undefined,
      onAffectedSheet,
    }),
  );
  const intent = {
    kind: "addSheet",
    anchorSheetId: "sheet-001",
    position: "after",
  } as Parameters<ProjectCorePort["applyWithOutcome"]>[0];

  let completed = false;
  await act(async () => {
    completed = await view.result.current.applyWithOutcome(intent);
  });

  expect(completed).toBe(true);
  expect(applyWithOutcome).toHaveBeenCalledWith(intent, expect.any(Function));
  expect(onProjectionChange).toHaveBeenCalledWith(updatedProjection);
  expect(onAffectedSheet).toHaveBeenCalledWith("sheet-001");
});

test.each(["success", "failure"] as const)("Save waits for replacement and respects its %s outcome", async (outcome) => {
  const pending = deferredProjection();
  const replaced = structuredClone(representativeProjection);
  replaced.state.revision += 1;
  replaced.state.dirty = true;
  const port = projectSessionPort(async () => representativeProjection, async () => representativeProjection);
  port.replaceImage = vi.fn(() => pending.promise);
  port.save = vi.fn(async (revision) => ({ outcome: { kind: "alreadyCurrent" as const, revision }, projection: replaced }));
  const onProjectionChange = vi.fn();
  const view = renderHook(() => useProjectMutations({ projectDialogPort: unusedDialogPort,
    projection: representativeProjection,
    runProjectMutation: useProjectMutationRunner(representativeProjection.state.projectId, port),
    onProjectionChange, onAffectedFrame: () => undefined, onAffectedSheet: () => undefined,
  }));
  act(() => view.result.current.replaceMedia("media-001"));
  await waitFor(() => expect(port.replaceImage).toHaveBeenCalledWith("media-001", expect.any(Function)));
  act(() => view.result.current.save());
  expect(port.save).not.toHaveBeenCalled();
  await act(async () => {
    if (outcome === "success") pending.resolve(replaced);
    else pending.reject(new Error("Imagem inválida"));
  });
  if (outcome === "success") {
    await waitFor(() => expect(port.save).toHaveBeenCalledWith(replaced.state.revision));
  } else {
    expect(port.save).not.toHaveBeenCalled();
    expect(onProjectionChange).not.toHaveBeenCalled();
  }
});

test("materializes a queued reorder beside its intended Sheet after History restores an earlier Sheet", async () => {
  const capturedProjection = createThreeSheetProjection();
  const afterUndo = structuredClone(capturedProjection);
  const restoredSheet = {
    ...structuredClone(afterUndo.state.album.sheets[1]),
    id: "sheet-restored",
    number: 2,
  };
  const restoredComposition = {
    ...structuredClone(afterUndo.composition.sheets[1]),
    sheetId: "sheet-restored",
    number: 2,
  };
  afterUndo.state.revision += 1;
  afterUndo.state.album.sheets.splice(1, 0, restoredSheet);
  afterUndo.composition.sheets.splice(1, 0, restoredComposition);

  const pendingUndo = deferredProjection();
  const undo = vi.fn<ProjectCorePort["undo"]>(() => pendingUndo.promise);
  const port = projectSessionPort(
    async () => afterUndo,
    undo,
  );
  const applyWithOutcome = vi
    .spyOn(port, "applyWithOutcome")
    .mockResolvedValue({
      projection: afterUndo,
      affectedFrameId: null,
      affectedSheetId: "sheet-003",
    });
  const view = renderHook(() => {
    const runner = useProjectMutationRunner(
      capturedProjection.state.projectId,
      port,
    );
    return useProjectMutations({ projectDialogPort: unusedDialogPort,
      projection: capturedProjection,
      runProjectMutation: runner,
      onProjectionChange: () => undefined,
      onAffectedFrame: () => undefined,
      onAffectedSheet: () => undefined,
    });
  });

  act(() => view.result.current.undo());
  await waitFor(() => expect(undo).toHaveBeenCalledOnce());
  let completed = false;
  await act(async () => {
    const completion = view.result.current.applyWithOutcome({
      kind: "reorderSheet",
      sheetId: "sheet-003",
      targetIndex: 1,
    });
    pendingUndo.resolve(afterUndo);
    completed = await completion;
  });

  expect(completed).toBe(true);
  expect(applyWithOutcome).toHaveBeenCalledWith({
    kind: "reorderSheet",
    sheetId: "sheet-003",
    targetIndex: 2,
  }, expect.any(Function));
});

test("keeps a queued reorder valid when the preceding History command fails", async () => {
  const capturedProjection = createThreeSheetProjection();
  const pendingUndo = deferredProjection();
  const undo = vi.fn<ProjectCorePort["undo"]>(() => pendingUndo.promise);
  const port = projectSessionPort(
    async () => capturedProjection,
    undo,
  );
  const applyWithOutcome = vi
    .spyOn(port, "applyWithOutcome")
    .mockResolvedValue({
      projection: capturedProjection,
      affectedFrameId: null,
      affectedSheetId: "sheet-003",
    });
  const view = renderHook(() => {
    const runner = useProjectMutationRunner(
      capturedProjection.state.projectId,
      port,
    );
    return useProjectMutations({ projectDialogPort: unusedDialogPort,
      projection: capturedProjection,
      runProjectMutation: runner,
      onProjectionChange: () => undefined,
      onAffectedFrame: () => undefined,
      onAffectedSheet: () => undefined,
    });
  });

  act(() => view.result.current.undo());
  await waitFor(() => expect(undo).toHaveBeenCalledOnce());
  let completed = false;
  await act(async () => {
    const completion = view.result.current.applyWithOutcome({
      kind: "reorderSheet",
      sheetId: "sheet-003",
      targetIndex: 1,
    });
    pendingUndo.reject(new Error("History failed"));
    completed = await completion;
  });

  expect(completed).toBe(true);
  expect(applyWithOutcome).toHaveBeenCalledWith({
    kind: "reorderSheet",
    sheetId: "sheet-003",
    targetIndex: 1,
  }, expect.any(Function));
});

test.each([true, false])("queues a scoped restoration after pending History without replacing adjacent album state (%s)", async (success) => {
  const pending = deferredProjection();
  const afterHistory = structuredClone(representativeProjection);
  afterHistory.state.revision += 1;
  afterHistory.state.album.visualDefaults.background = { scope: "bothSides", both: { kind: "color", rgb: "#AABBCC" } };
  const apply = vi.fn<ProjectCorePort["apply"]>(async () => afterHistory);
  const undo = vi.fn<ProjectCorePort["undo"]>(() => pending.promise);
  const port = projectSessionPort(apply, undo);
  const view = renderHook(() => {
    const runner = useProjectMutationRunner(representativeProjection.state.projectId, port);
    return useProjectMutations({ projectDialogPort: unusedDialogPort, projection: representativeProjection, runProjectMutation: runner,
      onProjectionChange: vi.fn(), onAffectedFrame: vi.fn(), onAffectedSheet: vi.fn() });
  });
  act(() => { void view.result.current.undo(); });
  await waitFor(() => expect(undo).toHaveBeenCalledOnce());
  const intent = { kind: "editSheetVisual" as const, sheetId: representativeProjection.state.album.sheets[0].id,
    scope: "left" as const, change: { kind: "restoreAlbum" as const, role: "background" as const } };
  await act(async () => {
    const completion = view.result.current.applyIntent(intent);
    expect(apply).not.toHaveBeenCalled();
    if (success) pending.resolve(afterHistory); else pending.reject(new Error("History unavailable"));
    expect(await completion).toBe(true);
  });
  expect(apply).toHaveBeenCalledExactlyOnceWith(intent, expect.any(Function));
});

test.each(["unchanged", "removed", "converted", "failure"] as const)(
  "revalidates duplication against preceding History (%s)", async (change) => {
    const initial = createThreeSheetProjection();
    const target = initial.state.album.sheets[1].id;
    const afterHistory = structuredClone(initial);
    afterHistory.state.revision += 1;
    if (change === "removed") {
      afterHistory.state.album.sheets.splice(1, 1);
      afterHistory.composition.sheets.splice(1, 1);
    } else if (change === "converted") {
      // A queued history result may restore this identity as a single-page edge.
      afterHistory.state.album.sheets.splice(2, 1);
      afterHistory.state.album.sheets[1].activeSides = "left";
      afterHistory.state.album.sheets[1].role = "final";
    }
    const pending = deferredProjection();
    const apply = vi.fn<ProjectCorePort["apply"]>(async () => afterHistory);
    const undo = vi.fn<ProjectCorePort["undo"]>(() => pending.promise);
    const port = projectSessionPort(apply, undo);
    const view = renderHook(() => {
      const runner = useProjectMutationRunner(initial.state.projectId, port);
      return useProjectMutations({ projectDialogPort: unusedDialogPort, projection: initial, runProjectMutation: runner,
        onProjectionChange: vi.fn(), onAffectedFrame: vi.fn(), onAffectedSheet: vi.fn() });
    });
    act(() => { view.result.current.undo(); });
    await waitFor(() => expect(undo).toHaveBeenCalledOnce());
    await act(async () => {
      const completion = view.result.current.applyWithOutcome({ kind: "duplicateSheet", sheetId: target });
      expect(apply).not.toHaveBeenCalled();
      if (change === "failure") pending.reject(new Error("History failed"));
      else pending.resolve(afterHistory);
      expect(await completion).toBe(change === "unchanged" || change === "failure");
    });
    if (change === "unchanged" || change === "failure") {
      expect(apply).toHaveBeenCalledExactlyOnceWith({ kind: "duplicateSheet", sheetId: target }, expect.any(Function));
    } else expect(apply).not.toHaveBeenCalled();
  },
);

test("preserves Redo when preceding History already materialized the Album Design target", async () => {
  const pendingUndo = deferredProjection();
  const target = {
    ...representativeProjection.state.album.visualDefaults,
    background: {
      scope: "bothSides" as const,
      both: { kind: "color" as const, rgb: "#F7F5F0" },
    },
  };
  const afterHistory = {
    ...target,
    overlay: {
      scope: "bothSides" as const,
      both: { kind: "media" as const, mediaId: "history-overlay" },
    },
  };
  const afterUndo: EditorProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      revision: representativeProjection.state.revision + 1,
      canRedo: true,
      album: {
        ...representativeProjection.state.album,
        visualDefaults: afterHistory,
      },
    },
  };
  const apply = vi.fn<ProjectCorePort["apply"]>(async () => afterUndo);
  const undo = vi.fn<ProjectCorePort["undo"]>(() => pendingUndo.promise);
  const port = projectSessionPort(apply, undo);
  const redo = vi.fn<ProjectCorePort["redo"]>(async () =>
    representativeProjection,
  );
  port.redo = redo;
  const onProjectionChange = vi.fn();
  const draft = createAlbumDesignProjectDraft(
    representativeProjection.state.revision,
    { ...representativeProjection.state.album.visualDefaults, frameGapUm: 5_000 },
  ).transition({ ...target, frameGapUm: 5_000 });
  const view = renderHook(() => {
    const runner = useProjectMutationRunner(
      representativeProjection.state.projectId,
      port,
    );
    return useProjectMutations({ projectDialogPort: unusedDialogPort,
      projection: representativeProjection,
      runProjectMutation: runner,
      onProjectionChange,
      onAffectedFrame: () => undefined,
      onAffectedSheet: () => undefined,
    });
  });

  act(() => view.result.current.undo());
  await waitFor(() => expect(undo).toHaveBeenCalledOnce());
  let completed = false;
  await act(async () => {
    const completion = view.result.current.applyAlbumDesign(draft);
    view.result.current.redo();
    pendingUndo.resolve(afterUndo);
    completed = await completion;
  });

  expect(completed).toBe(true);
  expect(apply).not.toHaveBeenCalled();
  await waitFor(() => expect(redo).toHaveBeenCalledOnce());
  expect(onProjectionChange).toHaveBeenCalledWith(afterUndo);
});


test.each(["completed", "cancelled", "failed"] as const)(
  "orders adjacent Save/Undo after a pending photo selection (%s)",
  async (terminal) => {
    type Result = Awaited<ReturnType<ProjectCorePort["importMedia"]>>;
    let resolve!: (value: Result) => void;
    let reject!: (error: Error) => void;
    const pending = new Promise<Result>((done, fail) => { resolve = done; reject = fail; });
    const initial = structuredClone(representativeProjection);
    initial.state.canUndo = false;
    const imported = structuredClone(initial);
    imported.state.revision += 1;
    imported.state.canUndo = true;
    imported.state.dirty = true;
    const port = projectSessionPort(async () => initial, vi.fn(async () => initial));
    port.importMedia = vi.fn(() => pending);
    port.save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({
      outcome: { kind: "saved", revision }, projection: revision === initial.state.revision ? initial : imported,
    }));
    const onProjectionChange = vi.fn();
    const view = renderHook(() => useProjectMutations({ projectDialogPort: unusedDialogPort,
      projection: initial,
      runProjectMutation: useProjectMutationRunner(initial.state.projectId, port),
      onProjectionChange,
      onAffectedFrame: () => undefined,
      onAffectedSheet: () => undefined,
    }));
    let completion!: Promise<string | null>;
    act(() => {
      completion = view.result.current.importMedia();
      void view.result.current.importMedia();
      view.result.current.save();
      view.result.current.undo();
    });
    expect(view.result.current.importPending).toBe(true);
    expect(port.importMedia).toHaveBeenCalledOnce();
    expect(port.save).not.toHaveBeenCalled();
    expect(port.undo).not.toHaveBeenCalled();
    await act(async () => {
      if (terminal === "failed") reject(new Error("Falha de leitura"));
      else if (terminal === "cancelled") resolve({ kind: "cancelled", projection: initial });
      else resolve({ kind: "completed", projection: imported, mediaIds: ["photo-a", "photo-b"],
        importedCount: 2, problems: [{ fileName: "quebrada.jpg", reason: "JPEG corrompido" }] });
      await completion;
    });
    await waitFor(() => expect(view.result.current.importPending).toBe(false));
    if (terminal === "completed") {
      expect(port.save).toHaveBeenCalledWith(imported.state.revision);
      await waitFor(() => expect(port.undo).toHaveBeenCalledOnce());
      expect(view.result.current.photoImportResult?.problems).toEqual([
        { fileName: "quebrada.jpg", reason: "JPEG corrompido" },
      ]);
    } else if (terminal === "failed") {
      expect(port.save).not.toHaveBeenCalled();
      expect(port.undo).not.toHaveBeenCalled();
      expect(view.result.current.message).toBe("Falha de leitura");
    } else {
      expect(port.save).toHaveBeenCalledWith(initial.state.revision);
      expect(view.result.current.photoImportResult).toBeNull();
    }
  },
);

test.each(["file", "operation"] as const)("waits for image cache before Save and keeps its %s warning through a queued edit", async (failureKind) => {
  let finish!: (result: Awaited<ReturnType<ProjectCorePort["importMedia"]>>) => void;
  let publish!: (progress: ImageProcessingProgress) => void;
  const imported = structuredClone(representativeProjection);
  imported.state.revision += 1;
  imported.state.dirty = true;
  const port = projectSessionPort(vi.fn(async () => imported), async () => imported);
  port.importMedia = vi.fn<ProjectCorePort["importMedia"]>((onProgress) => {
    publish = onProgress;
    publish({ completedFiles: 0, totalFiles: 1 });
    return new Promise((resolve) => { finish = resolve; });
  });
  port.save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({
    projection: imported, outcome: { kind: "saved", revision },
  }));
  const view = renderHook(() => useProjectMutations({ projectDialogPort: unusedDialogPort,
    projection: representativeProjection,
    runProjectMutation: useProjectMutationRunner(representativeProjection.state.projectId, port),
    onProjectionChange: () => undefined,
    onAffectedFrame: () => undefined,
    onAffectedSheet: () => undefined,
  }));
  let importing!: Promise<string | null>;
  let editing!: Promise<boolean>;
  act(() => {
    importing = view.result.current.importMedia();
    view.result.current.save();
    editing = view.result.current.applyIntent({ kind: "setDpi", dpi: 200 });
  });
  expect(view.result.current.imageProcessingProgress).toEqual({ completedFiles: 0, totalFiles: 1 });
  expect(port.save).not.toHaveBeenCalled();
  expect(port.apply).not.toHaveBeenCalled();
  const problem = { fileName: "Foto.jpg", reason: "A Foto foi vinculada, mas seu Cache não pôde ser preparado." };
  const operationProblem = "Não foi possível continuar o processamento por falta de memória.";
  const warning = failureKind === "file" ? { problem } : { operationProblem };
  await act(async () => {
    publish({ completedFiles: 1, totalFiles: 1, ...warning });
    finish({ kind: "completed", projection: imported, importedCount: 1, mediaIds: ["media-001"], problems: [] });
    await importing;
    await editing;
  });
  expect(port.save).toHaveBeenCalledWith(imported.state.revision);
  expect(view.result.current.photoImportResult?.importedCount).toBe(1);
  expect(view.result.current.imageProcessingProgress).toBeNull();
  expect(view.result.current.imageProcessingProblems).toEqual(failureKind === "file" ? [problem] : []);
  expect(view.result.current.imageProcessingOperationProblem).toBe(failureKind === "operation" ? operationProblem : null);
  act(() => view.result.current.dismissImageProcessingProblems());
  expect(view.result.current.imageProcessingProblems).toEqual([]);
  expect(view.result.current.imageProcessingOperationProblem).toBeNull();
});


test.each([false, true])("folder edits share the authoritative queue with Save and Undo (failure=%s)", async (fail) => {
  const pending = deferredProjection();
  const next = structuredClone(representativeProjection);
  next.state.revision += 1; next.state.dirty = true; next.state.canUndo = true;
  next.state.album.mediaFolders = [{ id: "folder-a", kind: "photo", name: "Turma", mediaIds: [] }];
  const apply = vi.fn<ProjectCorePort["apply"]>(() => pending.promise);
  const undo = vi.fn<ProjectCorePort["undo"]>(async () => representativeProjection);
  const port = projectSessionPort(apply, undo);
  port.save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({ outcome: { kind: "saved", revision }, projection: next }));
  const view = renderHook(() => useProjectMutations({ projectDialogPort: unusedDialogPort, projection: representativeProjection,
    runProjectMutation: useProjectMutationRunner(representativeProjection.state.projectId, port),
    onProjectionChange: vi.fn(), onAffectedFrame: vi.fn(), onAffectedSheet: vi.fn() }));
  let first!: Promise<boolean>; let adjacent!: Promise<boolean>;
  act(() => {
    first = view.result.current.editMediaFolder({ kind: "create", mediaKind: "photo", name: "Turma" });
    adjacent = view.result.current.editMediaFolder({ kind: "rename", folderId: "folder-a", name: "Formandos" });
    view.result.current.save(); view.result.current.undo();
  });
  expect(apply).toHaveBeenCalledTimes(1); expect(port.save).not.toHaveBeenCalled(); expect(undo).not.toHaveBeenCalled();
  await act(async () => {
    if (fail) pending.reject(new Error("Não foi possível criar a pasta.")); else pending.resolve(next);
    await Promise.all([first, adjacent]);
  });
  if (fail) {
    expect(apply).toHaveBeenCalledTimes(1);
    expect(view.result.current.message).toBe("Não foi possível criar a pasta.");
  } else {
    expect(apply).toHaveBeenNthCalledWith(2, { kind: "editMediaFolder", edit: { kind: "rename", folderId: "folder-a", name: "Formandos" } });
    await waitFor(() => expect(port.save).toHaveBeenCalledWith(next.state.revision));
    await waitFor(() => expect(undo).toHaveBeenCalledTimes(1));
  }
});

function conversionHarness(initial = decoratedEdgeProjection()) {
  let onAction: (action: ProjectDialogAction) => void = () => undefined;
  const present = vi.fn(async (_state: ProjectDialogState) => undefined);
  const dismiss = vi.fn(async () => undefined);
  const dialogPort: ProjectDialogPort = { acquire: (listener) => { onAction = listener; return { present, dismiss }; } };
  const apply = vi.fn<ProjectCorePort["apply"]>(async () => initial);
  const undo = vi.fn<ProjectCorePort["undo"]>(async () => initial);
  const port = projectSessionPort(apply, undo);
  const onAffectedSheet = vi.fn();
  const view = renderHook(({ projection }) => useProjectMutations({
    projection, projectDialogPort: dialogPort,
    runProjectMutation: useProjectMutationRunner(projection.state.projectId, port),
    onProjectionChange: vi.fn(), onAffectedFrame: vi.fn(), onAffectedSheet,
  }), { initialProps: { projection: initial } });
  return { ...view, apply, undo, present, dismiss, onAffectedSheet, emit: (action: ProjectDialogAction) => onAction(action) };
}

function decoratedEdgeProjection() {
  const projection = createThreeSheetProjection();
  projection.state.album.sheets[0].visuals = {
    background: { kind: "perSide", left: { kind: "custom", content: { kind: "color", rgb: "#123456" }, mapping: "side" }, right: { kind: "default" } },
    overlay: { kind: "default" },
  };
  return projection;
}

const convertFirstEdge = { kind: "convertEdgeSheet", sheetId: "sheet-001" } as const;

test.each(["confirmEdgeConversion", "cancelEdgeConversion"] as const)(
  "keeps the mutation queue free while awaiting conversion decision %s", async (action) => {
    const harness = conversionHarness();
    let completed!: Promise<boolean>;
    act(() => {
      completed = harness.result.current.applyWithOutcome(convertFirstEdge);
      harness.result.current.undo();
    });
    await waitFor(() => expect(harness.present).toHaveBeenCalledWith({ kind: "edgeConversionConfirmation",
      message: "O Background personalizado da página esquerda da Lâmina 1 será removido." }));
    expect(harness.apply).not.toHaveBeenCalled();
    await waitFor(() => expect(harness.undo).toHaveBeenCalledOnce());
    await act(async () => {
      harness.emit(action);
      harness.emit(action);
      expect(await completed).toBe(action === "confirmEdgeConversion");
    });
    expect(harness.apply).toHaveBeenCalledTimes(action === "confirmEdgeConversion" ? 1 : 0);
    expect(harness.dismiss).toHaveBeenCalledOnce();
    await waitFor(() => expect(harness.undo).toHaveBeenCalledOnce());
    expect(harness.onAffectedSheet).not.toHaveBeenCalled();
  },
);

test.each(["added", "removed", "failed"])("uses the authoritative visual after pending Undo (%s)", async (change) => {
  const initial = change === "added" ? createThreeSheetProjection() : decoratedEdgeProjection();
  const latest = change === "added" ? decoratedEdgeProjection() : createThreeSheetProjection();
  const harness = conversionHarness(initial);
  const pending = deferredProjection();
  harness.undo.mockReturnValueOnce(pending.promise);
  let completed!: Promise<boolean>;
  act(() => { harness.result.current.undo(); completed = harness.result.current.applyWithOutcome(convertFirstEdge); });
  expect(harness.present).not.toHaveBeenCalled();
  await act(async () => {
    if (change === "failed") pending.reject(new Error("Undo failed")); else pending.resolve(latest);
    await pending.promise.catch(() => undefined);
  });
  if (change !== "removed") {
    await waitFor(() => expect(harness.present).toHaveBeenCalledOnce());
    expect(harness.apply).not.toHaveBeenCalled();
    await act(async () => { harness.emit("cancelEdgeConversion"); expect(await completed).toBe(false); });
  } else {
    await act(async () => { expect(await completed).toBe(true); });
    expect(harness.present).not.toHaveBeenCalled();
    expect(harness.apply).toHaveBeenCalledOnce();
  }
});

test.each(["unmount", "projectChange"])("releases a pending conversion on %s and ignores late confirmation", async (reason) => {
  const harness = conversionHarness();
  let completed!: Promise<boolean>;
  act(() => { completed = harness.result.current.applyWithOutcome(convertFirstEdge); });
  await waitFor(() => expect(harness.present).toHaveBeenCalledOnce());
  if (reason === "unmount") harness.unmount(); else {
    const next = decoratedEdgeProjection(); next.state.projectId = "different-project";
    harness.rerender({ projection: next });
  }
  await act(async () => { harness.emit("confirmEdgeConversion"); expect(await completed).toBe(false); });
  expect(harness.apply).not.toHaveBeenCalled();
  expect(harness.dismiss).toHaveBeenCalledOnce();
});

test("a failed confirmation presentation does not convert and releases the queue", async () => {
  const harness = conversionHarness();
  harness.present.mockRejectedValueOnce(new Error("Dialog unavailable"));
  await act(async () => { expect(await harness.result.current.applyWithOutcome(convertFirstEdge)).toBe(false); });
  expect(harness.result.current.message).toBe("Dialog unavailable");
  expect(harness.apply).not.toHaveBeenCalled();
  await act(async () => { harness.result.current.undo(); });
  expect(harness.undo).toHaveBeenCalledOnce();
});

test("Album information re-reviews newly discarded content after queued Undo, then applies once", async () => {
  const initial = createThreeSheetProjection();
  const latest = decoratedEdgeProjection();
  const harness = conversionHarness(initial);
  const pending = deferredProjection();
  harness.undo.mockReturnValueOnce(pending.promise);
  const baseline = { ...initial.state.document, firstSheet: "double" as const, lastSheet: "double" as const };
  const information = { ...baseline, firstSheet: "singlePage" as const };
  const draft = createAlbumInformationProjectDraft(initial.state.revision, baseline).transition(information);
  const impact = { sheetWidthPx: 7_087, pageWidthPx: 3_543, heightPx: 3_543 };
  const review = createAlbumInformationReview(baseline, information, impact, initial.state.album.sheets);
  let commit!: ReturnType<typeof harness.result.current.applyAlbumInformation>;
  act(() => { harness.result.current.undo(); commit = harness.result.current.applyAlbumInformation(draft, review); });
  await act(async () => { pending.resolve(latest); await commit; });
  const outcome = await commit;
  expect(outcome.kind).toBe("reviewRequired");
  expect(harness.apply).not.toHaveBeenCalled();
  expect(harness.present).not.toHaveBeenCalled();
  if (outcome.kind !== "reviewRequired") throw new Error("Expected review");
  expect(outcome.review.conversionLosses).toHaveLength(1);
  await act(async () => {
    expect(await harness.result.current.applyAlbumInformation(draft, outcome.review)).toEqual({ kind: "completed" });
  });
  expect(harness.apply).toHaveBeenCalledOnce();
  expect(harness.present).not.toHaveBeenCalled();
});

test("rechecks changed discarded applications after confirmation without blocking History", async () => {
  const harness = conversionHarness();
  const latest = decoratedEdgeProjection();
  latest.state.album.sheets[0].visuals = { background: { kind: "default" },
    overlay: { kind: "perSide", left: { kind: "custom", content: { kind: "media", mediaId: "new-overlay" }, mapping: "side" }, right: { kind: "default" } } };
  harness.undo.mockResolvedValueOnce(latest);
  let completed!: Promise<boolean>;
  act(() => { completed = harness.result.current.applyWithOutcome(convertFirstEdge); });
  await waitFor(() => expect(harness.present).toHaveBeenCalledOnce());
  await act(async () => { harness.result.current.undo(); });
  await act(async () => { harness.emit("confirmEdgeConversion"); });
  await waitFor(() => expect(harness.present).toHaveBeenCalledTimes(2));
  expect(harness.present).toHaveBeenLastCalledWith({ kind: "edgeConversionConfirmation",
    message: "O Overlay personalizado da página esquerda da Lâmina 1 será removido." });
  expect(harness.apply).not.toHaveBeenCalled();
  await act(async () => { harness.emit("confirmEdgeConversion"); expect(await completed).toBe(true); });
  expect(harness.apply).toHaveBeenCalledOnce();
});

test("does not reverse a conversion that another queued action already satisfied", async () => {
  const harness = conversionHarness();
  const alreadySingle = decoratedEdgeProjection();
  alreadySingle.state.album.sheets[0].activeSides = "right";
  harness.undo.mockResolvedValueOnce(alreadySingle);
  let completed!: Promise<boolean>;
  act(() => { completed = harness.result.current.applyWithOutcome(convertFirstEdge); });
  await waitFor(() => expect(harness.present).toHaveBeenCalledOnce());
  await act(async () => { harness.result.current.undo(); });
  await act(async () => { harness.emit("confirmEdgeConversion"); expect(await completed).toBe(false); });
  expect(harness.apply).not.toHaveBeenCalled();
});
