export type LogLevel = "debug" | "info" | "warn" | "error";

export interface LogEvent {
  level: LogLevel;
  component: string;
  event: string;
  projectId?: string;
  operationId?: string;
  instanceId?: string;
  reason?: string;
  width?: number;
  height?: number;
  sheetCount?: number;
}

export interface Logger {
  write(event: LogEvent): void;
}

export const silentLogger: Logger = {
  write: () => undefined,
};

let instanceSequence = 0;

export function createLogInstanceId(prefix: string) {
  instanceSequence += 1;
  return `${prefix}-${instanceSequence}`;
}

export function logReasonFromError(error: unknown) {
  if (!(error instanceof Error)) return "unknown_error";
  const reason = error.name
    .replace(/([a-z])([A-Z])/g, "$1_$2")
    .replace(/[^a-zA-Z0-9_.-]/g, "_")
    .toLowerCase()
    .slice(0, 80);
  return reason || "error";
}
