import { useState } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { ComposedFrame, EditorProjection, LayoutQueryResult, ProjectIntent } from "../domain/project";
import { frameDeletionCorpus } from "../test/frameDeletionPreview";
import { useLayoutPanel } from "./useLayoutPanel";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

const initial = structuredClone(frameDeletionCorpus.before) as EditorProjection;
const sheetId = initial.state.album.sheets[0].id;
const frames = initial.composition.sheets[0].frames;
function query(id: string, target = sheetId, revision = initial.state.revision): LayoutQueryResult {
  return { queryId: id, projectId: initial.state.projectId, revision, catalogRevision: 0, sheetId: target,
    frameCount: frames.length, locked: false, settings: { permission: "pagesAndSheet", marginUm: 15000, gapUm: 5000, minimumSideUm: 20000 },
    listing: { algorithmVersion: 1, generationStatus: "candidates", candidates: [
      { isLastApplied: false, customId: null, layout: { origin: "automatic", definition: {
        surface: { type: "doubleSheet", widthUm: 600000, heightUm: 300000 }, scope: "page",
        positions: frames.map((frame) => frame.clipRect),
      } } },
    ] } };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function harness() {
  const queryLayouts = vi.fn(async (target: string) => query(`query-${target}`, target));
  const previewLayout = vi.fn(async (): Promise<ComposedFrame[]> => frames);
  const commit = vi.fn(async (_intent: ProjectIntent) => true);
  const runner = { waitForIdle: vi.fn(async () => null) } as unknown as ProjectMutationRunner;
  const onError = vi.fn();
  const view = renderHook(({ editing, blocked }) => {
    const [projection, setProjection] = useState(initial);
    return { panel: useLayoutPanel({ projection, editing, disabled: blocked,
      port: { queryLayouts, previewLayout }, runner, commit, onError }), setProjection };
  }, { initialProps: { editing: false, blocked: false } });
  return { view, queryLayouts, previewLayout, commit, runner, onError };
}

test("hover uses the Core composition without mutation; exit, close and edit mode discard it", async () => {
  const { view, commit } = harness();
  act(() => view.result.current.panel.toggle(sheetId));
  await waitFor(() => expect(view.result.current.panel.query?.queryId).toBe(`query-${sheetId}`));
  act(() => view.result.current.panel.preview(0));
  expect(view.result.current.panel.composition).not.toBe(initial.composition);
  expect(commit).not.toHaveBeenCalled();
  act(() => view.result.current.panel.cancelPreview());
  expect(view.result.current.panel.composition).toBe(initial.composition);
  view.rerender({ editing: true, blocked: false });
  expect(view.result.current.panel.visible).toBe(false);
  view.rerender({ editing: false, blocked: false });
  await waitFor(() => expect(view.result.current.panel.query).not.toBeNull());
  expect(view.result.current.panel.sheetId).toBe(sheetId);
  act(() => view.result.current.panel.close());
  expect(view.result.current.panel.visible).toBe(false);
  expect(view.result.current.panel.composition).toBe(initial.composition);
});

test("only the lock confirms extra positions, and a locked query only allows unlocking", async () => {
  const { view, commit, queryLayouts } = harness();
  const expanded = query("expanded");
  expanded.frameCount = frames.length - 1;
  queryLayouts.mockResolvedValueOnce(expanded);
  act(() => view.result.current.panel.toggle(sheetId));
  await waitFor(() => expect(view.result.current.panel.query?.queryId).toBe("expanded"));
  await act(async () => { expect(await view.result.current.panel.apply(0)).toBe(false); });
  expect(commit).not.toHaveBeenCalled();
  const locked = { ...query("locked"), locked: true };
  queryLayouts.mockResolvedValue(locked);
  await act(async () => { await view.result.current.panel.lock(0); });
  expect(commit).toHaveBeenCalledExactlyOnceWith({ kind: "lockLayout", selection: { queryId: "expanded", candidateIndex: 0 } });
  await waitFor(() => expect(view.result.current.panel.query?.locked).toBe(true));
  act(() => view.result.current.panel.preview(0));
  expect(view.result.current.panel.composition).toBe(initial.composition);
  await act(async () => { expect(await view.result.current.panel.apply(0)).toBe(false); expect(await view.result.current.panel.lock(0)).toBe(false); });
  await act(async () => { await view.result.current.panel.unlock(); });
  expect(commit).toHaveBeenLastCalledWith({ kind: "unlockLayout", sheetId });
});

test("late queries and previews cannot replace a newer target or revision", async () => {
  const { view, queryLayouts, previewLayout } = harness();
  const delayed = deferred<ComposedFrame[]>();
  previewLayout.mockImplementationOnce(() => delayed.promise);
  act(() => view.result.current.panel.toggle(sheetId));
  await waitFor(() => expect(previewLayout).toHaveBeenCalledOnce());
  const other = initial.state.album.sheets[1].id;
  act(() => view.result.current.panel.toggle(other));
  expect(view.result.current.panel.displayQuery).toBeNull();
  await waitFor(() => expect(queryLayouts).toHaveBeenCalledWith(other));
  await act(async () => { delayed.resolve(frames); });
  await waitFor(() => expect(view.result.current.panel.query?.sheetId).toBe(other));
  act(() => view.result.current.setProjection({ ...initial, state: { ...initial.state, revision: initial.state.revision + 1 } }));
  expect(view.result.current.panel.query).toBeNull();
  expect(view.result.current.panel.composition).toBe(initial.composition);
});

test("changing extra positions discards a late query and its preview", async () => {
  const { view, queryLayouts } = harness();
  const delayed = deferred<LayoutQueryResult>();
  queryLayouts.mockImplementationOnce(() => delayed.promise);
  act(() => view.result.current.panel.toggle(sheetId));
  await waitFor(() => expect(queryLayouts).toHaveBeenCalledOnce());
  act(() => view.result.current.panel.configurePositions(frames.length + 2));
  expect(view.result.current.panel.query).toBeNull();
  await act(async () => { delayed.resolve(query("obsolete")); });
  await waitFor(() => expect(queryLayouts).toHaveBeenLastCalledWith(sheetId, { additionalPositions: 2, orientation: "horizontal" }));
  await waitFor(() => expect(view.result.current.panel.query?.queryId).toBe(`query-${sheetId}`));
  expect(view.result.current.panel.composition).toBe(initial.composition);
});

test.each([false, true])("confirmation retains the prepared selection behind a pending mutation, failure=%s", async (fails) => {
  const { view, commit } = harness();
  const pending = deferred<boolean>();
  commit.mockImplementationOnce(() => pending.promise);
  act(() => view.result.current.panel.toggle(sheetId));
  await waitFor(() => expect(view.result.current.panel.query).not.toBeNull());
  let applied!: Promise<boolean>;
  act(() => { applied = view.result.current.panel.apply(0); });
  act(() => view.result.current.setProjection({ ...initial, state: { ...initial.state, revision: initial.state.revision + 1 } }));
  await act(async () => { pending.resolve(!fails); await applied; });
  expect(commit).toHaveBeenCalledExactlyOnceWith({ kind: "applyLayout", selection: { queryId: `query-${sheetId}`, candidateIndex: 0 } });
  expect(view.result.current.panel.composition).toBe(initial.composition);
});
