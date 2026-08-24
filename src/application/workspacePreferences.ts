import type { MediaKind } from "../domain/project";

export const MEDIA_THUMBNAIL_MIN_SIZE = 58;
export const MEDIA_THUMBNAIL_MAX_SIZE = 132;
export const MEDIA_THUMBNAIL_DEFAULT_SIZE = 84;

export type WorkspacePanel = "inspector" | "media";
export type MediaUsageFilter = "all" | "used" | "unused";
export type MediaSortDirection = "ascending" | "descending";

export interface WorkspacePanelPreference {
  size: number;
  visible: boolean;
}

export interface MediaPanelPersistentPreference {
  sortDirection: MediaSortDirection;
  usageFilter: MediaUsageFilter;
}

export interface WorkspacePreferences {
  inspectorSections: Readonly<Record<string, boolean>>;
  mediaPanel: Readonly<Record<MediaKind, MediaPanelPersistentPreference>>;
  mediaThumbnailSizes: Readonly<Record<MediaKind, number>>;
  workspacePanels: Readonly<
    Record<WorkspacePanel, WorkspacePanelPreference | null>
  >;
}

export type WorkspacePreferenceChange =
  | {
      kind: "inspectorSection";
      preferenceKey: string;
      open: boolean;
    }
  | {
      kind: "mediaThumbnailSize";
      mediaKind: MediaKind;
      size: number;
    }
  | {
      kind: "workspacePanelSize";
      panel: WorkspacePanel;
      size: number;
    }
  | {
      kind: "workspacePanelVisibility";
      panel: WorkspacePanel;
      visible: boolean;
    }
  | {
      kind: "mediaPanelSortDirection";
      mediaKind: MediaKind;
      sortDirection: MediaSortDirection;
    }
  | {
      kind: "mediaPanelUsageFilter";
      mediaKind: MediaKind;
      usageFilter: MediaUsageFilter;
    };

export interface WorkspacePreferencesPort {
  load(): Promise<WorkspacePreferences>;
  update(change: WorkspacePreferenceChange): Promise<WorkspacePreferences>;
}

export const WORKSPACE_PANEL_DEFAULTS: Readonly<
  Record<WorkspacePanel, WorkspacePanelPreference>
> = {
  inspector: { size: 310, visible: true },
  media: { size: 202, visible: true },
};

export const WORKSPACE_PANEL_SIZE_LIMITS: Readonly<
  Record<WorkspacePanel, Readonly<{ minimum: number; maximum: number }>>
> = {
  inspector: { minimum: 220, maximum: 480 },
  media: { minimum: 120, maximum: 360 },
};

const MEDIA_PANEL_DEFAULT: MediaPanelPersistentPreference = {
  sortDirection: "ascending",
  usageFilter: "all",
};

export function createWorkspacePreferences(
  overrides: Partial<WorkspacePreferences> = {},
): WorkspacePreferences {
  return {
    inspectorSections: { ...(overrides.inspectorSections ?? {}) },
    mediaPanel: {
      decorative: normalizeMediaPanelPreference(
        overrides.mediaPanel?.decorative,
      ),
      photo: normalizeMediaPanelPreference(overrides.mediaPanel?.photo),
    },
    mediaThumbnailSizes: {
      decorative: normalizeThumbnailSize(
        overrides.mediaThumbnailSizes?.decorative,
      ),
      photo: normalizeThumbnailSize(overrides.mediaThumbnailSizes?.photo),
    },
    workspacePanels: {
      inspector: normalizePanelPreference(
        "inspector",
        overrides.workspacePanels?.inspector,
      ),
      media: normalizePanelPreference(
        "media",
        overrides.workspacePanels?.media,
      ),
    },
  };
}

export function effectiveWorkspacePanelPreference(
  preferences: WorkspacePreferences,
  panel: WorkspacePanel,
): WorkspacePanelPreference {
  return preferences.workspacePanels[panel] ?? WORKSPACE_PANEL_DEFAULTS[panel];
}

export function applyWorkspacePreferenceChange(
  preferences: WorkspacePreferences,
  change: WorkspacePreferenceChange,
): WorkspacePreferences {
  if (change.kind === "inspectorSection") {
    return {
      ...preferences,
      inspectorSections: {
        ...preferences.inspectorSections,
        [change.preferenceKey]: change.open,
      },
    };
  }
  if (change.kind === "mediaThumbnailSize") {
    return {
      ...preferences,
      mediaThumbnailSizes: {
        ...preferences.mediaThumbnailSizes,
        [change.mediaKind]: normalizeThumbnailSize(change.size),
      },
    };
  }
  if (
    change.kind === "workspacePanelSize" ||
    change.kind === "workspacePanelVisibility"
  ) {
    const current =
      preferences.workspacePanels[change.panel] ??
      WORKSPACE_PANEL_DEFAULTS[change.panel];
    return {
      ...preferences,
      workspacePanels: {
        ...preferences.workspacePanels,
        [change.panel]: normalizePanelPreference(change.panel, {
          ...current,
          ...(change.kind === "workspacePanelSize"
            ? { size: change.size }
            : { visible: change.visible }),
        }),
      },
    };
  }
  const current = preferences.mediaPanel[change.mediaKind];
  return {
    ...preferences,
    mediaPanel: {
      ...preferences.mediaPanel,
      [change.mediaKind]: normalizeMediaPanelPreference({
        ...current,
        ...(change.kind === "mediaPanelSortDirection"
          ? { sortDirection: change.sortDirection }
          : { usageFilter: change.usageFilter }),
      }),
    },
  };
}

export function createFallbackWorkspacePreferencesPort(): WorkspacePreferencesPort {
  let preferences = createWorkspacePreferences();
  return {
    load: async () => createWorkspacePreferences(preferences),
    update: async (change) => {
      preferences = applyWorkspacePreferenceChange(preferences, change);
      return createWorkspacePreferences(preferences);
    },
  };
}

function normalizeThumbnailSize(value: number | undefined) {
  if (!Number.isFinite(value)) return MEDIA_THUMBNAIL_DEFAULT_SIZE;
  return Math.min(
    MEDIA_THUMBNAIL_MAX_SIZE,
    Math.max(MEDIA_THUMBNAIL_MIN_SIZE, Math.round(value as number)),
  );
}

function normalizePanelPreference(
  panel: WorkspacePanel,
  value: WorkspacePanelPreference | null | undefined,
): WorkspacePanelPreference | null {
  if (value === null || value === undefined) return null;
  const limits = WORKSPACE_PANEL_SIZE_LIMITS[panel];
  const candidate = Number.isFinite(value.size)
    ? Math.round(value.size)
    : WORKSPACE_PANEL_DEFAULTS[panel].size;
  return {
    size: Math.min(limits.maximum, Math.max(limits.minimum, candidate)),
    visible: value.visible !== false,
  };
}

function normalizeMediaPanelPreference(
  value: Partial<MediaPanelPersistentPreference> | undefined,
): MediaPanelPersistentPreference {
  return {
    sortDirection:
      value?.sortDirection === "descending" ? "descending" : "ascending",
    usageFilter:
      value?.usageFilter === "used" || value?.usageFilter === "unused"
        ? value.usageFilter
        : MEDIA_PANEL_DEFAULT.usageFilter,
  };
}
