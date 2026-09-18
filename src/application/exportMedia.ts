import type { EditorProjection } from "../domain/project";
import type { ConfiguredExportSelection, ImageProcessingProblem, ImageProcessingProgress } from "./projectPorts";

export interface ExportMediaProblem {
  mediaId: string;
  fileName: string;
  state: "absent" | "unavailable";
}

export class MediaExportBlockedError extends Error {
  constructor(readonly problems: ExportMediaProblem[]) {
    super("Confira os arquivos necessários à exportação.");
    this.name = "MediaExportBlockedError";
  }
}

/** Recovery updates the editing session before the immutable export is planned. */
export interface ExportMediaPort {
  inspect(selection: ConfiguredExportSelection): Promise<ExportMediaProblem[]>;
  relink(selection: ConfiguredExportSelection, onProgress: (progress: ImageProcessingProgress) => void): Promise<{
    projection: EditorProjection;
    problems: ExportMediaProblem[];
    notes: ImageProcessingProblem[];
  }>;
}
