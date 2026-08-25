import { afterEach, beforeEach, expect, test, vi } from "vitest";

const windowApi = vi.hoisted(() => ({
  center: vi.fn(async () => undefined),
  close: vi.fn(async () => undefined),
  minimize: vi.fn(async () => undefined),
  setSize: vi.fn<(size: unknown) => Promise<void>>(async () => undefined),
  toggleMaximize: vi.fn(async () => undefined),
}));
const coreApi = vi.hoisted(() => ({
  invoke: vi.fn(async () => undefined),
}));

vi.mock("@tauri-apps/api/core", () => coreApi);

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowApi,
  LogicalSize: class LogicalSize {
    constructor(
      public width: number,
      public height: number,
    ) {}
  },
}));

let tauriWindowControls: typeof import("./tauriWindowControls").tauriWindowControls;

beforeEach(async () => {
  vi.resetModules();
  vi.clearAllMocks();
  coreApi.invoke.mockResolvedValue(undefined);
  windowApi.center.mockResolvedValue(undefined);
  windowApi.setSize.mockResolvedValue(undefined);
  window.history.replaceState(
    null,
    "",
    "/?ownedReadyToken=1",
  );
  vi.spyOn(document.documentElement, "clientWidth", "get").mockReturnValue(
    520,
  );
  vi.spyOn(window.screen, "availHeight", "get").mockReturnValue(900);
  ({ tauriWindowControls } = await import("./tauriWindowControls"));
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
  expect(coreApi.invoke).toHaveBeenCalledOnce();
  expect(coreApi.invoke).toHaveBeenCalledWith(
    "owned_window_content_ready",
    { token: 1 },
  );
  expect(windowApi.center.mock.invocationCallOrder[0]).toBeLessThan(
    coreApi.invoke.mock.invocationCallOrder[0] ?? 0,
  );

  await tauriWindowControls.fitContent(198);
  expect(windowApi.setSize).toHaveBeenCalledOnce();
  expect(windowApi.center).toHaveBeenCalledOnce();
  expect(coreApi.invoke).toHaveBeenCalledOnce();

  await tauriWindowControls.fitContent(220);
  expect(windowApi.setSize).toHaveBeenCalledTimes(2);
  expect(windowApi.center).toHaveBeenCalledTimes(2);
  expect(coreApi.invoke).toHaveBeenCalledOnce();
});

test("coalesces concurrent fits for the same rendered size", async () => {
  let releaseSetSize: (() => void) | undefined;
  const setSizeGate = new Promise<void>((resolve) => {
    releaseSetSize = resolve;
  });
  windowApi.setSize.mockReturnValue(setSizeGate);

  const firstFit = tauriWindowControls.fitContent(264);
  const duplicateFit = tauriWindowControls.fitContent(264);

  releaseSetSize?.();
  await Promise.all([firstFit, duplicateFit]);

  expect(windowApi.setSize).toHaveBeenCalledOnce();
  expect(windowApi.center).toHaveBeenCalledOnce();
});

test("serializes changing fits instead of racing native window updates", async () => {
  const releaseSetSize: Array<() => void> = [];
  windowApi.setSize.mockImplementation(
    () =>
      new Promise<void>((resolve) => {
        releaseSetSize.push(resolve);
      }),
  );

  const firstFit = tauriWindowControls.fitContent(320);
  const latestFit = tauriWindowControls.fitContent(360);

  expect(windowApi.setSize).toHaveBeenCalledOnce();
  releaseSetSize[0]?.();
  await vi.waitFor(() => {
    expect(windowApi.setSize).toHaveBeenCalledTimes(2);
  });
  releaseSetSize[1]?.();
  await Promise.all([firstFit, latestFit]);

  expect(windowApi.setSize).toHaveBeenNthCalledWith(1, {
    height: 320,
    width: 520,
  });
  expect(windowApi.setSize).toHaveBeenNthCalledWith(2, {
    height: 360,
    width: 520,
  });
  expect(windowApi.center).toHaveBeenCalledTimes(2);
});

test("retries the readiness handshake without resizing again", async () => {
  coreApi.invoke.mockRejectedValueOnce(new Error("temporary IPC failure"));

  await expect(tauriWindowControls.fitContent(198)).rejects.toThrow(
    "temporary IPC failure",
  );
  await tauriWindowControls.fitContent(198);

  expect(windowApi.setSize).toHaveBeenCalledOnce();
  expect(windowApi.center).toHaveBeenCalledOnce();
  expect(coreApi.invoke).toHaveBeenCalledTimes(2);
});
