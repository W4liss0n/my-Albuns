import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection, PhotoAngleEdit } from "../domain/project";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";
import { useFrameCompositionDraft } from "./useFrameCompositionDraft";

interface PhotoAngleEditingInput {
  projection: EditorProjection;
  frameIds: readonly string[];
  disabled: boolean;
  port: ProjectCorePort;
  runner: ProjectMutationRunner;
  commit(edit: PhotoAngleEdit): Promise<EditorProjection | null>;
  onError(message: string): void;
}

export function usePhotoAngleEditing(input: PhotoAngleEditingInput) {
  return useFrameCompositionDraft<number>({ ...input, session: input.port,
    resolve: (frameIds, angleTenths) => input.port.previewPhotoAngle({ frameIds, angleTenths }),
    commit: (frameIds, angleTenths) => input.commit({ frameIds, angleTenths }),
  });
}
