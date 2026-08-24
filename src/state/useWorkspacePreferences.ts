import { useCallback, useEffect, useRef, useState } from "react";

import {
  applyWorkspacePreferenceChange,
  createWorkspacePreferences,
  type WorkspacePreferenceChange,
  type WorkspacePreferences,
  type WorkspacePreferencesPort,
} from "../application/workspacePreferences";
import {
  clearLegacyInspectorSectionPreference,
  clearLegacyWorkspacePanelPreference,
  readLegacyInspectorSectionPreferences,
  readLegacyWorkspacePanelSizes,
} from "./workspacePreferences";

export function useWorkspacePreferences(port: WorkspacePreferencesPort) {
  const [preferences, setPreferences] = useState<WorkspacePreferences>(() =>
    createWorkspacePreferences(),
  );
  const [ready, setReady] = useState(false);
  const confirmedPreferences = useRef(preferences);
  const updateQueue = useRef<Promise<unknown>>(Promise.resolve());
  const updateRevision = useRef(0);
  const refreshSequence = useRef(0);

  const refresh = useCallback(async () => {
    const sequence = ++refreshSequence.current;
    const revision = updateRevision.current;
    await updateQueue.current.catch(() => undefined);
    if (
      sequence !== refreshSequence.current ||
      revision !== updateRevision.current
    ) {
      return;
    }
    let loaded: WorkspacePreferences;
    try {
      loaded = createWorkspacePreferences(await port.load());
    } catch {
      loaded = confirmedPreferences.current;
    }
    if (
      sequence !== refreshSequence.current ||
      revision !== updateRevision.current
    ) {
      return;
    }

    const legacy = readLegacyInspectorSectionPreferences();
    let hydrated = loaded;
    const migrations: Array<{
      change: WorkspacePreferenceChange;
      clear(): void;
    }> = [];
    for (const [preferenceKey, open] of Object.entries(legacy)) {
      if (loaded.inspectorSections[preferenceKey] !== undefined) {
        clearLegacyInspectorSectionPreference(preferenceKey);
        continue;
      }
      const change = {
        kind: "inspectorSection",
        preferenceKey,
        open,
      } satisfies WorkspacePreferenceChange;
      hydrated = applyWorkspacePreferenceChange(hydrated, change);
      migrations.push({
        change,
        clear: () => clearLegacyInspectorSectionPreference(preferenceKey),
      });
    }
    const legacyPanelSizes = readLegacyWorkspacePanelSizes();
    for (const panel of ["inspector", "media"] as const) {
      const size = legacyPanelSizes[panel];
      if (size === undefined) continue;
      if (loaded.workspacePanels[panel] !== null) {
        clearLegacyWorkspacePanelPreference(panel);
        continue;
      }
      const change = {
        kind: "workspacePanelSize",
        panel,
        size,
      } satisfies WorkspacePreferenceChange;
      hydrated = applyWorkspacePreferenceChange(hydrated, change);
      migrations.push({
        change,
        clear: () => clearLegacyWorkspacePanelPreference(panel),
      });
    }
    confirmedPreferences.current = hydrated;
    setPreferences(hydrated);
    setReady(true);

    for (const migration of migrations) {
      updateQueue.current = updateQueue.current
        .catch(() => undefined)
        .then(() => port.update(migration.change))
        .then(() => migration.clear())
        .catch(() => undefined);
    }
  }, [port]);

  useEffect(() => {
    setReady(false);
    void refresh();
    window.addEventListener("focus", refresh);
    return () => {
      refreshSequence.current += 1;
      window.removeEventListener("focus", refresh);
    };
  }, [refresh]);

  const update = useCallback(
    (change: WorkspacePreferenceChange) => {
      const revision = ++updateRevision.current;
      setPreferences((current) =>
        applyWorkspacePreferenceChange(current, change),
      );
      updateQueue.current = updateQueue.current
        .catch(() => undefined)
        .then(() => port.update(change))
        .then((authoritative) => {
          const confirmed = createWorkspacePreferences(authoritative);
          confirmedPreferences.current = confirmed;
          if (revision === updateRevision.current) {
            setPreferences(confirmed);
          }
        })
        .catch(() => {
          if (revision === updateRevision.current) {
            setPreferences(confirmedPreferences.current);
          }
        });
    },
    [port],
  );

  return { preferences, ready, update };
}
