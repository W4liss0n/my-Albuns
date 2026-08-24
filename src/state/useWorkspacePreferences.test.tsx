import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import {
  applyWorkspacePreferenceChange,
  createFallbackWorkspacePreferencesPort,
  createWorkspacePreferences,
  type WorkspacePreferencesPort,
} from "../application/workspacePreferences";
import { useWorkspacePreferences } from "./useWorkspacePreferences";

beforeEach(() => localStorage.clear());

test("keeps the non-desktop fallback authoritative for the lifetime of its workspace", async () => {
  const port = createFallbackWorkspacePreferencesPort();

  await port.update({
    kind: "mediaThumbnailSize",
    mediaKind: "decorative",
    size: 110,
  });
  await port.update({
    kind: "inspectorSection",
    preferenceKey: "album.design",
    open: false,
  });
  await port.update({
    kind: "workspacePanelSize",
    panel: "media",
    size: 240,
  });
  await port.update({
    kind: "workspacePanelVisibility",
    panel: "media",
    visible: false,
  });
  await port.update({
    kind: "mediaPanelSortDirection",
    mediaKind: "photo",
    sortDirection: "descending",
  });
  await port.update({
    kind: "mediaPanelUsageFilter",
    mediaKind: "photo",
    usageFilter: "used",
  });

  await expect(port.load()).resolves.toEqual(
    createWorkspacePreferences({
      inspectorSections: { "album.design": false },
      mediaPanel: {
        decorative: { sortDirection: "ascending", usageFilter: "all" },
        photo: { sortDirection: "descending", usageFilter: "used" },
      },
      mediaThumbnailSizes: { decorative: 110, photo: 84 },
      workspacePanels: {
        inspector: null,
        media: { size: 240, visible: false },
      },
    }),
  );
});

test("hydrates shared UI state and refreshes it when another Project window gains focus", async () => {
  const initial = createWorkspacePreferences({
    inspectorSections: { "album.design": false },
    mediaThumbnailSizes: { decorative: 110, photo: 124 },
  });
  const refreshed = createWorkspacePreferences({
    inspectorSections: { "album.design": true },
    mediaThumbnailSizes: { decorative: 96, photo: 118 },
  });
  const port: WorkspacePreferencesPort = {
    load: vi
      .fn<WorkspacePreferencesPort["load"]>()
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce(refreshed),
    update: vi.fn(),
  };

  const { result } = renderHook(() => useWorkspacePreferences(port));
  expect(result.current.ready).toBe(false);
  await waitFor(() => expect(result.current.preferences).toEqual(initial));
  expect(result.current.ready).toBe(true);

  act(() => window.dispatchEvent(new Event("focus")));
  await waitFor(() => expect(result.current.preferences).toEqual(refreshed));
  expect(port.load).toHaveBeenCalledTimes(2);
});

test("applies changes immediately while the StateStore publishes them", async () => {
  let persisted = createWorkspacePreferences();
  const update = vi.fn<WorkspacePreferencesPort["update"]>(async (change) => {
    persisted = applyWorkspacePreferenceChange(persisted, change);
    return persisted;
  });
  const load = vi.fn(async () => persisted);
  const port: WorkspacePreferencesPort = {
    load,
    update,
  };
  const { result } = renderHook(() => useWorkspacePreferences(port));
  await waitFor(() => expect(load).toHaveBeenCalledTimes(1));

  act(() => {
    result.current.update({
      kind: "mediaThumbnailSize",
      mediaKind: "photo",
      size: 124,
    });
  });

  expect(result.current.preferences.mediaThumbnailSizes.photo).toBe(124);
  await waitFor(() => expect(update).toHaveBeenCalledOnce());
});

test("restores the last confirmed preferences when publishing a change fails", async () => {
  const confirmed = createWorkspacePreferences({
    mediaThumbnailSizes: { decorative: 84, photo: 110 },
  });
  const port: WorkspacePreferencesPort = {
    load: async () => confirmed,
    update: vi.fn(async () => {
      throw new Error("SettingsStore unavailable");
    }),
  };
  const { result } = renderHook(() => useWorkspacePreferences(port));
  await waitFor(() =>
    expect(result.current.preferences.mediaThumbnailSizes.photo).toBe(110),
  );

  act(() => {
    result.current.update({
      kind: "mediaThumbnailSize",
      mediaKind: "photo",
      size: 124,
    });
  });
  expect(result.current.preferences.mediaThumbnailSizes.photo).toBe(124);

  await waitFor(() =>
    expect(result.current.preferences.mediaThumbnailSizes.photo).toBe(110),
  );
});

test("keeps a local update when an older focus refresh completes later", async () => {
  let finishRefresh: (
    value: ReturnType<typeof createWorkspacePreferences>,
  ) => void = () => undefined;
  const staleRefresh = new Promise<
    ReturnType<typeof createWorkspacePreferences>
  >((resolve) => {
    finishRefresh = resolve;
  });
  let persisted = createWorkspacePreferences();
  const port: WorkspacePreferencesPort = {
    load: vi
      .fn<WorkspacePreferencesPort["load"]>()
      .mockResolvedValueOnce(persisted)
      .mockReturnValueOnce(staleRefresh),
    update: vi.fn(async (change) => {
      persisted = applyWorkspacePreferenceChange(persisted, change);
      return persisted;
    }),
  };
  const { result } = renderHook(() => useWorkspacePreferences(port));
  await waitFor(() => expect(port.load).toHaveBeenCalledOnce());

  act(() => window.dispatchEvent(new Event("focus")));
  await waitFor(() => expect(port.load).toHaveBeenCalledTimes(2));
  act(() => {
    result.current.update({
      kind: "mediaThumbnailSize",
      mediaKind: "photo",
      size: 124,
    });
  });
  act(() => finishRefresh(createWorkspacePreferences()));

  await waitFor(() => expect(port.update).toHaveBeenCalledOnce());
  expect(result.current.preferences.mediaThumbnailSizes.photo).toBe(124);
});

test("accepts the latest authoritative response including another host's fields", async () => {
  const port: WorkspacePreferencesPort = {
    load: async () => createWorkspacePreferences(),
    update: vi.fn(async (change) =>
      applyWorkspacePreferenceChange(
        createWorkspacePreferences({
          inspectorSections: { "sheet.design": false },
        }),
        change,
      ),
    ),
  };
  const { result } = renderHook(() => useWorkspacePreferences(port));

  act(() => {
    result.current.update({
      kind: "mediaThumbnailSize",
      mediaKind: "photo",
      size: 124,
    });
  });

  await waitFor(() =>
    expect(result.current.preferences).toEqual(
      createWorkspacePreferences({
        inspectorSections: { "sheet.design": false },
        mediaThumbnailSizes: { decorative: 84, photo: 124 },
      }),
    ),
  );
});

test("migrates legacy per-WebView Inspector state once and retains it on write failure", async () => {
  localStorage.setItem("myalbuns.inspector.album.design", "closed");
  const persistedPort: WorkspacePreferencesPort = {
    load: async () => createWorkspacePreferences(),
    update: vi.fn(async (change) =>
      applyWorkspacePreferenceChange(createWorkspacePreferences(), change),
    ),
  };

  const migrated = renderHook(() => useWorkspacePreferences(persistedPort));
  await waitFor(() =>
    expect(migrated.result.current.preferences.inspectorSections).toEqual({
      "album.design": false,
    }),
  );
  await waitFor(() => expect(persistedPort.update).toHaveBeenCalledOnce());
  expect(localStorage.getItem("myalbuns.inspector.album.design")).toBeNull();
  migrated.unmount();

  localStorage.setItem("myalbuns.inspector.sheet.design", "open");
  const unavailablePort: WorkspacePreferencesPort = {
    load: async () => createWorkspacePreferences(),
    update: vi.fn(async () => {
      throw new Error("State unavailable");
    }),
  };
  const retained = renderHook(() => useWorkspacePreferences(unavailablePort));
  await waitFor(() =>
    expect(retained.result.current.preferences.inspectorSections).toEqual({
      "sheet.design": true,
    }),
  );
  expect(localStorage.getItem("myalbuns.inspector.sheet.design")).toBe("open");
});

test("migrates legacy panel geometry only when the StateStore has no authoritative value", async () => {
  localStorage.setItem("myalbuns.workspace.inspector-width", "350");
  localStorage.setItem("myalbuns.workspace.media-panel-height", "200");
  let persisted = createWorkspacePreferences();
  const update = vi.fn<WorkspacePreferencesPort["update"]>(async (change) => {
    persisted = applyWorkspacePreferenceChange(persisted, change);
    return persisted;
  });
  const migrated = renderHook(() =>
    useWorkspacePreferences({ load: async () => persisted, update }),
  );

  await waitFor(() =>
    expect(migrated.result.current.preferences.workspacePanels).toEqual({
      inspector: { size: 350, visible: true },
      media: { size: 200, visible: true },
    }),
  );
  await waitFor(() => expect(update).toHaveBeenCalledTimes(2));
  expect(localStorage.getItem("myalbuns.workspace.inspector-width")).toBeNull();
  expect(
    localStorage.getItem("myalbuns.workspace.media-panel-height"),
  ).toBeNull();
  migrated.unmount();

  localStorage.setItem("myalbuns.workspace.inspector-width", "480");
  localStorage.setItem("myalbuns.inspector.album.design", "closed");
  const authoritative = createWorkspacePreferences({
    inspectorSections: { "album.design": true },
    workspacePanels: {
      inspector: { size: 320, visible: true },
      media: null,
    },
  });
  const preserved = renderHook(() =>
    useWorkspacePreferences({
      load: async () => authoritative,
      update: vi.fn(),
    }),
  );
  await waitFor(() =>
    expect(preserved.result.current.preferences.workspacePanels.inspector).toEqual(
      { size: 320, visible: true },
    ),
  );
  expect(localStorage.getItem("myalbuns.workspace.inspector-width")).toBeNull();
  expect(localStorage.getItem("myalbuns.inspector.album.design")).toBeNull();
});
