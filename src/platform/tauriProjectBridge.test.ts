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
  await tauriProjectBridge.exportPreview();

  expect(invoke).toHaveBeenNthCalledWith(1, "project_state", {
    operationId: "project-load-1",
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "apply_project_intent", {
    intent,
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "undo_project");
  expect(invoke).toHaveBeenNthCalledWith(4, "redo_project");
  expect(invoke).toHaveBeenNthCalledWith(5, "export_spike");
});
