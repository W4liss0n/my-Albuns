import { afterEach, expect, test, vi } from "vitest";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { MediaFileDrag } from "./generated/MediaFileDrag";
import { tauriMediaDropPort } from "./tauriMediaDropPort";

vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: vi.fn() }));
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

test("maps physical Windows drop coordinates to CSS pixels and keeps the native drop opaque", async () => {
  let publish!: (event: { payload: MediaFileDrag }) => void;
  const stop = vi.fn();
  const listen = vi.fn(async (_name: string, listener: typeof publish) => { publish = listener; return stop; });
  vi.mocked(getCurrentWindow).mockReturnValue({ listen } as unknown as ReturnType<typeof getCurrentWindow>);
  vi.stubGlobal("devicePixelRatio", 1.5);
  const listener = vi.fn();
  const unsubscribe = await tauriMediaDropPort.subscribe(listener);
  expect(listen).toHaveBeenCalledWith("myalbuns-media-file-drag", expect.any(Function));
  publish({ payload: { kind: "over", x: 300, y: 600 } });
  expect(listener).toHaveBeenLastCalledWith({ kind: "over", x: 200, y: 400 });
  publish({ payload: { kind: "drop", dropId: "native-drop-1", x: 300, y: 600 } });
  expect(listener).toHaveBeenLastCalledWith({ kind: "drop", dropId: "native-drop-1", x: 200, y: 400 });
  publish({ payload: { kind: "leave" } });
  expect(listener).toHaveBeenLastCalledWith({ kind: "leave" });
  unsubscribe();
  expect(stop).toHaveBeenCalledOnce();
});