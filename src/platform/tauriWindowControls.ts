import { getCurrentWindow } from "@tauri-apps/api/window";

import type { WindowControls } from "../ui/WindowControlsContext";

export const tauriWindowControls: WindowControls = {
  close: () => getCurrentWindow().close(),
  minimize: () => getCurrentWindow().minimize(),
  toggleMaximize: () => getCurrentWindow().toggleMaximize(),
};
