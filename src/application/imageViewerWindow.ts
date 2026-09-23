import type { ViewerAction } from "../contracts/generated/ViewerAction";
import type { ViewerPresentation } from "../contracts/generated/ViewerPresentation";
export type { ViewerPresentation, ViewerAction };
export interface ViewerCorrectionAction {
  sessionId: string;
  kind: "start" | "browse" | "select" | "preview" | "apply" | "cancel";
  referenceMediaId?: string;
  targetFace?: import("../image-viewer/faceLandmarks").Face;
  referenceFace?: import("../image-viewer/faceLandmarks").Face;
}
export interface PreparedEyeCorrection { token: string; url: string }

export interface ImageViewerWindowPort {
  open(presentation: ViewerPresentation): Promise<void>;
  update(presentation: ViewerPresentation): Promise<void>;
  close(sessionId: string): Promise<void>;
  onNavigate(callback: (action: ViewerAction) => void): Promise<() => void>;
  onClosed(callback: (sessionId: string) => void): Promise<() => void>;
  onCorrection?(callback: (action: ViewerCorrectionAction) => void): Promise<() => void>;
  prepareCorrection?(request: { sessionId: string; targetMediaId: string; referenceMediaId: string; targetFace: import("../image-viewer/faceLandmarks").Face; referenceFace: import("../image-viewer/faceLandmarks").Face }): Promise<PreparedEyeCorrection>;
  cancelCorrection?(): Promise<void>;
  applyCorrection?(sessionId: string, token: string): Promise<import("../domain/project").EditorProjection>;
}
