import { getCurrentWindow } from "@tauri-apps/api/window";
import type { MediaDropPort } from "../application/projectPorts";
import type { MediaFileDrag } from "./generated/MediaFileDrag";

export const tauriMediaDropPort: MediaDropPort = {
  subscribe: (listener) => getCurrentWindow().listen<MediaFileDrag>("myalbuns-media-file-drag", ({ payload }) => {
    if (payload.kind === "leave") { listener(payload); return; }
    const ratio = window.devicePixelRatio || 1;
    listener({ ...payload, x: payload.x / ratio, y: payload.y / ratio });
  }),
};
