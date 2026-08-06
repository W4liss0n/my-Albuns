import { act, renderHook } from "@testing-library/react";
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
