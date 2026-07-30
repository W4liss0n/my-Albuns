import { useCallback, useLayoutEffect, useMemo } from "react";

import type { ProjectSessionPort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";

export type ProjectMutationOutcome =
  | { status: "completed"; projection: EditorProjection }
  | { status: "failed"; error: unknown }
  | { status: "obsolete" };

export type ProjectMutationOperation = (
  port: ProjectSessionPort,
) => Promise<EditorProjection>;

export type ProjectMutationRunner = (
  operation: ProjectMutationOperation,
) => Promise<ProjectMutationOutcome>;

interface ProjectMutationContext {
  port: ProjectSessionPort;
  current: boolean;
  active: boolean;
  tail: Promise<void>;
}

export function useProjectMutationRunner(
  projectId: string,
  port: ProjectSessionPort,
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

  return useCallback(
    (operation: ProjectMutationOperation) =>
      context.current
        ? enqueueMutation(context, operation)
        : Promise.resolve({ status: "obsolete" } as const),
    [context],
  );
}

function createContext(
  port: ProjectSessionPort,
): ProjectMutationContext {
  return {
    port,
    current: false,
    active: false,
    tail: Promise.resolve(),
  };
}

function enqueueMutation(
  context: ProjectMutationContext,
  operation: ProjectMutationOperation,
): Promise<ProjectMutationOutcome> {
  const execute = async (): Promise<ProjectMutationOutcome> => {
    if (!context.current) return { status: "obsolete" };
    try {
      const projection = await operation(context.port);
      return context.current
        ? { status: "completed", projection }
        : { status: "obsolete" };
    } catch (error: unknown) {
      return context.current
        ? { status: "failed", error }
        : { status: "obsolete" };
    }
  };
  const result = context.active
    ? context.tail.then(execute, execute)
    : execute();
  context.active = true;
  const settledTail = result.then(() => undefined);
  context.tail = settledTail;
  void settledTail.then(() => {
    if (context.tail === settledTail) context.active = false;
  });
  return result;
}
