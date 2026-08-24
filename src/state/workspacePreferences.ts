import {
  WORKSPACE_PANEL_DEFAULTS,
  type WorkspacePanel,
} from "../application/workspacePreferences";

const PANEL_SIZE_KEYS: Record<WorkspacePanel, string> = {
  inspector: "myalbuns.workspace.inspector-width",
  media: "myalbuns.workspace.media-panel-height",
};

const LEGACY_INSPECTOR_PREFIX = "myalbuns.inspector.";

export function readLegacyInspectorSectionPreferences() {
  const preferences: Record<string, boolean> = {};
  try {
    for (let index = 0; index < window.localStorage.length; index += 1) {
      const storageKey = window.localStorage.key(index);
      if (!storageKey?.startsWith(LEGACY_INSPECTOR_PREFIX)) continue;
      const preferenceKey = storageKey.slice(LEGACY_INSPECTOR_PREFIX.length);
      if (!isSafePreferenceKey(preferenceKey)) continue;
      const value = window.localStorage.getItem(storageKey);
      if (value === "open" || value === "closed") {
        preferences[preferenceKey] = value === "open";
      }
    }
  } catch {
    // A Project keeps defaults when its isolated legacy profile is unavailable.
  }
  return preferences;
}

export function clearLegacyInspectorSectionPreference(preferenceKey: string) {
  try {
    window.localStorage.removeItem(`${LEGACY_INSPECTOR_PREFIX}${preferenceKey}`);
  } catch {
    // The successful StateStore write remains authoritative.
  }
}

export function readLegacyWorkspacePanelSizes() {
  const sizes: Partial<Record<WorkspacePanel, number>> = {};
  for (const panel of ["inspector", "media"] as const) {
    const storedSize = readPreference(PANEL_SIZE_KEYS[panel]);
    if (storedSize === null) continue;

    const candidate = Number(storedSize);
    sizes[panel] =
      Number.isFinite(candidate) && candidate > 0
        ? candidate
        : WORKSPACE_PANEL_DEFAULTS[panel].size;
  }
  return sizes;
}

export function clearLegacyWorkspacePanelPreference(panel: WorkspacePanel) {
  try {
    window.localStorage.removeItem(PANEL_SIZE_KEYS[panel]);
  } catch {
    // The successful StateStore write remains authoritative.
  }
}

function readPreference(key: string) {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function isSafePreferenceKey(value: string) {
  return (
    value.length > 0 &&
    value.length <= 128 &&
    /^[a-z0-9._-]+$/i.test(value)
  );
}
