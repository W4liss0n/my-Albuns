import type { ComposedSheet } from "../domain/project";

/** The composition order is stable; repeated uses of one photo appear once. */
export function sheetViewerMediaIds(sheet: ComposedSheet | null): string[] {
  if (!sheet) return [];
  const seen = new Set<string>();
  return sheet.frames.flatMap((frame) => {
    const id = frame.photo?.mediaId;
    if (!id || seen.has(id)) return [];
    seen.add(id);
    return [id];
  });
}

export function adjacentViewerDemand(ids: readonly string[], currentId: string) {
  const index = ids.indexOf(currentId);
  if (index < 0) return { visibleMediaIds: [], preloadMediaIds: [] };
  return {
    visibleMediaIds: [currentId],
    preloadMediaIds: [ids[index - 1], ids[index + 1]].filter((id): id is string => Boolean(id)),
  };
}
