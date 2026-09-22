export interface ViewerPresentation {
  sessionId: string;
  revision: number;
  mediaId: string;
  name: string;
  url: string | null;
  state: string;
  canPrevious: boolean;
  canNext: boolean;
}

export interface ImageViewerWindowPort {
  open(presentation: ViewerPresentation): Promise<void>;
  update(presentation: ViewerPresentation): Promise<void>;
  close(sessionId: string): Promise<void>;
  onNavigate(callback: (action: { sessionId: string; offset: -1 | 1 }) => void): Promise<() => void>;
  onClosed(callback: (sessionId: string) => void): Promise<() => void>;
}
