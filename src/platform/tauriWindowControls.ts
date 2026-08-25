import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

import type { WindowControls } from "../ui/WindowControlsContext";

const MIN_OWNED_WINDOW_HEIGHT = 120;
const OWNED_WINDOW_SCREEN_MARGIN = 64;
let lastFittedSize = "";

async function fitContent(height: number) {
  const width = Math.ceil(document.documentElement.clientWidth);
  const availableHeight = Math.max(
    MIN_OWNED_WINDOW_HEIGHT,
    window.screen.availHeight - OWNED_WINDOW_SCREEN_MARGIN,
  );
  const fittedHeight = Math.min(
    availableHeight,
    Math.max(MIN_OWNED_WINDOW_HEIGHT, Math.ceil(height)),
  );
  const sizeKey = `${width}x${fittedHeight}`;
  if (width <= 0 || sizeKey === lastFittedSize) return;

  const currentWindow = getCurrentWindow();
  await currentWindow.setSize(new LogicalSize(width, fittedHeight));
  await currentWindow.center();
  lastFittedSize = sizeKey;
}

export const tauriWindowControls: WindowControls = {
  close: () => getCurrentWindow().close(),
  fitContent,
  minimize: () => getCurrentWindow().minimize(),
  toggleMaximize: () => getCurrentWindow().toggleMaximize(),
};
