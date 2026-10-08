import type { MediaCatalogItem } from "../../domain/project";
import { MediaThumbnail } from "../media-panel/MediaThumbnail";
import "./InspectorImagePreview.css";

/**
 * The whole image of the contextual panel's subject, as the media panel shows
 * it: the Cache preview without crop, rotation, mirror or effects. It sits
 * below the context heading, fixed and without a section of its own.
 */
export function InspectorImagePreview({
  media,
  previewUrl,
  missing,
}: {
  media: Pick<MediaCatalogItem, "id" | "kind" | "name" | "sourceHeightPx" | "sourceWidthPx">;
  previewUrl?: string;
  /** Absent with no retained preview: shows the media panel's missing symbol. */
  missing: boolean;
}) {
  const dimensions = knownPhotoDimensions(media);
  const absent = missing && !previewUrl;
  return (
    <div className="inspector-image-preview" data-media-id={media.id}>
      <div
        aria-label={absent ? "Arquivo ausente" : `Prévia de ${media.name}`}
        className="inspector-image-preview__frame"
        role="img"
      >
        {/* A new image never shows the previous one while its preview loads. */}
        <MediaThumbnail
          key={media.id}
          loading="eager"
          media={{
            sourceWidthPx: dimensions?.width ?? null,
            sourceHeightPx: dimensions?.height ?? null,
          }}
          missing={absent}
          previewUrl={previewUrl}
        />
      </div>
    </div>
  );
}

/**
 * Pixel size of a Photo's original, which gives the box its proportion before
 * the preview loads. Decoratives have none, and the Core projects 1 × 1 until
 * a Photo's metadata has been observed.
 */
function knownPhotoDimensions(
  media: Pick<MediaCatalogItem, "kind" | "sourceHeightPx" | "sourceWidthPx">,
): { width: number; height: number } | null {
  if (media.kind !== "photo") return null;
  const { sourceWidthPx: width, sourceHeightPx: height } = media;
  return width !== null && height !== null && width > 1 && height > 1
    ? { width, height }
    : null;
}
