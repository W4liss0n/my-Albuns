import type { MediaPreviewDemand } from "../application/projectPorts";

export const MEDIA_PANEL_PRELOAD_MARGIN = 122;

// The panel has fixed square cards. Measure the current viewport to prepare the
// next catalog without mounting empty cards or decoding the entire catalog.
export function mediaPanelViewportDemand(
  grid: HTMLElement | null,
  orderedIds: readonly string[],
  thumbnailSize: number,
): MediaPreviewDemand {
  const visibleMediaIds: string[] = [];
  const preloadMediaIds: string[] = [];
  if (!grid || grid.clientWidth === 0 || grid.clientHeight === 0) {
    return { visibleMediaIds, preloadMediaIds };
  }
  const style = getComputedStyle(grid);
  const pixels = (value: string) => Number.parseFloat(value) || 0;
  const top = pixels(style.paddingTop);
  const rowGap = pixels(style.rowGap);
  const columnGap = pixels(style.columnGap);
  const width = grid.clientWidth - pixels(style.paddingLeft) - pixels(style.paddingRight);
  const columns = Math.max(1, Math.floor((width + columnGap) / (thumbnailSize + columnGap)));
  const rows = Math.ceil(orderedIds.length / columns);
  const height = top + rows * (thumbnailSize + rowGap) - rowGap + pixels(style.paddingBottom);
  const scrollTop = Math.min(grid.scrollTop, Math.max(0, height - grid.clientHeight));
  const bottom = scrollTop + grid.clientHeight;
  const preloadMargin = Math.max(MEDIA_PANEL_PRELOAD_MARGIN, grid.clientHeight * 3);
  orderedIds.forEach((id, index) => {
    const cardTop = top + Math.floor(index / columns) * (thumbnailSize + rowGap);
    const cardBottom = cardTop + thumbnailSize;
    if (cardBottom > scrollTop && cardTop < bottom) visibleMediaIds.push(id);
    else if (cardBottom > scrollTop - preloadMargin &&
      cardTop < bottom + preloadMargin) preloadMediaIds.push(id);
  });
  return { visibleMediaIds, preloadMediaIds };
}
