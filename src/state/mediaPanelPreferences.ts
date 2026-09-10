import {
  MEDIA_THUMBNAIL_DEFAULT_SIZE,
  type MediaSortDirection,
  type MediaSortKey,
  type MediaUsageFilter,
} from "../application/workspacePreferences";

export type { MediaSortDirection, MediaUsageFilter } from "../application/workspacePreferences";

export interface MediaPanelViewPreferences {
  sortKey: MediaSortKey;
  sortDirection: MediaSortDirection;
  thumbnailSize: number;
  usageFilter: MediaUsageFilter;
}

export {
  MEDIA_THUMBNAIL_DEFAULT_SIZE,
  MEDIA_THUMBNAIL_MAX_SIZE,
  MEDIA_THUMBNAIL_MIN_SIZE,
} from "../application/workspacePreferences";

export function createMediaPanelViewPreferences(): MediaPanelViewPreferences {
  return {
    sortKey: "name",
    sortDirection: "ascending",
    thumbnailSize: MEDIA_THUMBNAIL_DEFAULT_SIZE,
    usageFilter: "all",
  };
}
