import type { MediaPreview } from "./projectPorts";

interface MediaPreviewRenderingCandidate {
  state: "pending" | MediaPreview["state"];
  url?: string | null;
}

export function renderableMediaPreviewUrl(
  preview: MediaPreviewRenderingCandidate,
): string | null {
  if (preview.state === "pending") {
    return null;
  }
  const url = preview.url?.trim();
  return url ? preview.url ?? null : null;
}

export function renderableMediaPreviewUrls(
  previews: Readonly<Record<string, MediaPreview>>,
): Readonly<Record<string, string>> {
  return Object.fromEntries(
    Object.entries(previews).flatMap(([mediaId, preview]) => {
      const url = renderableMediaPreviewUrl(preview);
      return url ? [[mediaId, url]] : [];
    }),
  );
}
