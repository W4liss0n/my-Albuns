import type { MediaPreview } from "./projectPorts";

export function renderableMediaPreviewUrls(
  previews: Readonly<Record<string, MediaPreview>>,
): Readonly<Record<string, string>> {
  return Object.fromEntries(
    Object.entries(previews).flatMap(([mediaId, preview]) => {
      if (
        (preview.state !== "ready" && preview.state !== "unavailable") ||
        !preview.url?.trim()
      ) {
        return [];
      }
      return [[mediaId, preview.url]];
    }),
  );
}
