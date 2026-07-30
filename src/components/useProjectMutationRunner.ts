import { useCallback, useEffect, useRef } from "react";

import type { ProjectSessionPort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";

export type ProjectMutationOutcome =
  | { status: "completed"; projection: EditorProjection }
  | { status: "failed"; error: unknown }
  | { status: "obsolete" };

type ProjectMutationOperation = (
  port: ProjectSessionPort,
) => Promise<EditorProjection>;

interface ProjectMutationContext {
  projectId: string;
  port: ProjectSessionPort;
  current: boolean;
  active: boolean;
  tail: Promise<void>;
}

export function useProjectMutationRunner(
  projectId: string,
  port: ProjectSessionPort,
) {
  const contextRef = useRef<ProjectMutationContext | null>(null);
  const context = contextRef.current;
  if (
    context === null ||
    context.projectId !== projectId ||
    context.port !== port
  ) {
    if (context) context.current = false;
    contextRef.current = createContext(projectId, port);
  }

  useEffect(() => {
    const activeContext = contextRef.current;
    if (activeContext) activeContext.current = true;
    return () => {
      if (activeContext) activeContext.current = false;
    };
  }, [port, projectId]);

  return useCallback(
    (operation: ProjectMutationOperation) => {
      const activeContext = contextRef.current;
      return activeContext
        ? enqueueMutation(activeContext, operation)
        : Promise.resolve({ status: "obsolete" } as const);
    },
    [],
  );
}

function createContext(
  projectId: string,
  port: ProjectSessionPort,
): ProjectMutationContext {
  return {
    projectId,
    port,
    current: true,
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
