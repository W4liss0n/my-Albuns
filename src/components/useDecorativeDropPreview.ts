import { useCallback, useEffect, useRef, useState } from "react";
import type { DecorativeDropPreview, DecorativeDropRequest } from "../domain/project";
import type { CanvasPhotoDropPoint } from "./albumCanvasContract";
import type { MediaDrag } from "./useMediaDragGesture";

interface Input {
  projectId: string;
  revision?: number;
  drag?: MediaDrag | null;
  point(x: number, y: number): CanvasPhotoDropPoint | null;
  preview?: (request: DecorativeDropRequest) => Promise<DecorativeDropPreview | null>;
  commit?: (request: DecorativeDropRequest) => Promise<boolean>;
  cancel?: () => void;
}

export function useDecorativeDropPreview(input: Input) {
  const latest = useRef(input);
  latest.current = input;
  const requestId = useRef(0);
  const committing = useRef<{ projectId: string; gestureId: number } | null>(null);
  const [resolved, setResolved] = useState<{ gestureId: number; mediaId: string; preview: DecorativeDropPreview } | null>(null);
  const resolvedRef = useRef(resolved);
  resolvedRef.current = resolved;
  const clear = useCallback(() => { requestId.current += 1; setResolved(null); }, []);

  useEffect(() => {
    const { drag, projectId, revision } = input;
    if (committing.current?.projectId === projectId && committing.current.gestureId === drag?.gestureId) return;
    const sequence = ++requestId.current;
    if (!drag || drag.kind !== "decorative") { setResolved(null); return; }
    const point = latest.current.point(drag.x, drag.y);
    const previewPort = latest.current.preview;
    if (!point || !previewPort) {
      setResolved(null);
      if (drag.phase === "drop") latest.current.cancel?.();
      return;
    }
    const role = drag.shiftKey ? "overlay" : "background";
    const cached = resolvedRef.current;
    if (drag.phase === "dragging" && cached?.gestureId === drag.gestureId && cached.mediaId === drag.mediaId &&
        cached.preview.role === role && cached.preview.sheet.sheetId === point.sheetId &&
        (revision === undefined || cached.preview.revision === revision)) {
      const rect = cached.preview.zoneRect;
      if (point.xUm >= rect.x && point.xUm < rect.x + rect.width && point.yUm >= rect.y && point.yUm < rect.y + rect.height) return;
    }
    setResolved(null);
    const request: DecorativeDropRequest = { ...point, mediaId: drag.mediaId, role };
    void previewPort(request).then(async (preview) => {
      if (sequence !== requestId.current) return;
      if (!preview || (latest.current.revision !== undefined && preview.revision !== latest.current.revision)) {
        if (drag.phase === "drop") latest.current.cancel?.();
        return;
      }
      setResolved({ gestureId: drag.gestureId, mediaId: drag.mediaId, preview });
      if (drag.phase !== "drop") return;
      const commit = { projectId, gestureId: drag.gestureId };
      committing.current = commit;
      try { await latest.current.commit?.(request); }
      finally {
        if (committing.current === commit) committing.current = null;
        if (latest.current.projectId === projectId && latest.current.drag?.gestureId === drag.gestureId) {
          setResolved(null);
          latest.current.cancel?.();
        }
      }
    }).catch(() => {
      if (sequence === requestId.current) {
        setResolved(null);
        if (drag.phase === "drop") latest.current.cancel?.();
      }
    });
    return () => { if (sequence === requestId.current) requestId.current += 1; };
  }, [input.drag, input.projectId, input.revision]);

  const preview = input.drag?.kind === "decorative" && resolved?.gestureId === input.drag.gestureId &&
    resolved.mediaId === input.drag.mediaId && resolved.preview.role === (input.drag.shiftKey ? "overlay" : "background") &&
    (input.revision === undefined || resolved.preview.revision === input.revision) ? resolved.preview : null;
  return { preview, clear };
}
