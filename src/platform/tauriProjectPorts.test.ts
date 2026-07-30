import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";

import type { ProjectIntent } from "../domain/project";
import {
  tauriExportPort,
  tauriMediaPreviewPort,
  tauriProjectSessionPort,
} from "./tauriProjectPorts";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
}));

beforeEach(() => {
  vi.mocked(invoke).mockClear();
});

test("maps the Project ports to the desktop commands", async () => {
  const intent: ProjectIntent = {
    kind: "fillLeftmostPlaceholder",
    sheetId: "sheet-002",
    mediaId: "media-campo",
  };

  await tauriProjectSessionPort.load("project-load-1");
  await tauriProjectSessionPort.apply(intent);
  await tauriProjectSessionPort.undo();
  await tauriProjectSessionPort.redo();
  vi.mocked(invoke).mockResolvedValueOnce([
    {
      mediaId: "benchmark-a-001",
      url: "http://asset.localhost/cache-preview",
    },
  ]);
  const previews = await tauriMediaPreviewPort.prepareMediaPreviews();
  await tauriExportPort.exportPreview();

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
