import type { EditorProjection } from "../domain/project";
import type { ProjectCorePort } from "./projectPorts";

export type ProjectMutationOutcome =
  | { status: "completed"; projection: EditorProjection }
  | { status: "failed"; error: unknown }
  | { status: "obsolete" };

export type ProjectMutationOperation = (
  port: ProjectCorePort,
  latestProjection: EditorProjection | null,
) => Promise<EditorProjection>;

export interface ProjectMutationRunOptions {
  cancelAfterPendingFailure?: boolean;
}

export interface ProjectMutationRunner {
  run(
    operation: ProjectMutationOperation,
    options?: ProjectMutationRunOptions,
  ): Promise<ProjectMutationOutcome>;
  waitForIdle(): Promise<ProjectMutationOutcome | null>;
}
