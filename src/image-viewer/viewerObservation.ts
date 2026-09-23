import { observeSnapshot } from "../application/observeSnapshot";
import type { ViewerPresentation } from "../application/imageViewerWindow";

export function observeViewerPresentation(client: {
  onPresentation(callback: (presentation: ViewerPresentation) => void): Promise<() => void>;
  current(): Promise<ViewerPresentation | null>;
}, receive: (presentation: ViewerPresentation | null) => void) {
  let latest: ViewerPresentation | null = null;
  return observeSnapshot({
    subscribe: client.onPresentation,
    read: client.current,
    receive: (next) => {
      if (next && latest && next.sessionId === latest.sessionId && next.revision < latest.revision) return;
      latest = next;
      receive(next);
    },
  });
}
