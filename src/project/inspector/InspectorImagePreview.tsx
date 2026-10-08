import { useEffect, useRef, useState } from "react";

import type { MediaCatalogItem } from "../../domain/project";
import { MediaThumbnail } from "../media-panel/MediaThumbnail";
import "./InspectorImagePreview.css";

/**
 * Longest time the previous image stays under a new name while that name's
 * image loads and decodes. Counted from each switch: after a run of quick
 * switches, the last one gets the whole wait, and the images in between are
 * never shown once a later one was chosen.
 */
export const INSPECTOR_PREVIEW_SWAP_WAIT_MS = 250;

type PreviewedMedia = Pick<MediaCatalogItem, "id" | "kind" | "name" | "sourceHeightPx" | "sourceWidthPx">;

interface PreviewSubject {
  media: PreviewedMedia;
  previewUrl?: string;
  /** Absent with no retained preview: shows the media panel's missing symbol. */
  missing: boolean;
}

/**
 * The whole image of the contextual panel's subject, as the media panel shows
 * it: the Cache preview without crop, rotation, mirror or effects. It sits
 * below the context heading, fixed and without a section of its own.
 *
 * Switching to another image keeps the current one on screen until the next
 * preview is loaded and decoded, then swaps image and proportion together, so
 * the box never shows an empty frame between two photos.
 */
export function InspectorImagePreview(subject: PreviewSubject) {
  const { shown, decodedSize } = useDecodedPreviewSwap(subject);
  const { media, previewUrl, missing } = shown;
  const dimensions = knownPhotoDimensions(media) ?? decodedSize;
  const absent = missing && !previewUrl;
  return (
    <div className="inspector-image-preview" data-media-id={media.id}>
      <div
        aria-label={absent ? "Arquivo ausente" : `Prévia de ${media.name}`}
        className="inspector-image-preview__frame"
        role="img"
      >
        {/* Remounted per image: a new image never inherits the previous one's
            loaded proportion. */}
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

interface PreviewSwapHold {
  /** What stays on screen while the target decodes. */
  subject: PreviewSubject;
  /** `performance.now()` after which the target replaces it, decoded or not. */
  deadline: number;
}

interface DecodedPreview {
  url: string;
  width: number;
  height: number;
}

/**
 * Chooses what the preview shows: the target subject, or the previous one
 * while the target's preview loads and decodes off screen.
 *
 * The off-screen image uses the same CORS mode as MediaThumbnail's `<img>`,
 * and stays referenced after the swap, so the `<img>` rendered for the target
 * reuses the resource and its decoded data instead of fetching it again.
 */
function useDecodedPreviewSwap(target: PreviewSubject) {
  const [previousTarget, setPreviousTarget] = useState(target);
  const [hold, setHold] = useState<PreviewSwapHold | null>(null);
  const [decoded, setDecoded] = useState<DecodedPreview | null>(null);
  const preloadedImage = useRef<HTMLImageElement | null>(null);

  if (!samePreviewSubject(target, previousTarget)) {
    setPreviousTarget(target);
    if (
      target.media.id !== previousTarget.media.id ||
      target.previewUrl !== previousTarget.previewUrl
    ) {
      const onScreen = hold?.subject ?? previousTarget;
      // The same image (a newer preview of it) and an image without a preview
      // to wait for replace the shown one at once. Anything else waits, a
      // missing file with a retained preview included, and whatever is on
      // screen stays: a photo, or the stripes or missing symbol, which would
      // otherwise give way to an empty white box before the photo.
      const wait = onScreen.media.id !== target.media.id && Boolean(target.previewUrl);
      setHold(wait
        ? { subject: onScreen, deadline: performance.now() + INSPECTOR_PREVIEW_SWAP_WAIT_MS }
        : null);
    }
  }

  const awaitedUrl = hold ? target.previewUrl : undefined;
  const deadline = hold?.deadline;
  useEffect(() => {
    if (!awaitedUrl || deadline === undefined) return;
    let finished = false;
    const image = new Image();
    image.crossOrigin = "anonymous";
    image.src = awaitedUrl;
    const swap = (decodedImage: boolean) => {
      if (finished) return;
      finished = true;
      window.clearTimeout(timer);
      // Kept after a late swap too: the visible <img> joins this request.
      preloadedImage.current = image;
      setDecoded(decodedImage && image.naturalWidth > 0 && image.naturalHeight > 0
        ? { url: awaitedUrl, width: image.naturalWidth, height: image.naturalHeight }
        : null);
      setHold(null);
    };
    const timer = window.setTimeout(() => swap(false), Math.max(0, deadline - performance.now()));
    decodeImage(image).then(() => swap(true), () => swap(false));
    return () => {
      finished = true;
      window.clearTimeout(timer);
    };
  }, [awaitedUrl, deadline]);

  const shown = hold?.subject ?? target;
  const decodedSize = decoded && decoded.url === shown.previewUrl
    ? { width: decoded.width, height: decoded.height }
    : null;
  return { shown, decodedSize };
}

function samePreviewSubject(first: PreviewSubject, second: PreviewSubject) {
  return first.media.id === second.media.id &&
    first.media.kind === second.media.kind &&
    first.media.name === second.media.name &&
    first.media.sourceWidthPx === second.media.sourceWidthPx &&
    first.media.sourceHeightPx === second.media.sourceHeightPx &&
    first.previewUrl === second.previewUrl &&
    first.missing === second.missing;
}

function decodeImage(image: HTMLImageElement): Promise<void> {
  if (typeof image.decode === "function") return image.decode();
  return new Promise((resolve, reject) => {
    image.addEventListener("load", () => resolve(), { once: true });
    image.addEventListener("error", () => reject(new Error("preview did not load")), { once: true });
  });
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
