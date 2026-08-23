import { useState, type CSSProperties, type ReactNode } from "react";

import type { MediaCatalogItem } from "../domain/project";
import "./MediaThumbnail.css";

interface MediaPreviewGeometry {
  aspectRatio: string;
  isPortrait: boolean;
}

interface IntrinsicPreviewSize {
  height: number;
  previewUrl: string;
  width: number;
}

interface MediaThumbnailProps {
  "aria-label"?: string;
  children?: ReactNode;
  className?: string;
  loading?: "eager" | "lazy";
  media: Pick<MediaCatalogItem, "sourceHeightPx" | "sourceWidthPx">;
  previewUrl?: string;
}

/**
 * Miniatura visual compartilhada pelo Painel de imagens e pelos seletores de
 * Decorativos. A mídia inteira permanece dentro da moldura e, quando o
 * catálogo não conhece suas dimensões, a imagem carregada fornece a proporção.
 */
export function MediaThumbnail({
  "aria-label": ariaLabel,
  children,
  className,
  loading = "lazy",
  media,
  previewUrl,
}: MediaThumbnailProps) {
  const [intrinsicSize, setIntrinsicSize] =
    useState<IntrinsicPreviewSize | null>(null);
  const geometry = mediaPreviewGeometry(
    media,
    intrinsicSize && intrinsicSize.previewUrl === previewUrl
      ? intrinsicSize
      : undefined,
  );

  return (
    <span
      aria-label={ariaLabel}
      className={["media-preview-thumbnail", className]
        .filter(Boolean)
        .join(" ")}
      data-has-preview={String(Boolean(previewUrl))}
      data-portrait={String(geometry.isPortrait)}
      style={
        {
          "--media-aspect-ratio": geometry.aspectRatio,
        } as CSSProperties
      }
    >
      {previewUrl ? (
        <img
          alt=""
          draggable="false"
          loading={loading}
          src={previewUrl}
          onLoad={(event) => {
            if (hasSourceDimensions(media)) return;
            const { naturalHeight, naturalWidth } = event.currentTarget;
            if (naturalWidth <= 0 || naturalHeight <= 0) return;
            setIntrinsicSize({
              height: naturalHeight,
              previewUrl,
              width: naturalWidth,
            });
          }}
        />
      ) : null}
      {children}
    </span>
  );
}

function mediaPreviewGeometry(
  media: Pick<MediaCatalogItem, "sourceHeightPx" | "sourceWidthPx">,
  intrinsicSize?: IntrinsicPreviewSize,
): MediaPreviewGeometry {
  const width = hasSourceDimensions(media)
    ? media.sourceWidthPx
    : intrinsicSize?.width;
  const height = hasSourceDimensions(media)
    ? media.sourceHeightPx
    : intrinsicSize?.height;
  if (!width || !height) return { aspectRatio: "1 / 1", isPortrait: false };
  return {
    aspectRatio: `${width} / ${height}`,
    isPortrait: height > width,
  };
}

function hasSourceDimensions(
  media: Pick<MediaCatalogItem, "sourceHeightPx" | "sourceWidthPx">,
): media is {
  sourceHeightPx: number;
  sourceWidthPx: number;
} {
  return (
    media.sourceWidthPx !== null &&
    media.sourceHeightPx !== null &&
    media.sourceWidthPx > 0 &&
    media.sourceHeightPx > 0
  );
}
