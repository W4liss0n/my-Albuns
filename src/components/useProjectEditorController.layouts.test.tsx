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

function harness(pendingKind: "applyLayout" | "setDpi") {
  const sample = layoutPanelCorpus.cases.mixed;
  const initial = structuredClone(sample.before.projection);
  const applied = structuredClone(sample.applied!.projection);
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
    if (intent.kind === "applyLayout") {
      checkSelection(intent.selection);
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
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }), readSliderDoubleClickTime: async () => 500,
    queryLayouts: async (target) => {
      const query = { ...structuredClone(sample.before.queries[target].query),
        queryId: `prepared-${++querySequence}`, revision: authoritative.state.revision };
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
    return useProjectEditorController({ projection, projectCorePort: port, runProjectMutation: runner,
      onProjectionChange: setProjection });
  });
  return { view, initial, applied, sheetId, apply, save, undo, resolve, reject };
}

test.each([false, true])("Layout, Save and Undo keep their queue order, failure=%s", async (fails) => {
  const { view, sheetId, apply, save, undo, resolve, reject, applied } = harness("applyLayout");
  act(() => view.result.current.canvasProps.sheetLayouts!.onToggle(sheetId));
  await waitFor(() => expect(view.result.current.layoutPanel.query).not.toBeNull());
  const queryId = view.result.current.layoutPanel.query!.queryId;
  let completion!: Promise<unknown>;
  act(() => { completion = Promise.all([view.result.current.layoutPanel.apply(0), view.result.current.save(), view.result.current.undo()]); });
  expect(save).not.toHaveBeenCalled();
  expect(undo).not.toHaveBeenCalled();
  await act(async () => { if (fails) reject(new Error("Aplicação recusada.")); else resolve(); await completion; });
  expect(apply.mock.calls[0][0]).toEqual({ kind: "applyLayout", selection: { queryId, candidateIndex: 0 } });
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
