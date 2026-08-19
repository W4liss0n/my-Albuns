import { act, fireEvent, renderHook } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import type { ProjectSessionPort } from "../application/projectPorts";
import { useEditorView } from "../state/editorView";
import {
  createTwoSheetProjection,
  representativeProjection,
} from "../test/projectFixtures";
import { useProjectEditorController } from "./useProjectEditorController";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

function projectSessionPort(): ProjectSessionPort {
  return {
    load: async () => representativeProjection,
    apply: async () => representativeProjection,
    undo: async () => representativeProjection,
    redo: async () => representativeProjection,
    save: async () => {
      throw new Error("Salvamento não configurado neste teste.");
    },
  };
}

beforeEach(() => {
  useEditorView.setState({
    projectId: representativeProjection.state.projectId,
    selectedFrameId: null,
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    viewport: { offsetX: 0 },
  });
});

test("routes editor changes through the shared Project mutation runner", async () => {
  const port = projectSessionPort();
  const apply = vi.spyOn(port, "apply");
  const runProjectMutation = vi.fn<ProjectMutationRunner>(
    async (operation) => ({
      status: "completed",
      projection: await operation(port),
    }),
  );
  const view = renderHook(() =>
    useProjectEditorController({
      projection: representativeProjection,
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

  expect(runProjectMutation).toHaveBeenCalledOnce();
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
      runProjectMutation: vi.fn(),
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

test("centers the edited Sheet when leaving Sheet Edit Mode", () => {
  const projection = createTwoSheetProjection();
  const view = renderHook(() =>
    useProjectEditorController({
      projection,
      runProjectMutation: vi.fn(),
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
    focusedSheetId: "sheet-002",
    viewport: { offsetX: 0 },
  });

  act(() => {
    view.result.current.canvasProps.onCanvasMetricsChange?.({
      width: 1_000,
      scale: 0.5,
    });
  });
  const expectedOffset =
    view.result.current.canvasProps.continuousCanvasLayout.centeredOffset(
      "sheet-002",
      0.5,
      1_000,
    );
  expect(useEditorView.getState()).toMatchObject({
    centeredSheetId: "sheet-002",
    focusedSheetId: "sheet-002",
    viewport: { offsetX: expectedOffset },
  });
});
