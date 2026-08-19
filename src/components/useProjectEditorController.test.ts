import { act, fireEvent, renderHook } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import type { ProjectSessionPort } from "../application/projectPorts";
import { useEditorView } from "../state/editorView";
import { representativeProjection } from "../test/projectFixtures";
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
