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
    relink: async () => representativeProjection,
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
    useProjectMutations({
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
    return useProjectMutations({
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
    return useProjectMutations({
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
    representativeProjection.state.album.visualDefaults,
  ).transition(target);
  const view = renderHook(() => {
    const runner = useProjectMutationRunner(
      representativeProjection.state.projectId,
      port,
    );
    return useProjectMutations({
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
    const view = renderHook(() => useProjectMutations({
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

test("waits for image cache before Save and keeps its warning through a queued edit", async () => {
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
  const view = renderHook(() => useProjectMutations({
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
  await act(async () => {
    publish({ completedFiles: 1, totalFiles: 1, problem });
    finish({ kind: "completed", projection: imported, importedCount: 1, mediaIds: ["media-001"], problems: [] });
    await importing;
    await editing;
  });
  expect(port.save).toHaveBeenCalledWith(imported.state.revision);
  expect(view.result.current.photoImportResult?.importedCount).toBe(1);
  expect(view.result.current.imageProcessingProgress).toBeNull();
  expect(view.result.current.imageProcessingProblems).toEqual([problem]);
  act(() => view.result.current.dismissImageProcessingProblems());
  expect(view.result.current.imageProcessingProblems).toEqual([]);
});
