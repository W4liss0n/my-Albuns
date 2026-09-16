import { createPortal } from "react-dom";
import type { MediaCatalogItem } from "../domain/project";
import { MediaThumbnail } from "./MediaThumbnail";
import "./MediaDragGhost.css";

export function MediaDragGhost({ media, previewUrl, missing, x, y }: {
  media: MediaCatalogItem;
  previewUrl?: string;
  missing: boolean;
  x: number;
  y: number;
}) {
  const sourceWidth = media.sourceWidthPx ?? 0;
  const sourceHeight = media.sourceHeightPx ?? 0;
  const ratio = sourceWidth > 0 && sourceHeight > 0 ? sourceWidth / sourceHeight : 1;
  const width = Math.min(80, 60 * ratio);
  const height = width / ratio;
  const left = Math.max(0, Math.min(x + 6, window.innerWidth - width));
  const top = Math.max(0, Math.min(y + 6, window.innerHeight - height));

  return createPortal(<div aria-hidden="true" className="media-drag-ghost"
    data-media-drag-ghost={media.id}
    style={{ width, height, transform: `translate3d(${left}px, ${top}px, 0)` }}>
    <MediaThumbnail media={media} previewUrl={previewUrl} missing={missing} loading="eager" />
  </div>, document.body);
}
