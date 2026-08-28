import { act, fireEvent, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import type { ProjectCorePort } from "../application/projectPorts";
import { useEditorView } from "../state/editorView";
import {
  createTwoSheetProjection,
  representativeProjection,
} from "../test/projectFixtures";
import { useProjectEditorController } from "./useProjectEditorController";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

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
    importPhoto: async () => ({
      kind: "cancelled",
      projection: representativeProjection,
    }),
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

beforeEach(() => {
  useEditorView.setState({
    projectId: representativeProjection.state.projectId,
    selectedFrameId: null,
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    editingSheetId: null,
    viewport: { offsetX: 0 },
  });
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
    useProjectEditorController({
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
  });
});

test("enters the centered Sheet Edit Mode with Enter and returns to normal mode with Escape", () => {
  const view = renderHook(() =>
    useProjectEditorController({
      projection: representativeProjection,
      projectCorePort: projectCorePort(),
      runProjectMutation: {
        run: vi.fn(),
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
  const view = renderHook(() =>
    useProjectEditorController({
      projection,
      projectCorePort: projectCorePort(),
      runProjectMutation: {
        run: vi.fn(),
        waitForIdle: async () => null,
      },
      onProjectionChange: vi.fn(),
    }),
  );

  act(() => {
    view.result.current.canvasProps.onCanvasMetricsChange?.({
      width: 1_000,
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
    useProjectEditorController({
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
      useProjectEditorController({
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
    useProjectEditorController({
      projection: representativeProjection,
      projectCorePort: port,
      runProjectMutation: {
        run: vi.fn(),
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
