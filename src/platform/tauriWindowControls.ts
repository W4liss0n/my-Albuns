import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

import type { WindowControls } from "../ui/WindowControlsContext";

const MIN_OWNED_WINDOW_HEIGHT = 120;
const OWNED_WINDOW_SCREEN_MARGIN = 64;
let lastFittedSize = "";

interface RequestedFit {
  height: number;
  sizeKey: string;
  width: number;
}

let requestedFit: RequestedFit | null = null;
let fitQueue: Promise<void> | null = null;
let confirmedReadyToken: number | null = null;

function currentReadyToken() {
  const token = Number(
    new URLSearchParams(window.location.search).get("ownedReadyToken"),
  );
  return Number.isSafeInteger(token) && token > 0 ? token : null;
}

async function runFitQueue() {
  const currentWindow = getCurrentWindow();
  for (;;) {
    while (requestedFit) {
      const nextFit = requestedFit;
      if (nextFit.sizeKey === lastFittedSize) {
        if (requestedFit === nextFit) requestedFit = null;
        continue;
      }

      await currentWindow.setSize(
        new LogicalSize(nextFit.width, nextFit.height),
      );
      await currentWindow.center();
      lastFittedSize = nextFit.sizeKey;
      if (requestedFit === nextFit) requestedFit = null;
    }

    const readyToken = currentReadyToken();
    if (readyToken !== null && confirmedReadyToken !== readyToken) {
      confirmedReadyToken = readyToken;
      try {
        await invoke<void>("owned_window_content_ready", {
          token: readyToken,
        });
      } catch (error) {
        confirmedReadyToken = null;
        throw error;
      }
    }

    if (!requestedFit) return;
  }
}

function ensureFitQueue() {
  if (fitQueue) return fitQueue;
  const runningQueue = runFitQueue();
  fitQueue = runningQueue;
  void runningQueue.then(
    () => {
      if (fitQueue === runningQueue) fitQueue = null;
    },
    () => {
      if (fitQueue === runningQueue) fitQueue = null;
    },
  );
  return runningQueue;
}

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
  const readyToken = currentReadyToken();
  const readinessPending =
    readyToken !== null && confirmedReadyToken !== readyToken;
  if (width <= 0 || (sizeKey === lastFittedSize && !readinessPending)) return;
  if (sizeKey !== lastFittedSize && requestedFit?.sizeKey !== sizeKey) {
    requestedFit = { height: fittedHeight, sizeKey, width };
  }
  await ensureFitQueue();
}

export const tauriWindowControls: WindowControls = {
  close: () => getCurrentWindow().close(),
  fitContent,
  minimize: () => getCurrentWindow().minimize(),
  toggleMaximize: () => getCurrentWindow().toggleMaximize(),
};
