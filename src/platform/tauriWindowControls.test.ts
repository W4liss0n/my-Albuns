import { afterEach, beforeEach, expect, test, vi } from "vitest";

const windowApi = vi.hoisted(() => ({
  center: vi.fn(async () => undefined),
  close: vi.fn(async () => undefined),
  minimize: vi.fn(async () => undefined),
  setSize: vi.fn(async () => undefined),
  toggleMaximize: vi.fn(async () => undefined),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowApi,
  LogicalSize: class LogicalSize {
    constructor(
      public width: number,
      public height: number,
    ) {}
  },
}));

import { tauriWindowControls } from "./tauriWindowControls";

beforeEach(() => {
  vi.clearAllMocks();
  vi.spyOn(document.documentElement, "clientWidth", "get").mockReturnValue(
    520,
  );
  vi.spyOn(window.screen, "availHeight", "get").mockReturnValue(900);
});

afterEach(() => {
  vi.restoreAllMocks();
});

test("fits the logical inner height and recenters the owned window", async () => {
  await tauriWindowControls.fitContent(198);

  expect(windowApi.setSize).toHaveBeenCalledWith({
    height: 198,
    width: 520,
  });
  expect(windowApi.center).toHaveBeenCalledOnce();

  await tauriWindowControls.fitContent(198);
  expect(windowApi.setSize).toHaveBeenCalledOnce();
  expect(windowApi.center).toHaveBeenCalledOnce();
});
