export type MediaUsageFilter = "all" | "used" | "unused";
export type MediaSortDirection = "ascending" | "descending";

export interface MediaPanelViewPreferences {
  sortDirection: MediaSortDirection;
  thumbnailSize: number;
  usageFilter: MediaUsageFilter;
}

export const MEDIA_THUMBNAIL_MIN_SIZE = 58;
export const MEDIA_THUMBNAIL_MAX_SIZE = 132;
export const MEDIA_THUMBNAIL_DEFAULT_SIZE = 84;

/**
 * PLACEHOLDER INTEGRATION: Ordenação/Filtro must be hydrated through the
 * SettingsStore and Tamanho through the StateStore when those application
 * ports reach the Project Window. WebView storage is intentionally not used
 * because its profile is isolated per Project.
 */
export function createMediaPanelViewPreferences(): MediaPanelViewPreferences {
  return {
    sortDirection: "ascending",
    thumbnailSize: MEDIA_THUMBNAIL_DEFAULT_SIZE,
    usageFilter: "all",
  };
}
