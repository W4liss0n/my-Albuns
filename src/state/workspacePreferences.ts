type WorkspacePanelPreference = "inspector" | "media";

const PANEL_SIZE_KEYS: Record<WorkspacePanelPreference, string> = {
  inspector: "myalbuns.workspace.inspector-width",
  media: "myalbuns.workspace.media-panel-height",
};

export function readInspectorSectionPreference(
  preferenceKey: string,
  fallback: boolean,
) {
  const stored = readPreference(
    `myalbuns.inspector.${preferenceKey}`,
  );
  return stored === null ? fallback : stored === "open";
}

export function writeInspectorSectionPreference(
  preferenceKey: string,
  open: boolean,
) {
  writePreference(
    `myalbuns.inspector.${preferenceKey}`,
    open ? "open" : "closed",
  );
}

export function readWorkspacePanelSize(
  panel: WorkspacePanelPreference,
  fallback: number,
) {
  const stored = Number(readPreference(PANEL_SIZE_KEYS[panel]));
  return Number.isFinite(stored) && stored > 0 ? stored : fallback;
}

export function writeWorkspacePanelSize(
  panel: WorkspacePanelPreference,
  value: number,
) {
  writePreference(PANEL_SIZE_KEYS[panel], String(Math.round(value)));
}

function readPreference(key: string) {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writePreference(key: string, value: string) {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // The in-memory preference remains usable when storage is unavailable.
  }
}
