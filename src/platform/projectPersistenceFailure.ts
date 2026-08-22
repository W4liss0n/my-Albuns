import type {
  SaveProjectErrorContext,
  SaveProjectFailureCode,
} from "../application/projectPorts";
import { isIpcRecord, isIpcRevision } from "./ipcGuards";

export interface ProjectPersistenceFailure {
  code: SaveProjectFailureCode;
  message: string;
  context?: SaveProjectErrorContext;
}

export function parseProjectPersistenceFailure<
  Code extends SaveProjectFailureCode,
>(
  error: unknown,
  messages: Readonly<Record<Code, string>>,
): ProjectPersistenceFailure | null {
  if (!isIpcRecord(error) || typeof error.code !== "string") {
    return null;
  }

  const code = error.code as Code;
  if (!Object.prototype.hasOwnProperty.call(messages, code)) {
    return null;
  }

  if (code === "stale_revision") {
    if (
      !isIpcRevision(error.expectedRevision) ||
      !isIpcRevision(error.currentRevision)
    ) {
      return null;
    }
    return {
      code,
      message: messages[code],
      context: {
        expected: error.expectedRevision,
        current: error.currentRevision,
      },
    };
  }

  return { code, message: messages[code] };
}
