import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../../test/layoutCatalogPorts";
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type { ProjectCorePort } from "../../application/projectPorts";
import type { EditorProjection } from "../../domain/project";
import { useEditorView } from "../../state/editorView";
import { representativeProjection } from "../../test/projectFixtures";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => useEditorView.setState(useEditorView.getInitialState(), true));

const LEFT = { x: -1, y: 0 } as const;
const RIGHT = { x: 1, y: 0 } as const;
const DOWN = { x: 0, y: 1 } as const;

function movedProjection(source: EditorProjection, deltaXUm: number, deltaYUm: number) {
  const moved = structuredClone(source);
  moved.state.revision += 1;
  const frame = moved.state.album.sheets[0].frames[0];
  frame.rect = { ...frame.rect, x: frame.rect.x + deltaXUm, y: frame.rect.y + deltaYUm };
  const composed = moved.composition.sheets[0].frames[0];
  composed.clipRect = { ...composed.clipRect, x: composed.clipRect.x + deltaXUm, y: composed.clipRect.y + deltaYUm };
  return moved;
}

function renderController(initial: EditorProjection, apply: ProjectCorePort["apply"], editingSheetId: string | null = "sheet-001") {
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this move test."); };
  const port: ProjectCorePort = {
    load: async () => initial, apply, save: unsupported, undo: unsupported,
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    readSliderDoubleClickTime: async () => 500,
    validateMediaFolderName: unsupported, queryLayouts: unsupported, previewLayout: unsupported,
    previewFrameStyle: unsupported, previewDecorativeDrop: unsupported, previewPhotoZoom: unsupported,
    previewPhotoAngle: unsupported, previewFrameGeometry: unsupported, redo: unsupported, saveAs: unsupported,
    validateAlbumInformation: unsupported, applyWithOutcome: unsupported,
    importMedia: unsupported, resolvePhotoDropTarget: unsupported, replaceImage: unsupported, relink: unsupported,
  };
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId,
    focusedSheetId: "sheet-001", centeredSheetId: "sheet-001", selectedFrameIds: ["frame-001"] });
  return renderHook(() => {
    const [projection, setProjection] = useState(initial);
    const runProjectMutation = useProjectMutationRunner(initial.state.projectId, port);
    return useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort, projection, projectCorePort: port,
      runProjectMutation, onProjectionChange: setProjection });
  });
}

test("an arrow moves the selected Frames 1 mm, and 10 mm with Shift, from the confirmed geometry", async () => {
  const initial = structuredClone(representativeProjection);
  const apply = vi.fn<ProjectCorePort["apply"]>(async (intent) => {
    if (intent.kind !== "editFrameGeometry" || intent.edit.gesture.kind !== "move") throw new Error(intent.kind);
    return movedProjection(initial, intent.edit.gesture.deltaXUm, intent.edit.gesture.deltaYUm);
  });
  const view = renderController(initial, apply);
  expect(view.result.current.canMoveFrames).toBe(true);
  await act(async () => { view.result.current.moveFrames(LEFT, false); });
  await act(async () => { view.result.current.moveFrames(DOWN, true); });
  expect(apply.mock.calls.map(([intent]) => intent)).toEqual([
    { kind: "editFrameGeometry", edit: {
      frames: [{ frameId: "frame-001", expectedRect: { x: 20_000, y: 20_000, width: 280_000, height: 260_000 } }],
      gesture: { kind: "move", deltaXUm: -1_000, deltaYUm: 0 },
    } },
    { kind: "editFrameGeometry", edit: {
      frames: [{ frameId: "frame-001", expectedRect: { x: 19_000, y: 20_000, width: 280_000, height: 260_000 } }],
      gesture: { kind: "move", deltaXUm: 0, deltaYUm: 10_000 },
    } },
  ]);
});

test("keys pressed while a move runs add up into one following edit", async () => {
  const initial = structuredClone(representativeProjection);
  let resolveFirst!: (projection: EditorProjection) => void;
  const apply = vi.fn<ProjectCorePort["apply"]>((intent) => {
    if (intent.kind !== "editFrameGeometry" || intent.edit.gesture.kind !== "move") throw new Error(intent.kind);
    if (apply.mock.calls.length === 1) return new Promise((resolve) => { resolveFirst = resolve; });
    const before = intent.edit.frames[0].expectedRect;
    return Promise.resolve(movedProjection(initial, before.x - 20_000 + intent.edit.gesture.deltaXUm, 0));
  });
  const view = renderController(initial, apply);
  act(() => { view.result.current.moveFrames(RIGHT, false); });
  act(() => {
    view.result.current.moveFrames(RIGHT, false, true);
    view.result.current.moveFrames(RIGHT, false, true);
    view.result.current.moveFrames(RIGHT, true, true);
  });
  expect(apply).toHaveBeenCalledOnce();
  await act(async () => { resolveFirst(movedProjection(initial, 1_000, 0)); });
  expect(apply).toHaveBeenCalledTimes(2);
  const second = apply.mock.calls[1][0];
  expect(second).toEqual({ kind: "editFrameGeometry", edit: {
    frames: [{ frameId: "frame-001", expectedRect: { x: 21_000, y: 20_000, width: 280_000, height: 260_000 } }],
    gesture: { kind: "move", deltaXUm: 12_000, deltaYUm: 0 },
  } });
});

test("a locked layout reports why the Frames stay, once per press, and normal mode ignores the arrows", async () => {
  const locked = structuredClone(representativeProjection);
  locked.state.album.sheets[0].layoutLocked = true;
  const apply = vi.fn<ProjectCorePort["apply"]>();
  const view = renderController(locked, apply);
  act(() => { view.result.current.moveFrames(LEFT, false); });
  expect(view.result.current.message).toBe("O layout está travado. Destrave-o no painel de layouts para mover os quadros.");
  act(() => { view.result.current.dismissFeedback(); });
  act(() => { view.result.current.moveFrames(LEFT, false, true); });
  expect(view.result.current.message).toBeNull();
  view.unmount();

  const normal = renderController(structuredClone(representativeProjection), apply, null);
  expect(normal.result.current.canMoveFrames).toBe(false);
  act(() => { normal.result.current.moveFrames(LEFT, false); });
  expect(apply).not.toHaveBeenCalled();
});
