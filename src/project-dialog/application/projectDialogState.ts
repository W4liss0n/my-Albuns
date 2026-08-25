import type { ProjectDialogState } from "../../application/projectDialogPort";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function parseProjectDialogState(
  value: unknown,
): ProjectDialogState | null {
  if (!isRecord(value) || typeof value.kind !== "string") return null;

  switch (value.kind) {
    case "albumInformationConfirmation":
      return typeof value.busy === "boolean" &&
        Array.isArray(value.details) &&
        value.details.every((detail) => typeof detail === "string")
        ? {
            busy: value.busy,
            details: value.details,
            kind: value.kind,
          }
        : null;
    case "projectCloseConfirmation":
      return typeof value.busy === "boolean"
        ? { busy: value.busy, kind: value.kind }
        : null;
    case "projectCloseFailure":
      return typeof value.message === "string"
        ? { kind: value.kind, message: value.message }
        : null;
    case "exportProgress": {
      if (
        typeof value.cancelRequested !== "boolean" ||
        typeof value.cancellable !== "boolean" ||
        !isRecord(value.progress) ||
        typeof value.progress.kind !== "string" ||
        typeof value.progress.status !== "string"
      ) {
        return null;
      }
      const progress =
        value.progress.kind === "indeterminate"
          ? {
              kind: "indeterminate" as const,
              status: value.progress.status,
            }
          : value.progress.kind === "determinate" &&
              typeof value.progress.completed === "number" &&
              Number.isFinite(value.progress.completed) &&
              typeof value.progress.total === "number" &&
              Number.isFinite(value.progress.total)
            ? {
                completed: value.progress.completed,
                kind: "determinate" as const,
                status: value.progress.status,
                total: value.progress.total,
              }
            : null;
      return progress
        ? {
            cancelRequested: value.cancelRequested,
            cancellable: value.cancellable,
            kind: value.kind,
            progress,
          }
        : null;
    }
    case "exportFailure":
      return typeof value.cancelled === "boolean" &&
        typeof value.message === "string" &&
        typeof value.retryDisabled === "boolean"
        ? {
            cancelled: value.cancelled,
            kind: value.kind,
            message: value.message,
            retryDisabled: value.retryDisabled,
          }
        : null;
    default:
      return null;
  }
}

export function parseInitialProjectDialogState(
  search: string,
): ProjectDialogState | null {
  const encoded = new URLSearchParams(search).get("state");
  if (!encoded) return null;
  try {
    return parseProjectDialogState(JSON.parse(encoded));
  } catch {
    return null;
  }
}
