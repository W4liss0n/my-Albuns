import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { MediaDropPort } from "../application/projectPorts";

export const tauriMediaDropPort: MediaDropPort = {
  subscribe: (listener) => getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (payload.type === "leave") { listener({ kind: "leave" }); return; }
    const ratio = window.devicePixelRatio || 1;
    const position = { x: payload.position.x / ratio, y: payload.position.y / ratio };
    listener(payload.type === "drop"
      ? { kind: "drop", ...position, paths: payload.paths }
      : { kind: "over", ...position });
  }),
};
