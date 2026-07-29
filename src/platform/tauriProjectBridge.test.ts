import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";

import type { ProjectIntent } from "../domain/project";
import { tauriProjectBridge } from "./tauriProjectBridge";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
}));

beforeEach(() => {
  vi.mocked(invoke).mockClear();
});

test("maps the ProjectBridge interface to the desktop commands", async () => {
  const intent: ProjectIntent = {
    kind: "fillLeftmostPlaceholder",
    sheetId: "sheet-002",
    mediaId: "media-campo",
  };

  await tauriProjectBridge.load("project-load-1");
  await tauriProjectBridge.apply(intent);
  await tauriProjectBridge.undo();
  await tauriProjectBridge.redo();
  vi.mocked(invoke).mockResolvedValueOnce({
    previews: [
      {
        mediaId: "benchmark-a-001",
        url: "http://asset.localhost/cache-preview",
        widthPx: 1200,
        heightPx: 800,
      },
    ],
    generatedCount: 1,
    reusedCount: 0,
    sourceBytes: 24_000_000,
    previewBytes: 240_000,
    elapsedMs: 1200,
  });
  const previews = await tauriProjectBridge.prepareMediaPreviews();
  await tauriProjectBridge.exportPreview();

  expect(invoke).toHaveBeenNthCalledWith(1, "project_state", {
    operationId: "project-load-1",
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "apply_project_intent", {
    intent,
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "undo_project");
  expect(invoke).toHaveBeenNthCalledWith(4, "redo_project");
  expect(invoke).toHaveBeenNthCalledWith(5, "prepare_media_previews");
  expect(invoke).toHaveBeenNthCalledWith(6, "export_spike");
  expect(previews?.[0].url).toBe("http://asset.localhost/cache-preview");
});
