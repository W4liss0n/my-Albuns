import type { ImageProcessingProblem, MediaPreview, MediaPreviewDemand, PhotoImportCompletion } from "./projectPorts";

export type PrepareImportedMedia = (imported: PhotoImportCompletion) => Promise<readonly ImageProcessingProblem[]>;

export function mergeMediaPreviewDemands(...demands: readonly MediaPreviewDemand[]): MediaPreviewDemand {
  const visible = new Set(demands.flatMap((demand) => [...demand.visibleMediaIds]));
  const preload = new Set(demands.flatMap((demand) => [...demand.preloadMediaIds]));
  return {
    visibleMediaIds: [...visible],
    preloadMediaIds: [...preload].filter((id) => !visible.has(id)),
  };
}

export async function decodeMediaPreview(url: string): Promise<void> {
  const image = new Image();
  image.src = url;
  await image.decode();
}

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
