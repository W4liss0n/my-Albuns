import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { ProjectSessionPort } from "../application/projectPorts";
import { createAlbumDesignProjectDraft } from "../application/projectSettingsDraft";
import type { EditorProjection } from "../domain/project";
import { representativeProjection } from "../test/projectFixtures";
import { useProjectMutationRunner } from "./useProjectMutationRunner";
import { useProjectMutations } from "./useProjectMutations";

function deferredProjection() {
  let resolve!: (projection: EditorProjection) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<EditorProjection>((resolver, rejecter) => {
    resolve = resolver;
    reject = rejecter;
  });
  return { promise, reject, resolve };
}

function projectSessionPort(
  apply: ProjectSessionPort["apply"],
  undo: ProjectSessionPort["undo"],
): ProjectSessionPort {
  return {
    load: async () => representativeProjection,
    validateAlbumInformation: async () => ({
      errors: [],
      impact: { sheetWidthPx: 7_087, pageWidthPx: 3_543, heightPx: 3_543 },
    }),
    apply,
    undo,
    redo: async () => representativeProjection,
    save: async () => ({
      outcome: {
        kind: "alreadyCurrent",
        revision: representativeProjection.state.revision,
      },
      projection: representativeProjection,
    }),
  };
}

test("preserves Redo when preceding History already materialized the Album Design target", async () => {
  const pendingUndo = deferredProjection();
  const target = {
    ...representativeProjection.state.album.visualDefaults,
    background: {
      scope: "bothSides" as const,
      both: { kind: "color" as const, rgb: "#F7F5F0" },
    },
  };
  const afterHistory = {
    ...target,
    overlay: {
      scope: "bothSides" as const,
      both: { kind: "media" as const, mediaId: "history-overlay" },
    },
  };
  const afterUndo: EditorProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      revision: representativeProjection.state.revision + 1,
      canRedo: true,
      album: {
        ...representativeProjection.state.album,
        visualDefaults: afterHistory,
      },
    },
  };
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () => afterUndo);
  const undo = vi.fn<ProjectSessionPort["undo"]>(() => pendingUndo.promise);
  const port = projectSessionPort(apply, undo);
  const redo = vi.fn<ProjectSessionPort["redo"]>(async () =>
    representativeProjection,
  );
  port.redo = redo;
  const onProjectionChange = vi.fn();
  const draft = createAlbumDesignProjectDraft(
    representativeProjection.state.revision,
    representativeProjection.state.album.visualDefaults,
  ).transition(target);
  const view = renderHook(() => {
    const runner = useProjectMutationRunner(
      representativeProjection.state.projectId,
      port,
    );
    return useProjectMutations({
      projection: representativeProjection,
      runProjectMutation: runner,
      onProjectionChange,
    });
  });

  act(() => view.result.current.undo());
  await waitFor(() => expect(undo).toHaveBeenCalledOnce());
  let completed = false;
  await act(async () => {
    const completion = view.result.current.applyAlbumDesign(draft);
    view.result.current.redo();
    pendingUndo.resolve(afterUndo);
    completed = await completion;
  });

  expect(completed).toBe(true);
  expect(apply).not.toHaveBeenCalled();
  await waitFor(() => expect(redo).toHaveBeenCalledOnce());
  expect(onProjectionChange).toHaveBeenCalledWith(afterUndo);
});
