import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import { frameStyleCorpus as corpus } from "../test/frameStylePreview";
import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../test/layoutCatalogPorts";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => useEditorView.setState(useEditorView.getInitialState(), true));

test.each([
  { outcome: "success", reselect: false },
  { outcome: "failure", reselect: false },
  { outcome: "success", reselect: true },
  { outcome: "failure", reselect: true },
])("individual Zoom retains pending ownership across another property: $outcome, reselect=$reselect", async ({ outcome, reselect }) => {
  const initial = structuredClone(corpus.states.album);
  const selected = initial.state.album.sheets.flatMap(sheet => sheet.frames)
    .find(frame => frame.id === corpus.single[0])!;
  const baseline = selected.photo!.transform.userZoom;
  const zoomed = structuredClone(initial);
  zoomed.state.album.sheets.flatMap(sheet => sheet.frames)
    .find(frame => frame.id === selected.id)!.photo!.transform.userZoom = baseline + 0.5;
  zoomed.state.revision += 1;
  let resolve!: (value: EditorProjection) => void;
  let reject!: (error: Error) => void;
  const pending = new Promise<EditorProjection>((complete, fail) => { resolve = complete; reject = fail; });
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this property queue test."); };
  const apply = vi.fn<ProjectCorePort["apply"]>().mockImplementationOnce(() => pending).mockResolvedValue(zoomed);
  const port: ProjectCorePort = {
    load: async () => initial, apply, applyWithOutcome: unsupported,
    save: unsupported, undo: unsupported, redo: unsupported,
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    readSliderDoubleClickTime: async () => 500,
    queryLayouts: unsupported, previewLayout: unsupported, previewDecorativeDrop: unsupported,
    previewPhotoZoom: unsupported, previewFrameStyle: unsupported,
    previewPhotoAngle: unsupported, previewFrameGeometry: unsupported, saveAs: unsupported,
    validateAlbumInformation: unsupported, importMedia: unsupported,
    resolvePhotoDropTarget: unsupported, replaceImage: unsupported, relink: unsupported,
  };
  const sheetId = initial.state.album.sheets[0].id;
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: sheetId,
    focusedSheetId: sheetId, centeredSheetId: sheetId, selectedFrameIds: corpus.single });
  const view = renderHook(() => {
    const [projection, setProjection] = useState(initial);
    const runner = useProjectMutationRunner(initial.state.projectId, port);
    return { runner, ...useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort,
      projection, projectCorePort: port, runProjectMutation: runner, onProjectionChange: setProjection }) };
  });
  act(() => {    view.result.current.photoZoom.onPreview((baseline + 0.5) * 100);
    view.result.current.photoZoom.onCommit((baseline + 0.5) * 100);
  });
  expect(view.result.current.photoZoom.disabled).toBe(true);
  act(() => view.result.current.frameStyle.onCommit({ kind: "opacity", opacityPercent: 50 }));
  try {
    // InspectorPanel uses these actions to disable the individual control. Re-enabling
    // it here lets a second gesture capture the old Zoom as its delta baseline.
    expect(view.result.current.photoZoom.disabled).toBe(true);
    expect(view.result.current.displayedPhotoZoom).toBe(baseline + 0.5);
    expect(view.result.current.canvasProps.photoZoomPreview).toEqual({ frameId: selected.id, value: baseline + 0.5 });
    expect(apply).toHaveBeenCalledExactlyOnceWith({ kind: "transformPhoto", frameId: selected.id,
      deltaPanX: 0, deltaPanY: 0, deltaZoom: 0.5 }, expect.any(Function));
    if (reselect) {
      act(() => useEditorView.getState().selectFrames(corpus.placeholders));
      expect(view.result.current.photoZoom.disabled).toBe(true);
      expect(view.result.current.canvasProps.photoZoomPreview).toBeNull();
    }
  } finally {
    await act(async () => {
      if (outcome === "success") resolve(zoomed);
      else reject(new Error("Falha ao confirmar Zoom."));
      await view.result.current.runner.waitForIdle();
    });
    expect(view.result.current.photoZoom.disabled).toBe(reselect);
    expect(view.result.current.canvasProps.photoZoomPreview).toBeNull();
    expect(apply).toHaveBeenCalledTimes(outcome === "success" ? 2 : 1);
    if (reselect) expect(useEditorView.getState().selectedFrameIds).toEqual(corpus.placeholders);
    view.unmount();
  }
});
