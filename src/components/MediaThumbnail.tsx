import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";

import type { MediaCatalogItem } from "../domain/project";
import { registerMediaPreviewImage } from "../application/mediaPreviewImages";
import "./MediaThumbnail.css";

interface MediaPreviewGeometry {
  aspectRatio: string;
  isPortrait: boolean;
}

interface MediaThumbnailProps {
  "aria-label"?: string;
  children?: ReactNode;
  className?: string;
  loading?: "eager" | "lazy";
  media: Pick<MediaCatalogItem, "sourceHeightPx" | "sourceWidthPx">;
  previewUrl?: string;
  missing?: boolean;
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
  missing = false,
}: MediaThumbnailProps) {
  const [loaded, setLoaded] = useState<{
    url: string;
    width: number;
    height: number;
    geometry: MediaPreviewGeometry;
  } | null>(null);
  useEffect(() => {
    if (!previewUrl) setLoaded(null);
  }, [previewUrl]);
  const retained = previewUrl && loaded && loaded.url !== previewUrl ? loaded : null;
  const geometry = retained?.geometry ?? mediaPreviewGeometry(
    media,
    loaded && loaded.url === previewUrl ? loaded : undefined,
  );

  return (
    <span
      aria-label={ariaLabel}
      className={["media-preview-thumbnail", className]
        .filter(Boolean)
        .join(" ")}
      data-has-preview={String(Boolean(previewUrl))}
      data-missing={String(missing && !previewUrl)}
      data-portrait={String(geometry.isPortrait)}
      style={
        {
          "--media-aspect-ratio": geometry.aspectRatio,
        } as CSSProperties
      }
    >
      {previewUrl ? [retained?.url, previewUrl].filter((url): url is string => Boolean(url)).map((url) => (
        <ThumbnailImage key={url} url={url} loading={retained ? "eager" : loading}
          pending={Boolean(retained && url === previewUrl)}
          onLoad={url === previewUrl ? (image) => {
            const size = { width: image.naturalWidth, height: image.naturalHeight };
            setLoaded({ url, ...size, geometry: mediaPreviewGeometry(media, size) });
          } : undefined} />
      )) : missing ? (
        <span aria-hidden="true" className="media-preview-thumbnail__missing-symbol" />
      ) : null}
      {children}
    </span>
  );
}

function ThumbnailImage({ url, loading, pending, onLoad }: {
  url: string;
  loading: "eager" | "lazy";
  pending: boolean;
  onLoad?: (image: HTMLImageElement) => void;
}) {
  const imageRef = useRef<HTMLImageElement>(null);
  useEffect(() => {
    if (imageRef.current) return registerMediaPreviewImage(url, imageRef.current);
  }, [url]);
  return <img ref={imageRef} alt="" crossOrigin="anonymous" draggable="false"
    loading={loading} src={url} data-pending={pending || undefined}
    onLoad={(event) => onLoad?.(event.currentTarget)} />;
}

function mediaPreviewGeometry(
  media: Pick<MediaCatalogItem, "sourceHeightPx" | "sourceWidthPx">,
  intrinsicSize?: { width: number; height: number },
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
