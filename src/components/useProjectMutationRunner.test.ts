import {
  act,
  render,
  renderHook,
  waitFor,
} from "@testing-library/react";
import {
  createElement,
  startTransition,
  StrictMode,
  Suspense,
  type ReactNode,
  useState,
} from "react";
import { expect, test, vi } from "vitest";

import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import { representativeProjection } from "../test/projectFixtures";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

function deferredProjection() {
  let resolve!: (value: EditorProjection) => void;
  const promise = new Promise<EditorProjection>((resolver) => {
    resolve = resolver;
  });
  return { promise, resolve };
}

function projectCorePort(): ProjectCorePort {
  return {
    load: async () => representativeProjection,
    apply: async () => representativeProjection,
    applyWithOutcome: async () => ({
      projection: representativeProjection,
      affectedFrameId: null,
    }),
    importPhoto: async () => ({
      kind: "cancelled",
      projection: representativeProjection,
    }),
    resolvePhotoDropTarget: async () => ({ kind: "invalid" }),
    relink: async () => representativeProjection,
    undo: async () => representativeProjection,
    redo: async () => representativeProjection,
    save: async () => {
      throw new Error("Salvamento não configurado neste teste.");
    },
  };
}

function StrictModeWrapper({ children }: { children: ReactNode }) {
  return createElement(StrictMode, null, children);
}

test("remains current after StrictMode replays its mount effect", async () => {
  const port = projectCorePort();
  const view = renderHook(
    () => useProjectMutationRunner("project-001", port),
    { wrapper: StrictModeWrapper },
  );

  await expect(
    view.result.current(async () => representativeProjection),
  ).resolves.toEqual({
    status: "completed",
    projection: representativeProjection,
  });
});

test("keeps Project mutations serial and applies them in request order", async () => {
  const first = deferredProjection();
  const second = deferredProjection();
  const firstOperation = vi.fn(() => first.promise);
  const secondOperation = vi.fn(() => second.promise);
  const port = projectCorePort();
  const view = renderHook(() =>
    useProjectMutationRunner("project-001", port),
  );

  const firstResult = view.result.current(firstOperation);
  const secondResult = view.result.current(secondOperation);

  expect(firstOperation).toHaveBeenCalledOnce();
  expect(secondOperation).not.toHaveBeenCalled();

  const firstProjection = {
    ...representativeProjection,
    state: { ...representativeProjection.state, revision: 26 },
  };
  await act(async () => {
    first.resolve(firstProjection);
    await first.promise;
  });

  await expect(firstResult).resolves.toEqual({
    status: "completed",
    projection: firstProjection,
  });
  expect(secondOperation).toHaveBeenCalledOnce();

  const secondProjection = {
    ...representativeProjection,
    state: { ...representativeProjection.state, revision: 27 },
  };
  await act(async () => {
    second.resolve(secondProjection);
    await second.promise;
  });
  await expect(secondResult).resolves.toEqual({
    status: "completed",
    projection: secondProjection,
  });
});

test("continues the Project queue after a mutation fails", async () => {
  const failure = new Error("primeira mutação falhou");
  const nextProjection = {
    ...representativeProjection,
    state: { ...representativeProjection.state, revision: 26 },
  };
  const firstOperation = vi.fn(async () => {
    throw failure;
  });
  const secondOperation = vi.fn(async () => nextProjection);
  const port = projectCorePort();
  const view = renderHook(() =>
    useProjectMutationRunner("project-001", port),
  );

  const failedResult = view.result.current(firstOperation);
  const nextResult = view.result.current(secondOperation);

  await expect(failedResult).resolves.toEqual({
    status: "failed",
    error: failure,
  });
  await expect(nextResult).resolves.toEqual({
    status: "completed",
    projection: nextProjection,
  });
  expect(secondOperation).toHaveBeenCalledOnce();
  expect(secondOperation).toHaveBeenCalledWith(port);
});

test("keeps the committed Project context when a concurrent render is abandoned", async () => {
  const firstPort = projectCorePort();
  const secondPort = projectCorePort();
  const suspended = new Promise<never>(() => undefined);
  let runCommittedMutation!: ReturnType<
    typeof useProjectMutationRunner
  >;
  let beginSuspendedRender!: () => void;
  let suspendedRenderAttempted = false;

  function Harness() {
    const [useSecondProject, setUseSecondProject] = useState(false);
    const runner = useProjectMutationRunner(
      useSecondProject ? "project-002" : "project-001",
      useSecondProject ? secondPort : firstPort,
    );
    beginSuspendedRender = () => {
      startTransition(() => setUseSecondProject(true));
    };
    if (useSecondProject) {
      suspendedRenderAttempted = true;
      throw suspended;
    }
    runCommittedMutation = runner;
    return null;
  }

  render(
    createElement(
      Suspense,
      { fallback: null },
      createElement(Harness),
    ),
  );
  const operation = vi.fn(
    async (_port: ProjectCorePort) => representativeProjection,
  );

  beginSuspendedRender();
  await waitFor(() => expect(suspendedRenderAttempted).toBe(true));

  await expect(runCommittedMutation(operation)).resolves.toEqual({
    status: "completed",
    projection: representativeProjection,
  });
  expect(operation.mock.calls[0]?.[0]).toBe(firstPort);
});

test("starts a new Project queue immediately and discards obsolete queued work", async () => {
  const first = deferredProjection();
  const firstPort = projectCorePort();
  const secondPort = projectCorePort();
  const obsoleteQueuedOperation = vi.fn(
    async () => representativeProjection,
  );
  const view = renderHook(
    ({
      projectId,
      port,
    }: {
      projectId: string;
      port: ProjectCorePort;
    }) => useProjectMutationRunner(projectId, port),
    {
      initialProps: {
        projectId: "project-001",
        port: firstPort,
      },
    },
  );

  const running = view.result.current(() => first.promise);
  const queued = view.result.current(obsoleteQueuedOperation);

  view.rerender({
    projectId: "project-002",
    port: secondPort,
  });
  const currentProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      projectId: "project-002",
      revision: 26,
    },
  };
  const currentOperation = vi.fn(
    async (port: ProjectCorePort) => {
      expect(port).toBe(secondPort);
      return currentProjection;
    },
  );

  await expect(view.result.current(currentOperation)).resolves.toEqual({
    status: "completed",
    projection: currentProjection,
  });

  await act(async () => {
    first.resolve(representativeProjection);
    await first.promise;
  });
  await expect(running).resolves.toEqual({ status: "obsolete" });
  await expect(queued).resolves.toEqual({ status: "obsolete" });
  expect(obsoleteQueuedOperation).not.toHaveBeenCalled();
});
