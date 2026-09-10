import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../test/layoutCatalogPorts";
import { useState } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection, LayoutSelection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import { layoutPanelCorpus } from "../test/layoutPanelPreview";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => useEditorView.setState(useEditorView.getInitialState(), true));

function harness(pendingKind: "applyLayout" | "lockLayout" | "unlockLayout" | "setDpi", locked = false) {
  const sample = layoutPanelCorpus.cases.mixed;
  const initial = structuredClone(sample.before.projection);
  initial.state.album.sheets[0].layoutLocked = locked;
  const applied = structuredClone(sample.applied!.projection);
  applied.state.album.sheets[0].layoutLocked = pendingKind === "lockLayout";
  const sheetId = initial.state.album.sheets[0].id;
  let authoritative = initial;
  let querySequence = 0;
  let prepared: { queryId: string; revision: number } | null = null;
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const pending = new Promise<void>((yes, no) => { resolve = yes; reject = no; });
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this Layout test."); };
  const checkSelection = (selection: LayoutSelection) => {
    if (selection.queryId !== prepared?.queryId || prepared.revision !== authoritative.state.revision) {
      throw new Error("Esta prévia de Layout expirou.");
    }
  };
  const apply = vi.fn<ProjectCorePort["apply"]>(async (intent) => {
    if (intent.kind === pendingKind) await pending;
    if (intent.kind === "applyLayout" || intent.kind === "lockLayout") {
      checkSelection(intent.selection);
      authoritative = applied;
    } else if (intent.kind === "unlockLayout") {
      expect(intent.sheetId).toBe(sheetId);
      authoritative = applied;
    } else if (intent.kind === "setDpi") {
      authoritative = { ...initial, state: { ...initial.state, revision: initial.state.revision + 1,
        document: { ...initial.state.document, dpi: intent.dpi } } };
    } else throw new Error("Unexpected command");
    return authoritative;
  });
  const save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({ outcome: { kind: "saved", revision }, projection: authoritative }));
  const undo = vi.fn(async () => { authoritative = initial; return initial; });
  const port: ProjectCorePort = {
    load: async () => initial, apply, save, undo, redo: async () => applied,
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }), readSliderDoubleClickTime: async () => 500,
    queryLayouts: async (target) => {
      const query = { ...structuredClone(sample.before.queries[target].query),
        queryId: `prepared-${++querySequence}`, revision: authoritative.state.revision,
        locked: authoritative.state.album.sheets[0].layoutLocked };
      prepared = query;
      return query;
    },
    previewLayout: async (selection) => {
      checkSelection(selection);
      return structuredClone(sample.before.queries[sheetId].previews[selection.candidateIndex]);
    },
    previewFrameStyle: unsupported, previewPhotoAngle: unsupported, previewFrameGeometry: unsupported,
    saveAs: unsupported, validateAlbumInformation: unsupported, applyWithOutcome: unsupported,
    importPhoto: unsupported, resolvePhotoDropTarget: unsupported, relink: unsupported,
  };
  const view = renderHook(() => {
    const [projection, setProjection] = useState<EditorProjection>(initial);
    const runner = useProjectMutationRunner(initial.state.projectId, port);
    return useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort, projection, projectCorePort: port, runProjectMutation: runner,
      onProjectionChange: setProjection });
  });
  return { view, initial, applied, sheetId, apply, save, undo, resolve, reject };
}

test("locked Sheet metadata reaches the Canvas and disables structural commands without disabling content or order", async () => {
  const { view, sheetId, initial } = harness("setDpi", true);
  act(() => {
    useEditorView.getState().enterSheetEdit(sheetId);
    useEditorView.getState().selectFrames(initial.state.album.sheets[0].frames.map((frame) => frame.id));
  });
  await waitFor(() => expect(view.result.current.selectedFrames.length).toBeGreaterThan(0));
  expect(view.result.current.canvasProps.sheetBarMetadata[0].layoutLocked).toBe(true);
  expect(view.result.current.canAddFrame).toBe(false);
  expect(view.result.current.canPasteFrames).toBe(false);
  expect(view.result.current.canArrangeFrames).toBe(true);
  expect(view.result.current.canDeleteFrames).toBe(true);
  expect(view.result.current.canOrientPhotos).toBe(true);
});

test.each((["applyLayout", "lockLayout", "unlockLayout"] as const).flatMap((kind) => [false, true].map((fails) => ({ kind, fails }))))("$kind, Save and Undo keep their queue order, failure=$fails", async ({ kind, fails }) => {
  const { view, sheetId, apply, save, undo, resolve, reject, applied } = harness(kind, kind === "unlockLayout");
  act(() => view.result.current.canvasProps.sheetLayouts!.onToggle(sheetId));
  await waitFor(() => expect(view.result.current.layoutPanel.query).not.toBeNull());
  const queryId = view.result.current.layoutPanel.query!.queryId;
  let completion!: Promise<unknown>;
  act(() => {
    const panel = view.result.current.layoutPanel;
    const command = kind === "applyLayout" ? panel.apply(0) : kind === "lockLayout" ? panel.lock(0) : panel.unlock();
    completion = Promise.all([command, view.result.current.save(), view.result.current.undo()]);
  });
  expect(save).not.toHaveBeenCalled();
  expect(undo).not.toHaveBeenCalled();
  await act(async () => { if (fails) reject(new Error("Aplicação recusada.")); else resolve(); await completion; });
  expect(apply.mock.calls[0][0]).toEqual(kind === "unlockLayout" ? { kind, sheetId } : { kind, selection: { queryId, candidateIndex: 0 } });
  if (fails) {
    expect(save).not.toHaveBeenCalled();
    expect(undo).not.toHaveBeenCalled();
    expect(view.result.current.message).toBe("Aplicação recusada.");
  } else {
    expect(save).toHaveBeenCalledExactlyOnceWith(applied.state.revision);
    expect(undo).toHaveBeenCalledOnce();
  }
});

test.each([false, true])("a pending predecessor never silently retargets the prepared Layout, predecessor failure=%s", async (fails) => {
  const { view, sheetId, apply, resolve, reject } = harness("setDpi");
  act(() => view.result.current.canvasProps.sheetLayouts!.onToggle(sheetId));
  await waitFor(() => expect(view.result.current.layoutPanel.query).not.toBeNull());
  const queryId = view.result.current.layoutPanel.query!.queryId;
  let completion!: Promise<unknown>;
  let layoutApplied!: Promise<boolean>;
  act(() => {
    const previous = view.result.current.applyDpi(240);
    layoutApplied = view.result.current.layoutPanel.apply(0);
    completion = Promise.all([previous, layoutApplied]);
  });
  expect(apply).toHaveBeenCalledOnce();
  await act(async () => { if (fails) reject(new Error("DPI recusado.")); else resolve(); await completion; });
  expect(apply.mock.calls[1][0]).toEqual({ kind: "applyLayout", selection: { queryId, candidateIndex: 0 } });
  expect(await layoutApplied).toBe(fails);
  if (!fails) expect(view.result.current.message).toBe("Esta prévia de Layout expirou.");
});
