import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../../test/layoutCatalogPorts";
import { useState } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ProjectCorePort } from "../../application/projectPorts";
import type { EditorProjection, LayoutCandidate, LayoutSelection } from "../../domain/project";
import { useEditorView } from "../../state/editorView";
import { layoutPanelCorpus } from "../../test/layoutPanelPreview";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => useEditorView.setState(useEditorView.getInitialState(), true));

function harness(pendingKind: "applyLayout" | "lockLayout" | "unlockLayout" | "setDpi", locked = false,
  candidates?: LayoutCandidate[]) {
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
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this layout test."); };
  const checkSelection = (selection: LayoutSelection) => {
    if (selection.queryId !== prepared?.queryId || prepared.revision !== authoritative.state.revision) {
      throw new Error("Esta prévia de layout expirou.");
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
    validateMediaFolderName: async () => { throw new Error("Folder validation is not configured in this fixture."); },
    queryLayouts: async (target) => {
      const query = { ...structuredClone(sample.before.queries[target].query),
        queryId: `prepared-${++querySequence}`, revision: authoritative.state.revision,
        locked: authoritative.state.album.sheets[0].layoutLocked };
      if (candidates) {
        query.listing = { ...query.listing, candidates: structuredClone(candidates), cycleOrder: [1, 3, 2, 0, 4] };
        query.candidateRequiresLock = candidates.map(() => false);
      }
      prepared = query;
      return query;
    },
    previewLayout: async (selection) => {
      checkSelection(selection);
      return structuredClone(sample.before.queries[sheetId].previews[selection.candidateIndex]);
    },
    previewDecorativeDrop: async () => { throw new Error("Decorative preview is not configured in this fixture."); },
    previewPhotoZoom: async () => { throw new Error("Photo zoom preview is not configured in this fixture."); },
    previewFrameStyle: unsupported, previewPhotoAngle: unsupported, previewFrameGeometry: unsupported,
    saveAs: unsupported, validateAlbumInformation: unsupported, applyWithOutcome: unsupported,
    importMedia: unsupported, resolvePhotoDropTarget: unsupported, replaceImage: unsupported, relink: unsupported,
  };
  const view = renderHook(() => {
    const [projection, setProjection] = useState<EditorProjection>(initial);
    const runner = useProjectMutationRunner(initial.state.projectId, port);
    return useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort, projection, projectCorePort: port, runProjectMutation: runner,
      onProjectionChange: setProjection });
  });
  return { view, initial, applied, sheetId, apply, save, undo, resolve, reject, queryCount: () => querySequence };
}

function cycleCandidate(origin: "automatic" | "custom", options: { favorite?: boolean; last?: boolean } = {}): LayoutCandidate {
  const sample = layoutPanelCorpus.cases.mixed.before.queries;
  const definition = Object.values(sample)[0].query.listing.candidates[0].layout.definition;
  return { isLastApplied: options.last ?? false, customId: origin === "custom" ? "custom-id" : null,
    favoriteId: options.favorite ? "favorite-id" : null, layout: { origin, definition } };
}

// Core order: last applied, favorites, custom, automatic. Cycle order: [1, 3, 2, 0, 4].
const cycleCandidates = [
  cycleCandidate("automatic", { last: true }),
  cycleCandidate("custom", { favorite: true }),
  cycleCandidate("automatic", { favorite: true }),
  cycleCandidate("custom"),
  cycleCandidate("automatic"),
];

test.each([{ direction: "next", candidateIndex: 4 }, { direction: "previous", candidateIndex: 2 }] as const)(
  "$direction Layout with the panel closed queries the centered Sheet and applies in cycle order", async ({ direction, candidateIndex }) => {
    const { view, sheetId, apply, queryCount } = harness("setDpi", false, cycleCandidates);
    act(() => useEditorView.getState().centerSheet(sheetId));
    let applied!: Promise<boolean>;
    act(() => { applied = view.result.current.cycleLayout(direction); });
    expect(await applied).toBe(true);
    expect(queryCount()).toBe(1);
    expect(apply).toHaveBeenCalledExactlyOnceWith({ kind: "applyLayout", selection: { queryId: "prepared-1", candidateIndex } }, expect.anything());
  });

test("with the panel open, the Layout cycle reuses the prepared panel query", async () => {
  const { view, sheetId, apply, queryCount } = harness("setDpi", false, cycleCandidates);
  act(() => view.result.current.canvasProps.sheetLayouts!.onToggle(sheetId));
  await waitFor(() => expect(view.result.current.layoutPanel.query).not.toBeNull());
  const queryId = view.result.current.layoutPanel.query!.queryId;
  const queriesBefore = queryCount();
  let applied!: Promise<boolean>;
  act(() => { applied = view.result.current.cycleLayout("next"); });
  expect(await applied).toBe(true);
  expect(apply).toHaveBeenCalledExactlyOnceWith({ kind: "applyLayout", selection: { queryId, candidateIndex: 4 } }, expect.anything());
  await waitFor(() => expect(queryCount()).toBeGreaterThan(queriesBefore));
});

test("a locked Sheet or Sheet Edit Mode ignores the Layout cycle without an error", async () => {
  const locked = harness("setDpi", true, cycleCandidates);
  act(() => useEditorView.getState().centerSheet(locked.sheetId));
  expect(await locked.view.result.current.cycleLayout("next")).toBe(false);
  expect(locked.apply).not.toHaveBeenCalled();
  expect(locked.view.result.current.message).toBeNull();

  const editing = harness("setDpi", false, cycleCandidates);
  act(() => useEditorView.getState().enterSheetEdit(editing.sheetId));
  expect(await editing.view.result.current.cycleLayout("next")).toBe(false);
  expect(editing.apply).not.toHaveBeenCalled();
});

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
  if (!fails) expect(view.result.current.message).toBe("Esta prévia de layout expirou.");
});
