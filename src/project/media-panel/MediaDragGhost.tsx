import { useRef, useState, type SyntheticEvent } from "react";
import { createPortal } from "react-dom";
import type { DecorativeRole, DecorativeScope, MediaCatalogItem } from "../../domain/project";
import { loadedMediaPreviewImage } from "../../application/mediaPreviewImages";
import { IMAGE_DRAG_GHOST_STYLE, imageDragGhostGeometry, imageDragGhostPosition } from "../../ui/imageDragGhostVisual";
import { MediaThumbnail } from "./MediaThumbnail";
import "./MediaDragGhost.css";

/** Where the Canvas would apply the dragged Decorative if it were released now. */
export interface DecorativeDropTarget {
  role: DecorativeRole;
  scope: DecorativeScope;
}

/** Padding and border of the target card; the thumbnail keeps its place when the card appears. */
const TARGET_CARD_PADDING_PX = 4;
const TARGET_CARD_BORDER_PX = 1;

export function MediaDragGhost({ media, previewUrl, missing, x, y, decorativeTarget = null }: {
  media: MediaCatalogItem;
  previewUrl?: string;
  missing: boolean;
  x: number;
  y: number;
  decorativeTarget?: DecorativeDropTarget | null;
}) {
  const [loaded, setLoaded] = useState<{ url: string; width: number; height: number } | null>(null);
  const cardRef = useRef<HTMLDivElement>(null);
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
  const viewport = { width: window.innerWidth, height: window.innerHeight };
  const position = imageDragGhostPosition({ x, y }, geometry, viewport);
  const recordLoadedSize = (event: SyntheticEvent) => {
    const image = event.target;
    if (previewUrl && image instanceof HTMLImageElement && image.getAttribute("src") === previewUrl &&
      image.naturalWidth > 0 && image.naturalHeight > 0) {
      setLoaded({ url: previewUrl, width: image.naturalWidth, height: image.naturalHeight });
    }
  };

  if (decorativeTarget) {
    const pad = TARGET_CARD_PADDING_PX, inset = pad + TARGET_CARD_BORDER_PX;
    // The card size is only known after layout; the previous frame's size keeps it inside the window.
    const card = cardRef.current;
    const size = { width: card?.offsetWidth ?? geometry.width + inset * 2, height: card?.offsetHeight ?? geometry.height + inset * 2 };
    const cardPosition = imageDragGhostPosition({ x: x - inset, y: y - inset }, size, viewport);
    return createPortal(<div ref={cardRef} className="media-drag-ghost media-drag-ghost--target"
      data-media-drag-ghost={media.id}
      onLoadCapture={recordLoadedSize}
      style={{ padding: pad, transform: `translate3d(${cardPosition.x}px, ${cardPosition.y}px, 0)` }}>
      <span aria-hidden="true" className="media-drag-ghost__image" style={{ width: geometry.width, height: geometry.height }}>
        <MediaThumbnail media={media} previewUrl={previewUrl} missing={missing} loading="eager" />
      </span>
      <span className="media-drag-ghost__target" role="status" aria-label="Aplicação do decorativo">
        <span className="media-drag-ghost__role">{decorativeTarget.role === "background" ? "Fundo" : "Sobreposição"}</span>
        <span className="media-drag-ghost__scope">{decorativeTarget.scope === "bothSides" ? "Ambos os lados"
          : decorativeTarget.scope === "left" ? "Lado esquerdo" : "Lado direito"}</span>
      </span>
    </div>, document.body);
  }

  return createPortal(<div aria-hidden="true" className="media-drag-ghost"
    data-media-drag-ghost={media.id}
    onLoadCapture={recordLoadedSize}
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
