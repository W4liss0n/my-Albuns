import { useEffect, useMemo, useRef } from "react";

import { registerMediaPreviewImage } from "../../application/mediaPreviewImages";
import type { ComposedSheet } from "../../domain/project";

/**
 * Keeps the Decoratives the Album uses loaded while the Canvas is open. A far
 * Sheet reached from the Grade borrows these images, as it borrows the panel's
 * Photo thumbnails, and draws its Background and Overlay on arrival instead of
 * loading them afterwards. Decoratives are few and reused; Photos are not.
 */
export function DecorativePreviewImages({
  sheets,
  mediaPreviewUrls,
}: {
  sheets: readonly ComposedSheet[];
  mediaPreviewUrls: Readonly<Record<string, string>>;
}) {
  const urls = useMemo(
    () => decorativePreviewUrls(sheets, mediaPreviewUrls),
    [sheets, mediaPreviewUrls],
  );
  if (urls.length === 0) return null;
  return (
    <div aria-hidden="true" hidden>
      {urls.map((url) => (
        <DecorativePreviewImage key={url} url={url} />
      ))}
    </div>
  );
}

function DecorativePreviewImage({ url }: { url: string }) {
  const imageRef = useRef<HTMLImageElement>(null);
  useEffect(() => {
    if (imageRef.current) return registerMediaPreviewImage(url, imageRef.current);
  }, [url]);
  return <img ref={imageRef} alt="" crossOrigin="anonymous" src={url} />;
}

function decorativePreviewUrls(
  sheets: readonly ComposedSheet[],
  mediaPreviewUrls: Readonly<Record<string, string>>,
) {
  const urls = new Set<string>();
  for (const sheet of sheets) {
    const mediaIds = [
      ...sheet.backgrounds.flatMap((background) =>
        background.kind === "media" ? [background.mediaId] : [],
      ),
      ...sheet.overlays.map((overlay) => overlay.mediaId),
    ];
    for (const mediaId of mediaIds) {
      const url = mediaPreviewUrls[mediaId];
      if (url) urls.add(url);
    }
  }
  return [...urls];
}
