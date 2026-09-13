import { afterEach, beforeEach, expect, test, vi } from "vitest";

const windowApi = vi.hoisted(() => ({
  center: vi.fn(async () => undefined),
  close: vi.fn(async () => undefined),
  minimize: vi.fn(async () => undefined),
  toggleMaximize: vi.fn(async () => undefined),
}));
const coreApi = vi.hoisted(() => ({
  fitBounds: vi.fn<(size: unknown) => Promise<void>>(async () => undefined),
  ready: vi.fn<(...args: unknown[]) => Promise<void>>(async () => undefined),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args: unknown) => {
    if (command === "fit_owned_window") return coreApi.fitBounds(args);
    if (command === "owned_window_content_ready") return coreApi.ready(command, args);
    throw new Error(`Unexpected command: ${command}`);
  },
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowApi,
}));

let tauriWindowControls: typeof import("./tauriWindowControls").tauriWindowControls;

beforeEach(async () => {
  vi.resetModules();
  vi.clearAllMocks();
  coreApi.ready.mockResolvedValue(undefined);
  windowApi.center.mockResolvedValue(undefined);
  coreApi.fitBounds.mockResolvedValue(undefined);
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
  document.documentElement.style.removeProperty("--ui-owned-window-height-limit");
  vi.restoreAllMocks();
});

test("fits and centers the owned window with a single native command", async () => {
  await tauriWindowControls.fitContent(() => 198);

  expect(coreApi.fitBounds).toHaveBeenCalledWith({
    height: 198,
    width: 520,
  });
  expect(windowApi.center).not.toHaveBeenCalled();
  expect(coreApi.ready).toHaveBeenCalledOnce();
  expect(coreApi.ready).toHaveBeenCalledWith(
    "owned_window_content_ready",
    { token: 1 },
  );
  expect(coreApi.fitBounds.mock.invocationCallOrder[0]).toBeLessThan(
    coreApi.ready.mock.invocationCallOrder[0] ?? 0,
  );

  await tauriWindowControls.fitContent(() => 198);
  expect(coreApi.fitBounds).toHaveBeenCalledOnce();
  expect(windowApi.center).not.toHaveBeenCalled();
  expect(coreApi.ready).toHaveBeenCalledOnce();

  await tauriWindowControls.fitContent(() => 220);
  expect(coreApi.fitBounds).toHaveBeenCalledTimes(2);
  expect(windowApi.center).not.toHaveBeenCalled();
  expect(coreApi.ready).toHaveBeenCalledOnce();
});

test("coalesces concurrent fits for the same rendered size", async () => {
  let releaseSetSize: (() => void) | undefined;
  const setSizeGate = new Promise<void>((resolve) => {
    releaseSetSize = resolve;
  });
  coreApi.fitBounds.mockReturnValue(setSizeGate);

  const firstFit = tauriWindowControls.fitContent(() => 264);
  const duplicateFit = tauriWindowControls.fitContent(() => 264);

  releaseSetSize?.();
  await Promise.all([firstFit, duplicateFit]);

  expect(coreApi.fitBounds).toHaveBeenCalledOnce();
  expect(windowApi.center).not.toHaveBeenCalled();
});

test("fits the current dialog width even while the native viewport retains the previous width", async () => {
  vi.spyOn(document.documentElement, "clientWidth", "get").mockReturnValue(800);
  await tauriWindowControls.fitContent(() => 501, 800);
  await tauriWindowControls.fitContent(() => 208, 440);

  expect(coreApi.fitBounds).toHaveBeenLastCalledWith({ width: 440, height: 208 });
  await tauriWindowControls.fitContent(() => 208, 440);
  expect(coreApi.fitBounds).toHaveBeenCalledTimes(2);
});

test("sets screen bounds before measuring the first visible content size", async () => {
  const measuredLimits: string[] = [];
  await tauriWindowControls.fitContent(() => {
    const limit = document.documentElement.style.getPropertyValue(
      "--ui-owned-window-height-limit",
    );
    measuredLimits.push(limit);
    return limit === "836px" ? 500 : 478;
  });

  expect(measuredLimits).toEqual(["836px"]);
  expect(coreApi.fitBounds).toHaveBeenCalledExactlyOnceWith({
    height: 500,
    width: 520,
  });
  expect(coreApi.ready).toHaveBeenCalledOnce();
});

test("restores the current size when a superseded expansion is still in flight", async () => {
  await tauriWindowControls.fitContent(() => 208, 440);
  let finishExpansion!: () => void;
  coreApi.fitBounds.mockImplementationOnce(
    () => new Promise<void>((resolve) => { finishExpansion = resolve; }),
  );
  const expansion = tauriWindowControls.fitContent(() => 501, 800);
  const currentContent = tauriWindowControls.fitContent(() => 208, 440);
  finishExpansion();
  await Promise.all([expansion, currentContent]);

  expect(coreApi.fitBounds).toHaveBeenLastCalledWith({ width: 440, height: 208 });
});

test("serializes changing fits instead of racing native window updates", async () => {
  const releaseSetSize: Array<() => void> = [];
  coreApi.fitBounds.mockImplementation(
    () =>
      new Promise<void>((resolve) => {
        releaseSetSize.push(resolve);
      }),
  );

  const firstFit = tauriWindowControls.fitContent(() => 320);
  const latestFit = tauriWindowControls.fitContent(() => 360);

  expect(coreApi.fitBounds).toHaveBeenCalledOnce();
  releaseSetSize[0]?.();
  await vi.waitFor(() => {
    expect(coreApi.fitBounds).toHaveBeenCalledTimes(2);
  });
  releaseSetSize[1]?.();
  await Promise.all([firstFit, latestFit]);

  expect(coreApi.fitBounds).toHaveBeenNthCalledWith(1, {
    height: 320,
    width: 520,
  });
  expect(coreApi.fitBounds).toHaveBeenNthCalledWith(2, {
    height: 360,
    width: 520,
  });
  expect(windowApi.center).not.toHaveBeenCalled();
});

test("applies a newer fit requested during the readiness handshake", async () => {
  let releaseReadiness: (() => void) | undefined;
  coreApi.ready.mockReturnValue(
    new Promise<undefined>((resolve) => {
      releaseReadiness = () => resolve(undefined);
    }),
  );

  const firstFit = tauriWindowControls.fitContent(() => 198);
  await vi.waitFor(() => expect(coreApi.ready).toHaveBeenCalledOnce());

  const newerFit = tauriWindowControls.fitContent(() => 236);
  releaseReadiness?.();
  await Promise.all([firstFit, newerFit]);

  expect(coreApi.fitBounds).toHaveBeenNthCalledWith(1, {
    height: 198,
    width: 520,
  });
  expect(coreApi.fitBounds).toHaveBeenNthCalledWith(2, {
    height: 236,
    width: 520,
  });
  expect(windowApi.center).not.toHaveBeenCalled();
});

test("retries the readiness handshake without resizing again", async () => {
  coreApi.ready.mockRejectedValueOnce(new Error("temporary IPC failure"));

  await expect(tauriWindowControls.fitContent(() => 198)).rejects.toThrow(
    "temporary IPC failure",
  );
  await tauriWindowControls.fitContent(() => 198);

  expect(coreApi.fitBounds).toHaveBeenCalledOnce();
  expect(windowApi.center).not.toHaveBeenCalled();
  expect(coreApi.ready).toHaveBeenCalledTimes(2);
});
