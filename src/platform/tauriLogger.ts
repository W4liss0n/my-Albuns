import { invoke } from "@tauri-apps/api/core";

import type { LogEvent, Logger } from "../application/logging";
import type { FrontendLogEvent } from "./generated/FrontendLogEvent";

function writeToDevelopmentConsole(event: LogEvent) {
  if (!import.meta.env.DEV) return;
  const label = `[${event.component}] ${event.event}`;
  switch (event.level) {
    case "debug":
      console.debug(label, event);
      break;
    case "info":
      console.info(label, event);
      break;
    case "warn":
      console.warn(label, event);
      break;
    case "error":
      console.error(label, event);
      break;
  }
}

export const tauriLogger: Logger = {
  write(event) {
    writeToDevelopmentConsole(event);
    const ipcEvent = event satisfies FrontendLogEvent;
    void invoke("frontend_log", { event: ipcEvent }).catch(() => {
      console.warn(
        "Não foi possível encaminhar um evento de diagnóstico ao host Tauri.",
      );
    });
  },
};
