import { useCallback, useLayoutEffect, useMemo } from "react";

import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import type { ProjectMutationOperation, ProjectMutationOutcome, ProjectMutationRunner, ProjectMutationRunOptions } from "../application/projectMutation";
export type { ProjectMutationOperation, ProjectMutationOutcome, ProjectMutationRunner } from "../application/projectMutation";

interface ProjectMutationContext {
  port: ProjectCorePort;
  current: boolean;
  active: boolean;
  latestProjection: EditorProjection | null;
  tail: Promise<ProjectMutationOutcome | null>;
}

export function useProjectMutationRunner(
  projectId: string,
  port: ProjectCorePort,
): ProjectMutationRunner {
  const context = useMemo(
    () => createContext(port),
    [port, projectId],
  );

  useLayoutEffect(() => {
    context.current = true;
    return () => {
      context.current = false;
    };
  }, [context]);

  const run = useCallback(
    (
      operation: ProjectMutationOperation,
      options?: ProjectMutationRunOptions,
    ) =>
      context.current
        ? enqueueMutation(context, operation, options)
        : Promise.resolve({ status: "obsolete" } as const),
    [context],
  );
  const waitForIdle = useCallback(
    () => waitForMutationQueue(context),
    [context],
  );
  return useMemo(() => ({ run, waitForIdle }), [run, waitForIdle]);
}

function createContext(
  port: ProjectCorePort,
): ProjectMutationContext {
  return {
    port,
    current: false,
    active: false,
    latestProjection: null,
    tail: Promise.resolve(null),
  };
}

function enqueueMutation(
  context: ProjectMutationContext,
  operation: ProjectMutationOperation,
  options?: ProjectMutationRunOptions,
): Promise<ProjectMutationOutcome> {
  const previousTail = context.tail;
  const hasPendingPredecessor = context.active;
  const execute = async (
    previousOutcome: ProjectMutationOutcome | null,
  ): Promise<ProjectMutationOutcome> => {
    if (!context.current) return { status: "obsolete" };
    if (
      hasPendingPredecessor &&
      options?.cancelAfterPendingFailure &&
      previousOutcome?.status === "failed"
    ) {
      return previousOutcome;
    }
    try {
      const projection = await operation(
        context.port,
        context.latestProjection,
      );
      if (!context.current) return { status: "obsolete" };
      context.latestProjection = projection;
      return { status: "completed", projection };
    } catch (error: unknown) {
      return context.current
        ? { status: "failed", error }
        : { status: "obsolete" };
    }
  };
  const result = hasPendingPredecessor
    ? previousTail.then(execute, () => execute(null))
    : execute(null);
  context.active = true;
  context.tail = result;
  void result.then(() => {
    if (context.tail === result) context.active = false;
  });
  return result;
}

async function waitForMutationQueue(
  context: ProjectMutationContext,
): Promise<ProjectMutationOutcome | null> {
  if (!context.active) return null;
  let observedTail = context.tail;
  while (true) {
    const outcome = await observedTail;
    if (context.tail === observedTail) return outcome;
    observedTail = context.tail;
  }
}
