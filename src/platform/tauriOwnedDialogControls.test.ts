import { expect, test, vi } from "vitest";

const coreApi = vi.hoisted(() => ({
  invoke: vi.fn(async () => undefined),
}));

vi.mock("@tauri-apps/api/core", () => coreApi);

import { dismissOwnedWindow } from "./tauriOwnedDialogControls";

test("delegates dismissal to the native owner-safe lifecycle", async () => {
  await dismissOwnedWindow();

  expect(coreApi.invoke).toHaveBeenCalledOnce();
  expect(coreApi.invoke).toHaveBeenCalledWith("dismiss_owned_dialog");
});
