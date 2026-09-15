import { invoke } from "@tauri-apps/api/core";
import { expect, test, vi } from "vitest";
import { tauriProjectLauncher } from "./tauriProjectLauncher";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

test("delegates each editor action without replacing, saving, or closing the current session", async () => {
  await tauriProjectLauncher.newProject();
  await tauriProjectLauncher.openProject();
  expect(vi.mocked(invoke).mock.calls).toEqual([["new_project_from_editor"], ["open_project_from_editor"]]);
});
