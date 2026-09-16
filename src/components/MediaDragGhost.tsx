import { useState } from "react";
import { createPortal } from "react-dom";
import type { MediaCatalogItem } from "../domain/project";
import { loadedMediaPreviewImage } from "../application/mediaPreviewImages";
import { IMAGE_DRAG_GHOST_STYLE, imageDragGhostGeometry, imageDragGhostPosition } from "../ui/imageDragGhostVisual";
import { MediaThumbnail } from "./MediaThumbnail";
import "./MediaDragGhost.css";

export function MediaDragGhost({ media, previewUrl, missing, x, y }: {
  media: MediaCatalogItem;
  previewUrl?: string;
  missing: boolean;
  x: number;
  y: number;
}) {
  const [loaded, setLoaded] = useState<{ url: string; width: number; height: number } | null>(null);
  const borrowed = previewUrl ? loadedMediaPreviewImage(previewUrl) : undefined;
  const previewSize = borrowed ? { width: borrowed.naturalWidth, height: borrowed.naturalHeight }
    : loaded?.url === previewUrl ? loaded : null;
  // The Core projects 1 × 1 until a Photo has observed source dimensions.
  const unobserved = media.sourceWidthPx === 1 && media.sourceHeightPx === 1;
  const style = IMAGE_DRAG_GHOST_STYLE;
  const geometry = imageDragGhostGeometry(
    previewSize?.width ?? (unobserved ? null : media.sourceWidthPx),
    previewSize?.height ?? (unobserved ? null : media.sourceHeightPx),
  );
  const position = imageDragGhostPosition({ x, y }, geometry, { width: window.innerWidth, height: window.innerHeight });

  return createPortal(<div aria-hidden="true" className="media-drag-ghost"
    data-media-drag-ghost={media.id}
    onLoadCapture={(event) => {
      const image = event.target;
      if (previewUrl && image instanceof HTMLImageElement && image.getAttribute("src") === previewUrl &&
        image.naturalWidth > 0 && image.naturalHeight > 0) {
        setLoaded({ url: previewUrl, width: image.naturalWidth, height: image.naturalHeight });
      }
    }}
    style={{ width: geometry.width, height: geometry.height, opacity: style.opacity,
      transform: `translate3d(${position.x}px, ${position.y}px, 0)` }}>
    <span className="media-drag-ghost__decoration" style={{
      left: geometry.shadow.x, top: geometry.shadow.y, width: geometry.shadow.width, height: geometry.shadow.height,
      background: style.shadow.color, opacity: style.shadow.opacity }} />
    <span className="media-drag-ghost__decoration" style={{
      left: geometry.border.x, top: geometry.border.y, width: geometry.border.width, height: geometry.border.height,
      background: style.border.color }} />
    <MediaThumbnail media={media} previewUrl={previewUrl} missing={missing} loading="eager" />
  </div>, document.body);
}
