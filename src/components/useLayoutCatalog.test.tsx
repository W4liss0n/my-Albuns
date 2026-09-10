import { useState } from "react";
import { act, fireEvent, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { ProjectCorePort } from "../application/projectPorts";
import type { ProjectDialogAction, ProjectDialogPort } from "../application/projectDialogPort";
import type { EditorProjection } from "../domain/project";
import { representativeProjection } from "../test/projectFixtures";
import { useLayoutCatalog } from "./useLayoutCatalog";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

const layoutId = "00000000-0000-4000-8000-000000000099";
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function harness() {
  let authoritative = structuredClone(representativeProjection);
  let revision = 0;
  const initial = authoritative;
  const capturedRevisions: number[] = [];
  const saveCustomLayout = vi.fn(async () => {
    capturedRevisions.push(authoritative.state.revision);
    return { catalogRevision: ++revision, layoutId, created: true };
  });
  const refreshLayoutCatalog = vi.fn(async () => revision);
  const deleteCustomLayout = vi.fn(async () => ++revision);
  const onError = vi.fn();
  const present = vi.fn(async () => undefined);
  const dismiss = vi.fn(async () => undefined);
  let onAction: (action: ProjectDialogAction) => void = () => undefined;
  const dialogPort: ProjectDialogPort = { acquire: (listener) => {
    onAction = listener;
    return { present, dismiss };
  } };
  // Only catalog methods are reached by this hook; adjacent operations use the runner seam.
  const port = { saveCustomLayout, refreshLayoutCatalog, deleteCustomLayout } as unknown as ProjectCorePort;
  const view = renderHook(() => {
    const [projection, setProjection] = useState(initial);
    const runner = useProjectMutationRunner(projection.state.projectId, port);
    const catalog = useLayoutCatalog({ projection, runner, port, dialogPort, onError });
    return { catalog, runner, projection, setProjection };
  });
  return { view, initial, capturedRevisions, saveCustomLayout, refreshLayoutCatalog, deleteCustomLayout,
    onError, present, dismiss, action: (action: ProjectDialogAction) => onAction(action),
    replace: (next: EditorProjection) => { authoritative = next; }, setRevision: (value: number) => { revision = value; } };
}

test.each([false, true])("saving captures geometry after the queued edit; edit failure=%s", async (fails) => {
  const h = harness();
  await act(async () => { await h.view.result.current.runner.waitForIdle(); });
  const gate = deferred<EditorProjection>();
  const changed = { ...h.initial, state: { ...h.initial.state, revision: h.initial.state.revision + 1 } };
  let saved!: Promise<void>;
  let observed: number | undefined;
  await act(async () => {
    void h.view.result.current.runner.run(async () => {
      const next = await gate.promise;
      h.replace(next);
      return next;
    });
    saved = h.view.result.current.catalog.save("sheet-001");
    void h.view.result.current.runner.run(async (_port, latest) => {
      observed = latest?.state.revision;
      return latest!;
    });
  });
  expect(h.saveCustomLayout).not.toHaveBeenCalled();
  await act(async () => {
    if (fails) gate.reject(new Error("Falha ao mover Frame")); else gate.resolve(changed);
    await saved;
    await h.view.result.current.runner.waitForIdle();
  });
  if (fails) {
    expect(h.saveCustomLayout).not.toHaveBeenCalled();
    expect(h.onError).toHaveBeenCalledWith("Falha ao mover Frame");
    expect(h.view.result.current.catalog.notice).toBeNull();
  } else {
    expect(h.capturedRevisions).toEqual([changed.state.revision]);
    expect(observed).toBe(changed.state.revision);
    expect(h.view.result.current.catalog.revealId).toBe(layoutId);
  }
  expect(h.view.result.current.projection).toEqual(h.initial);
});

test("duplicate saves keep the catalog revision and retain the pending reveal until acknowledged", async () => {
  const h = harness();
  h.saveCustomLayout.mockResolvedValue({ catalogRevision: 7, layoutId, created: false });
  await act(async () => { await h.view.result.current.catalog.save("sheet-001"); });
  expect(h.view.result.current.catalog.revision).toBe(7);
  expect(h.view.result.current.catalog.notice).toBe("Este Layout já está em Personalizados.");
  expect(h.present).not.toHaveBeenCalled();
  h.view.rerender();
  expect(h.view.result.current.catalog.revealId).toBe(layoutId);
  act(() => h.view.result.current.catalog.acknowledgeReveal());
  expect(h.view.result.current.catalog.revealId).toBeNull();
});

test("deletion requires confirmation, dismisses cancellation, and preserves the Project", async () => {
  const h = harness();
  await act(async () => h.view.result.current.catalog.requestDelete(layoutId));
  expect(h.present).toHaveBeenLastCalledWith({ kind: "layoutDeletionConfirmation", busy: false });
  expect(h.deleteCustomLayout).not.toHaveBeenCalled();
  await act(async () => h.action("cancelLayoutDeletion"));
  expect(h.dismiss).toHaveBeenCalledOnce();
  expect(h.deleteCustomLayout).not.toHaveBeenCalled();
  await act(async () => h.view.result.current.catalog.requestDelete(layoutId));
  await act(async () => h.action("confirmLayoutDeletion"));
  await waitFor(() => expect(h.view.result.current.catalog.busy).toBe(false));
  expect(h.deleteCustomLayout).toHaveBeenCalledExactlyOnceWith(layoutId);
  expect(h.view.result.current.catalog.revision).toBe(1);
  expect(h.view.result.current.projection).toEqual(h.initial);
});

test("mount, focus and explicit refresh consume only newer confirmed revisions; failures retain them", async () => {
  const h = harness();
  await waitFor(() => expect(h.refreshLayoutCatalog).toHaveBeenCalledOnce());
  h.setRevision(4);
  await act(async () => fireEvent.focus(window));
  expect(h.view.result.current.catalog.revision).toBe(4);
  h.setRevision(5);
  await act(async () => { await h.view.result.current.catalog.refresh(); });
  expect(h.view.result.current.catalog.revision).toBe(5);
  h.refreshLayoutCatalog.mockRejectedValue(new Error("Catálogo indisponível"));
  await act(async () => fireEvent.focus(window));
  expect(h.view.result.current.catalog.revision).toBe(5);
  expect(h.onError).toHaveBeenCalledWith("Catálogo indisponível");
  expect(h.view.result.current.projection).toEqual(h.initial);
});

test("a deleted catalog write that fails leaves the previous revision available and closes its confirmation", async () => {
  const h = harness();
  h.setRevision(3);
  await act(async () => { await h.view.result.current.catalog.refresh(); });
  h.deleteCustomLayout.mockRejectedValue(new Error("Sem acesso para gravar"));
  await act(async () => h.view.result.current.catalog.requestDelete(layoutId));
  await act(async () => h.action("confirmLayoutDeletion"));
  await waitFor(() => expect(h.onError).toHaveBeenCalledWith("Sem acesso para gravar"));
  expect(h.dismiss).toHaveBeenCalledOnce();
  expect(h.view.result.current.catalog.revision).toBe(3);
  expect(h.view.result.current.catalog.notice).toBeNull();
});
