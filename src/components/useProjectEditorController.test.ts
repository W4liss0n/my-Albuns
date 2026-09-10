import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../test/layoutCatalogPorts";
import { act, fireEvent, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import type { ProjectCorePort } from "../application/projectPorts";
import { useEditorView } from "../state/editorView";
import {
  createTwoSheetProjection,
  representativeProjection,
} from "../test/projectFixtures";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner, type ProjectMutationRunner } from "./useProjectMutationRunner";
import type { ComposedFrame, EditorProjection, FrameGeometryEdit } from "../domain/project";

function projectCorePort(): ProjectCorePort {
  return {
    load: async () => representativeProjection,
    validateAlbumInformation: async () => ({
      errors: [],
      impact: { sheetWidthPx: 7_087, pageWidthPx: 3_543, heightPx: 3_543 },
    }),
    apply: async () => representativeProjection,
    applyWithOutcome: async () => ({
      projection: representativeProjection,
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
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewFrameGeometry: async () => { throw new Error("Frame geometry preview is not configured in this fixture."); },
    resolvePhotoDropTarget: async () => ({ kind: "invalid" }),
    relink: async () => representativeProjection,
    undo: async () => representativeProjection,
    redo: async () => representativeProjection,
    save: async () => {
      throw new Error("Salvamento não configurado neste teste.");
    },
    saveAs: async () => {
      throw new Error("Salvar como não configurado neste teste.");
    },
  };
}

function deferredValue<Value>() {
  let resolve!: (value: Value) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<Value>((resolver, rejecter) => {
    resolve = resolver;
    reject = rejecter;
  });
  return { promise, reject, resolve };
}

beforeEach(() => {
  useEditorView.setState({
    projectId: representativeProjection.state.projectId,
    selectedFrameIds: [],
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    editingSheetId: null,
    viewport: { offsetX: 0 },
  });
});

test.each(["success", "failure"])("a pending Frame edit followed by Save uses the shared queue (%s)", async (outcome) => {
  const port = projectCorePort();
  const pending = deferredValue<EditorProjection>();
  const changed: EditorProjection = { ...structuredClone(representativeProjection),
    state: { ...representativeProjection.state, revision: representativeProjection.state.revision + 1, dirty: true } };
  changed.composition.sheets[0].frames[0].clipRect.x += 30_000;
  const apply = vi.spyOn(port, "apply").mockReturnValue(pending.promise);
  const save = vi.spyOn(port, "save").mockImplementation(async (revision) => ({
    outcome: { kind: "saved", revision }, projection: changed,
  }));
  const onProjectionChange = vi.fn();
  const view = renderHook(() => {
    const runProjectMutation = useProjectMutationRunner(representativeProjection.state.projectId, port);
    return useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort, projection: representativeProjection, projectCorePort: port,
      runProjectMutation, onProjectionChange });
  });
  const edit: FrameGeometryEdit = {
    frames: [{ frameId: "frame-001", expectedRect: representativeProjection.state.album.sheets[0].frames[0].rect }],
    gesture: { kind: "move", deltaXUm: 30_000, deltaYUm: 20_000 },
  };
  let finished!: Promise<ComposedFrame[] | null>;
  act(() => {
    finished = view.result.current.canvasProps.frameGeometry!.commit(edit);
    view.result.current.save();
  });
  expect(apply).toHaveBeenCalledWith({ kind: "editFrameGeometry", edit }, expect.any(Function));
  expect(save).not.toHaveBeenCalled();
  await act(async () => {
    if (outcome === "success") pending.resolve(changed);
    else pending.reject(new Error("A geometria do Frame foi alterada durante o gesto."));
    await finished;
  });
  if (outcome === "success") {
    expect(await finished).toEqual([changed.composition.sheets[0].frames[0]]);
    expect(save).toHaveBeenCalledWith(changed.state.revision);
    expect(onProjectionChange).toHaveBeenCalledWith(changed);
  } else {
    expect(await finished).toBeNull();
    expect(save).not.toHaveBeenCalled();
    expect(onProjectionChange).not.toHaveBeenCalled();
    expect(view.result.current.message).toContain("geometria do Frame");
  }
});

test("loads the platform drag threshold in both modes and ignores replies from the previous mode", async () => {
  const port = projectCorePort();
  const reply = deferredValue<{ x: number; y: number }>();
  const currentReply = deferredValue<{ x: number; y: number }>();
  const read = vi.spyOn(port, "readFrameDragThreshold").mockReturnValueOnce(reply.promise)
    .mockReturnValueOnce(reply.promise).mockReturnValue(currentReply.promise);
  const view = renderHook(() => useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort,
    projection: representativeProjection, projectCorePort: port,
    runProjectMutation: { run: vi.fn(async () => ({ status: "obsolete" as const })), waitForIdle: async () => null }, onProjectionChange: vi.fn(),
  }));
  expect(read).toHaveBeenCalledOnce();
  act(() => view.result.current.canvasProps.onEditSheet("sheet-001"));
  expect(read).toHaveBeenCalledTimes(2);
  expect(view.result.current.canvasProps.frameGeometry!.dragThreshold).toBeNull();
  fireEvent.keyDown(window, { key: "Escape" });
  await act(async () => reply.resolve({ x: 7, y: 9 }));
  expect(view.result.current.canvasProps.frameGeometry!.dragThreshold).toBeNull();
  expect(view.result.current.canvasProps.frameContentSwap!.dragThreshold).toBeNull();
  await act(async () => currentReply.resolve({ x: 8, y: 10 }));
  expect(view.result.current.canvasProps.frameContentSwap!.dragThreshold).toEqual({ x: 8, y: 10 });
  act(() => view.result.current.canvasProps.onEditSheet("sheet-001"));
  await waitFor(() => expect(view.result.current.canvasProps.frameGeometry!.dragThreshold).toEqual({ x: 8, y: 10 }));
});

test("routes editor changes through the shared Project mutation runner", async () => {
  const port = projectCorePort();
  const apply = vi.spyOn(port, "apply");
  const run = vi.fn<ProjectMutationRunner["run"]>(async (operation) => ({
    status: "completed",
    projection: await operation(port, null),
  }));
  const runProjectMutation: ProjectMutationRunner = {
    run,
    waitForIdle: async () => null,
  };
  const view = renderHook(() =>
    useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort,
      projection: representativeProjection,
      projectCorePort: port,
      runProjectMutation,
      onProjectionChange: vi.fn(),
    }),
  );

  await act(async () => {
    await view.result.current.canvasProps.onTransformCommit({
      frameId: "frame-001",
      deltaPanX: 0.1,
      deltaPanY: 0,
      deltaZoom: 0,
    });
  });

  expect(run).toHaveBeenCalledOnce();
  expect(apply).toHaveBeenCalledWith({
    kind: "transformPhoto",
    frameId: "frame-001",
    deltaPanX: 0.1,
    deltaPanY: 0,
    deltaZoom: 0,
  }, expect.any(Function));
});

test("enters the centered Sheet Edit Mode with Enter and returns to normal mode with Escape", () => {
  const port = projectCorePort();
  const view = renderHook(() =>
    useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort,
      projection: representativeProjection,
      projectCorePort: port,
      runProjectMutation: {
        run: vi.fn(async () => ({ status: "obsolete" as const })),
        waitForIdle: async () => null,
      },
      onProjectionChange: vi.fn(),
    }),
  );
  const outsideButton = document.createElement("button");
  const canvasHost = document.createElement("div");
  const canvas = document.createElement("canvas");
  const input = document.createElement("input");
  canvasHost.className = "canvas-host";
  canvasHost.append(canvas);
  document.body.append(outsideButton, canvasHost, input);

  expect(view.result.current.canvasProps.mode).toEqual({ kind: "normal" });

  fireEvent.keyDown(outsideButton, { key: "Enter" });
  expect(view.result.current.canvasProps.mode).toEqual({ kind: "normal" });

  fireEvent.keyDown(canvas, { key: "Enter" });

  expect(view.result.current.canvasProps.mode).toEqual({
    kind: "sheet-editing",
    sheetId: "sheet-001",
  });

  fireEvent.keyDown(input, { key: "Escape" });

  expect(view.result.current.canvasProps.mode).toEqual({ kind: "normal" });
  outsideButton.remove();
  canvasHost.remove();
  input.remove();
});

test("targets the edited Sheet when leaving Sheet Edit Mode", () => {
  const projection = createTwoSheetProjection();
  const port = projectCorePort();
  const view = renderHook(() =>
    useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort,
      projection,
      projectCorePort: port,
      runProjectMutation: {
        run: vi.fn(async () => ({ status: "obsolete" as const })),
        waitForIdle: async () => null,
      },
      onProjectionChange: vi.fn(),
    }),
  );

  act(() => {
    view.result.current.canvasProps.onCanvasMetricsChange?.({
      width: 1_000,
      height: 500,
      scale: 0.5,
    });
    view.result.current.canvasProps.onEditSheet("sheet-002");
  });
  expect(view.result.current.canvasProps.mode).toEqual({
    kind: "sheet-editing",
    sheetId: "sheet-002",
  });
  act(() => {
    view.result.current.canvasProps.onCanvasMetricsChange?.({
      width: 1_000,
      height: 500,
      scale: 0.8,
    });
  });

  fireEvent.keyDown(window, { key: "Escape" });

  expect(view.result.current.canvasProps.mode).toEqual({ kind: "normal" });
  expect(useEditorView.getState()).toMatchObject({
    centeredSheetId: "sheet-002",
    focusedSheetId: "sheet-002",
    viewport: { offsetX: 0 },
  });
});

test("maps Sheet structure commands to explicit intents and falls back to the implicit Sheet", async () => {
  const projection = createTwoSheetProjection();
  const port = projectCorePort();
  const applyWithOutcome = vi
    .spyOn(port, "applyWithOutcome")
    .mockImplementation(async () => ({
      projection,
      affectedFrameId: null,
      affectedSheetId: null,
    }));
  const runProjectMutation: ProjectMutationRunner = {
    run: vi.fn(async (operation) => ({
      status: "completed" as const,
      projection: await operation(port, null),
    })),
    waitForIdle: async () => null,
  };
  const view = renderHook(() =>
    useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort,
      projection,
      projectCorePort: port,
      runProjectMutation,
      onProjectionChange: vi.fn(),
    }),
  );

  const outcomes: boolean[] = [];
  await act(async () => {
    outcomes.push(await view.result.current.addSheetBefore());
    outcomes.push(await view.result.current.addSheetAfter("sheet-002"));
    outcomes.push(await view.result.current.deleteSheet());
    outcomes.push(await view.result.current.reorderSheet("sheet-002", 0));
  });

  expect(outcomes).toEqual([true, true, true, true]);
  expect(applyWithOutcome.mock.calls.map(([intent]) => intent)).toEqual([
    {
      kind: "addSheet",
      anchorSheetId: "sheet-001",
      position: "before",
    },
    {
      kind: "addSheet",
      anchorSheetId: "sheet-002",
      position: "after",
    },
    { kind: "deleteSheet", sheetId: "sheet-001" },
    { kind: "reorderSheet", sheetId: "sheet-002", targetIndex: 0 },
  ]);
});

test.each(["completed", "failed"] as const)(
  "rejects an adjacent structural invocation before the pending state renders when the predecessor %s",
  async (predecessorOutcome) => {
    const projection = createTwoSheetProjection();
    const port = projectCorePort();
    const pending = deferredValue<
      Awaited<ReturnType<ProjectCorePort["applyWithOutcome"]>>
    >();
    const applyWithOutcome = vi
      .spyOn(port, "applyWithOutcome")
      .mockImplementation(() => pending.promise);
    const runProjectMutation: ProjectMutationRunner = {
      run: vi.fn(async (operation) => {
        try {
          return {
            status: "completed" as const,
            projection: await operation(port, null),
          };
        } catch (error: unknown) {
          return { status: "failed" as const, error };
        }
      }),
      waitForIdle: async () => null,
    };
    const view = renderHook(() =>
      useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort,
        projection,
        projectCorePort: port,
        runProjectMutation,
        onProjectionChange: vi.fn(),
      }),
    );

    let predecessor!: Promise<boolean>;
    let adjacent!: Promise<boolean>;
    act(() => {
      predecessor = view.result.current.deleteSheet("sheet-001");
      adjacent = view.result.current.addSheetAfter("sheet-001");
    });

    expect(await adjacent).toBe(false);
    expect(applyWithOutcome).toHaveBeenCalledOnce();
    expect(view.result.current.structuralMutationPending).toBe(true);

    let completed = false;
    await act(async () => {
      if (predecessorOutcome === "completed") {
        pending.resolve({
          projection,
          affectedFrameId: null,
          affectedSheetId: null,
        });
      } else {
        pending.reject(new Error("Falha estrutural controlada."));
      }
      completed = await predecessor;
    });

    expect(completed).toBe(predecessorOutcome === "completed");
    expect(view.result.current.structuralMutationPending).toBe(false);
    expect(applyWithOutcome).toHaveBeenCalledOnce();
  },
);

test("navigates to a newly affected Sheet after its projection becomes visible", async () => {
  const projection = createTwoSheetProjection();
  const port = projectCorePort();
  vi.spyOn(port, "applyWithOutcome").mockResolvedValue({
    projection,
    affectedFrameId: null,
    affectedSheetId: "sheet-002",
  });
  const runProjectMutation: ProjectMutationRunner = {
    run: vi.fn(async (operation) => ({
      status: "completed" as const,
      projection: await operation(port, null),
    })),
    waitForIdle: async () => null,
  };
  const onProjectionChange = vi.fn();
  const view = renderHook(
    ({ visibleProjection }) =>
      useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort,
        projection: visibleProjection,
        projectCorePort: port,
        runProjectMutation,
        onProjectionChange,
      }),
    { initialProps: { visibleProjection: representativeProjection } },
  );

  await act(async () => {
    expect(await view.result.current.addSheetAfter()).toBe(true);
  });
  expect(onProjectionChange).toHaveBeenCalledWith(projection);
  expect(useEditorView.getState().centeredSheetId).toBe("sheet-001");

  view.rerender({ visibleProjection: projection });
  await waitFor(() =>
    expect(useEditorView.getState()).toMatchObject({
      centeredSheetId: "sheet-002",
      focusedSheetId: "sheet-002",
    }),
  );
});

test("disables structural commands while a Sheet is being edited", async () => {
  const port = projectCorePort();
  const applyWithOutcome = vi.spyOn(port, "applyWithOutcome");
  const view = renderHook(() =>
    useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort,
      projection: representativeProjection,
      projectCorePort: port,
      runProjectMutation: {
        run: vi.fn(async () => ({ status: "obsolete" as const })),
        waitForIdle: async () => null,
      },
      onProjectionChange: vi.fn(),
    }),
  );

  expect(view.result.current.structuralCommandsDisabled).toBe(false);
  act(() => view.result.current.canvasProps.onEditSheet("sheet-001"));
  expect(view.result.current.structuralCommandsDisabled).toBe(true);

  await act(async () => {
    expect(await view.result.current.addSheetBefore()).toBe(false);
    expect(await view.result.current.addSheetAfter()).toBe(false);
    expect(await view.result.current.deleteSheet()).toBe(false);
    expect(await view.result.current.reorderSheet("sheet-001", 0)).toBe(false);
  });
  expect(applyWithOutcome).not.toHaveBeenCalled();
});
