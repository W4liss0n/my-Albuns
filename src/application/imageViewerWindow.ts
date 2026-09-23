import type { ViewerAction } from "../contracts/generated/ViewerAction";
import type { ViewerPresentation } from "../contracts/generated/ViewerPresentation";
export type { ViewerPresentation, ViewerAction };

export interface ImageViewerWindowPort {
  open(presentation: ViewerPresentation): Promise<void>;
  update(presentation: ViewerPresentation): Promise<void>;
  close(sessionId: string): Promise<void>;
  onNavigate(callback: (action: ViewerAction) => void): Promise<() => void>;
  onClosed(callback: (sessionId: string) => void): Promise<() => void>;
}
