import { afterEach, expect, test, vi } from "vitest";
import { getCurrentWebview, type DragDropEvent } from "@tauri-apps/api/webview";
import { tauriMediaDropPort } from "./tauriMediaDropPort";

vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: vi.fn() }));
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

test("maps physical Windows drop coordinates to CSS pixels and returns native cleanup", async () => {
  let publish!: (event: { payload: DragDropEvent }) => void;
  const stop = vi.fn();
  vi.mocked(getCurrentWebview).mockReturnValue({ onDragDropEvent: async (listener: typeof publish) => { publish = listener; return stop; } } as unknown as ReturnType<typeof getCurrentWebview>);
  vi.stubGlobal("devicePixelRatio", 1.5);
  const listener = vi.fn();
  const unsubscribe = await tauriMediaDropPort.subscribe(listener);
  const position = { x: 300, y: 600 } as Extract<DragDropEvent, { type: "drop" }>["position"];
  publish({ payload: { type: "enter", paths: ["C:\\Fotos"], position } });
  expect(listener).toHaveBeenLastCalledWith({ kind: "over", x: 200, y: 400 });
  publish({ payload: { type: "drop", paths: ["C:\\Fotos"], position } });
  expect(listener).toHaveBeenLastCalledWith({ kind: "drop", paths: ["C:\\Fotos"], x: 200, y: 400 });
  publish({ payload: { type: "leave" } });
  expect(listener).toHaveBeenLastCalledWith({ kind: "leave" });
  unsubscribe();
  expect(stop).toHaveBeenCalledOnce();
});
