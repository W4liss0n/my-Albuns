import type { ViewerAction } from "../contracts/generated/ViewerAction";
import type { ViewerPresentation } from "../contracts/generated/ViewerPresentation";
import type { ViewerCorrectionAction } from "../contracts/generated/ViewerCorrectionAction";
import type { PreparedEyeCorrection } from "../contracts/generated/PreparedEyeCorrection";
import type { ViewerFace } from "../contracts/generated/ViewerFace";
export type { ViewerPresentation, ViewerAction, ViewerCorrectionAction, PreparedEyeCorrection };

export interface ImageViewerWindowPort {
  open(presentation: ViewerPresentation): Promise<void>;
  update(presentation: ViewerPresentation): Promise<void>;
  close(sessionId: string): Promise<void>;
  onNavigate(callback: (action: ViewerAction) => void): Promise<() => void>;
  onClosed(callback: (sessionId: string) => void): Promise<() => void>;
  onCorrection?(callback: (action: ViewerCorrectionAction) => void): Promise<() => void>;
  prepareCorrection?(request: { sessionId: string; targetMediaId: string; referenceMediaId: string; targetFace: ViewerFace; referenceFace: ViewerFace }): Promise<PreparedEyeCorrection>;
  cancelCorrection?(): Promise<void>;
  applyCorrection?(sessionId: string, token: string): Promise<import("../domain/project").EditorProjection>;
}
