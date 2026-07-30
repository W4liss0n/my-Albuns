import { act, renderHook, waitFor } from "@testing-library/react";
import { startTransition, useState } from "react";
import { beforeEach, expect, test, vi } from "vitest";

import type {
  ExportPort,
  ExportResult,
  ProjectSessionPort,
} from "../application/projectPorts";
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
  };
}

function projectMutationRunner(
  port: ProjectSessionPort,
): ProjectMutationRunner {
  return async (operation) => {
    try {
      return {
        status: "completed",
        projection: await operation(port),
      };
    } catch (error: unknown) {
      return { status: "failed", error };
    }
  };
}

function deferredExport() {
  let resolve!: (value: ExportResult) => void;
  const promise = new Promise<ExportResult>((resolver) => {
    resolve = resolver;
  });
  return { promise, resolve };
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

test("keeps an Export result when a concurrent context render is abandoned", async () => {
  const pendingExport = deferredExport();
  const committedExportPort: ExportPort = {
    exportPreview: vi.fn(() => pendingExport.promise),
  };
  const uncommittedExportPort: ExportPort = {
    exportPreview: vi.fn(async () => ({
      outputPath: "C:\\Temp\\uncommitted.png",
      widthPx: 1,
      heightPx: 1,
    })),
  };
  const sessionPort = projectSessionPort();
  const runProjectMutation = projectMutationRunner(sessionPort);
  const suspended = new Promise<never>(() => undefined);
  let controller!: ReturnType<typeof useProjectEditorController>;
  let beginSuspendedRender!: () => void;
  let suspendedRenderAttempted = false;

  const view = renderHook(() => {
    const [useUncommittedContext, setUseUncommittedContext] =
      useState(false);
    const currentController = useProjectEditorController({
      projection: representativeProjection,
      exportPort: useUncommittedContext
        ? uncommittedExportPort
        : committedExportPort,
      runProjectMutation,
      onProjectionChange: () => undefined,
    });
    beginSuspendedRender = () => {
      startTransition(() => setUseUncommittedContext(true));
    };
    if (useUncommittedContext) {
      suspendedRenderAttempted = true;
      throw suspended;
    }
    controller = currentController;
    return currentController;
  });

  act(() => controller.exportPreview());
  expect(committedExportPort.exportPreview).toHaveBeenCalledOnce();

  beginSuspendedRender();
  await waitFor(() => expect(suspendedRenderAttempted).toBe(true));

  const result: ExportResult = {
    outputPath: "C:\\Temp\\Album-Horizonte_001.png",
    widthPx: 600,
    heightPx: 300,
  };
  await act(async () => {
    pendingExport.resolve(result);
    await pendingExport.promise;
  });

  expect(view.result.current.exportResult).toEqual(result);
  expect(uncommittedExportPort.exportPreview).not.toHaveBeenCalled();
});

test("clears feedback and ignores an Export result after a committed Project change", async () => {
  const pendingExport = deferredExport();
  const firstExportPort: ExportPort = {
    exportPreview: vi.fn(() => pendingExport.promise),
  };
  const secondExportPort: ExportPort = {
    exportPreview: vi.fn(async () => ({
      outputPath: "C:\\Temp\\second-project.png",
      widthPx: 300,
      heightPx: 300,
    })),
  };
  const sessionPort = projectSessionPort();
  const runProjectMutation = projectMutationRunner(sessionPort);
  const secondProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      projectId: "project-002",
    },
  };
  const view = renderHook(
    ({
      exportPort,
      projection,
    }: {
      exportPort: ExportPort;
      projection: typeof representativeProjection;
    }) =>
      useProjectEditorController({
        projection,
        exportPort,
        runProjectMutation,
        onProjectionChange: () => undefined,
      }),
    {
      initialProps: {
        exportPort: firstExportPort,
        projection: representativeProjection,
      },
    },
  );

  act(() => view.result.current.exportPreview());
  expect(view.result.current.busy).toBe("Exportando");

  view.rerender({
    exportPort: secondExportPort,
    projection: secondProjection,
  });
  expect(view.result.current.busy).toBeNull();

  await act(async () => {
    pendingExport.resolve({
      outputPath: "C:\\Temp\\first-project.png",
      widthPx: 600,
      heightPx: 300,
    });
    await pendingExport.promise;
  });

  expect(view.result.current.busy).toBeNull();
  expect(view.result.current.exportResult).toBeNull();
  expect(secondExportPort.exportPreview).not.toHaveBeenCalled();
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
      exportPort: {
        exportPreview: vi.fn(),
      },
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
