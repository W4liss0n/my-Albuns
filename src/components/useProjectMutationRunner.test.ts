import { act, renderHook } from "@testing-library/react";
import {
  createElement,
  StrictMode,
  type ReactNode,
} from "react";
import { expect, test, vi } from "vitest";

import type { ProjectSessionPort } from "../application/projectPorts";
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

function projectSessionPort(): ProjectSessionPort {
  return {
    load: async () => representativeProjection,
    apply: async () => representativeProjection,
    undo: async () => representativeProjection,
    redo: async () => representativeProjection,
  };
}

function StrictModeWrapper({ children }: { children: ReactNode }) {
  return createElement(StrictMode, null, children);
}

test("remains current after StrictMode replays its mount effect", async () => {
  const port = projectSessionPort();
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

test("starts a new Project queue immediately and discards obsolete queued work", async () => {
  const first = deferredProjection();
  const firstPort = projectSessionPort();
  const secondPort = projectSessionPort();
  const obsoleteQueuedOperation = vi.fn(
    async () => representativeProjection,
  );
  const view = renderHook(
    ({
      projectId,
      port,
    }: {
      projectId: string;
      port: ProjectSessionPort;
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
    async (port: ProjectSessionPort) => {
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
