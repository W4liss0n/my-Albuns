import type {
  ProjectDialogAction,
  ProjectDialogActionEvent,
  ProjectDialogDetail,
  ProjectDialogPresentation,
  ProjectDialogProgress,
  ProjectDialogState,
} from "../application/projectDialogPort";
import type { ProjectDialogAction as IpcProjectDialogAction } from "./generated/ProjectDialogAction";
import type { ProjectDialogActionEvent as IpcProjectDialogActionEvent } from "./generated/ProjectDialogActionEvent";
import type { ProjectDialogDetail as IpcProjectDialogDetail } from "./generated/ProjectDialogDetail";
import type { ProjectDialogPresentation as IpcProjectDialogPresentation } from "./generated/ProjectDialogPresentation";
import type { ProjectDialogProgress as IpcProjectDialogProgress } from "./generated/ProjectDialogProgress";
import type { ProjectDialogState as IpcProjectDialogState } from "./generated/ProjectDialogState";

type ProjectDialogStateKind = ProjectDialogState["kind"];
type IpcProjectDialogStateKind = IpcProjectDialogState["kind"];
type ProjectDialogProgressKind = ProjectDialogProgress["kind"];
type IpcProjectDialogProgressKind = IpcProjectDialogProgress["kind"];

const projectDialogActionMap = {
  cancelAlbumInformation: "cancelAlbumInformation",
  cancelExport: "cancelExport",
  cancelProjectClose: "cancelProjectClose",
  confirmAlbumInformation: "confirmAlbumInformation",
  closeProjectAfterGraphicsFailure: "closeProjectAfterGraphicsFailure",
  discardAndClose: "discardAndClose",
  dismissExport: "dismissExport",
  dismissProjectCloseFailure: "dismissProjectCloseFailure",
  dismissProjectOperationFailure: "dismissProjectOperationFailure",
  retryExport: "retryExport",
  saveAndClose: "saveAndClose",
} as const satisfies Record<IpcProjectDialogAction, ProjectDialogAction> &
  Record<ProjectDialogAction, IpcProjectDialogAction>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function hasOwn<Value extends object, Key extends PropertyKey>(
  value: Value,
  key: Key,
): key is Key & keyof Value {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function isWireU64(value: unknown): value is number {
  return (
    typeof value === "number" &&
    Number.isSafeInteger(value) &&
    value >= 0
  );
}

function isProjectDialogSessionId(value: unknown): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= 128;
}

function decodeDetails(
  value: unknown,
): IpcProjectDialogDetail[] | null {
  if (!Array.isArray(value)) return null;
  const details: IpcProjectDialogDetail[] = [];
  for (const detail of value) {
    if (
      !isRecord(detail) ||
      typeof detail.label !== "string" ||
      typeof detail.value !== "string"
    ) {
      return null;
    }
    details.push({ label: detail.label, value: detail.value });
  }
  return details;
}

type ProgressDecoder = (
  value: Record<string, unknown>,
) => IpcProjectDialogProgress | null;

const progressDecoders: Record<
  IpcProjectDialogProgressKind,
  ProgressDecoder
> &
  Record<ProjectDialogProgressKind, ProgressDecoder> = {
  determinate: (value) =>
    isWireU64(value.completed) &&
    typeof value.status === "string" &&
    isWireU64(value.total)
      ? {
          completed: value.completed,
          kind: "determinate",
          status: value.status,
          total: value.total,
        }
      : null,
  indeterminate: (value) =>
    typeof value.status === "string"
      ? { kind: "indeterminate", status: value.status }
      : null,
};

function decodeProgress(value: unknown): IpcProjectDialogProgress | null {
  if (
    !isRecord(value) ||
    typeof value.kind !== "string" ||
    !hasOwn(progressDecoders, value.kind)
  ) {
    return null;
  }
  return progressDecoders[value.kind](value);
}

type StateDecoder = (
  value: Record<string, unknown>,
) => IpcProjectDialogState | null;

const stateDecoders: Record<
  IpcProjectDialogStateKind,
  StateDecoder
> &
  Record<ProjectDialogStateKind, StateDecoder> = {
  albumInformationConfirmation: (value) => {
    const details = decodeDetails(value.details);
    return typeof value.busy === "boolean" && details
      ? {
          busy: value.busy,
          details,
          kind: "albumInformationConfirmation",
        }
      : null;
  },
  exportFailure: (value) =>
    typeof value.cancelled === "boolean" &&
    typeof value.message === "string" &&
    typeof value.retryDisabled === "boolean"
      ? {
          cancelled: value.cancelled,
          kind: "exportFailure",
          message: value.message,
          retryDisabled: value.retryDisabled,
        }
      : null,
  exportProgress: (value) => {
    const progress = decodeProgress(value.progress);
    return typeof value.cancelRequested === "boolean" &&
      typeof value.cancellable === "boolean" &&
      progress
      ? {
          cancelRequested: value.cancelRequested,
          cancellable: value.cancellable,
          kind: "exportProgress",
          progress,
        }
      : null;
  },
  exportSuccess: (value) =>
    typeof value.message === "string"
      ? { kind: "exportSuccess", message: value.message }
      : null,
  graphicsFailure: (value) =>
    typeof value.reason === "string"
      ? { kind: "graphicsFailure", reason: value.reason }
      : null,
  projectCloseConfirmation: (value) =>
    typeof value.busy === "boolean"
      ? { busy: value.busy, kind: "projectCloseConfirmation" }
      : null,
  projectCloseFailure: (value) =>
    typeof value.message === "string"
      ? { kind: "projectCloseFailure", message: value.message }
      : null,
  projectOperationFailure: (value) =>
    typeof value.message === "string"
      ? { kind: "projectOperationFailure", message: value.message }
      : null,
};

export function parseProjectDialogAction(
  value: unknown,
): ProjectDialogAction | null {
  return typeof value === "string" &&
    hasOwn(projectDialogActionMap, value)
    ? projectDialogActionMap[value]
    : null;
}

export function parseProjectDialogActionEvent(
  value: unknown,
): ProjectDialogActionEvent | null {
  if (
    !isRecord(value) ||
    !isProjectDialogSessionId(value.sessionId)
  ) {
    return null;
  }
  const action = parseProjectDialogAction(value.action);
  if (!action) return null;
  const event = {
    action: toIpcProjectDialogAction(action),
    sessionId: value.sessionId,
  } satisfies IpcProjectDialogActionEvent;
  return { action: projectDialogActionMap[event.action], sessionId: event.sessionId };
}

export function parseProjectDialogPresentation(
  value: unknown,
): ProjectDialogPresentation | null {
  if (!isRecord(value) || !isProjectDialogSessionId(value.sessionId)) {
    return null;
  }
  const state = parseProjectDialogState(value.state);
  if (!state) return null;
  const presentation = {
    sessionId: value.sessionId,
    state: toIpcProjectDialogState(state),
  } satisfies IpcProjectDialogPresentation;
  return {
    sessionId: presentation.sessionId,
    state: fromIpcProjectDialogState(presentation.state),
  };
}

export function toIpcProjectDialogAction(
  action: ProjectDialogAction,
): IpcProjectDialogAction {
  return projectDialogActionMap[action];
}

export function parseProjectDialogState(
  value: unknown,
): ProjectDialogState | null {
  if (
    !isRecord(value) ||
    typeof value.kind !== "string" ||
    !hasOwn(stateDecoders, value.kind)
  ) {
    return null;
  }
  const state = stateDecoders[value.kind](value);
  return state ? fromIpcProjectDialogState(state) : null;
}

export function toIpcProjectDialogState(
  state: ProjectDialogState,
): IpcProjectDialogState {
  switch (state.kind) {
    case "albumInformationConfirmation":
      return {
        busy: state.busy,
        details: state.details.map(toIpcProjectDialogDetail),
        kind: state.kind,
      };
    case "projectCloseConfirmation":
      return { busy: state.busy, kind: state.kind };
    case "projectCloseFailure":
    case "projectOperationFailure":
    case "exportSuccess":
      return { kind: state.kind, message: state.message };
    case "graphicsFailure":
      return { kind: state.kind, reason: state.reason };
    case "exportProgress":
      return {
        cancelRequested: state.cancelRequested,
        cancellable: state.cancellable,
        kind: state.kind,
        progress: toIpcProjectDialogProgress(state.progress),
      };
    case "exportFailure":
      return {
        cancelled: state.cancelled,
        kind: state.kind,
        message: state.message,
        retryDisabled: state.retryDisabled,
      };
  }
  return assertNever(state);
}

function fromIpcProjectDialogState(
  state: IpcProjectDialogState,
): ProjectDialogState {
  switch (state.kind) {
    case "albumInformationConfirmation":
      return {
        busy: state.busy,
        details: state.details.map(fromIpcProjectDialogDetail),
        kind: state.kind,
      };
    case "projectCloseConfirmation":
      return { busy: state.busy, kind: state.kind };
    case "projectCloseFailure":
    case "projectOperationFailure":
    case "exportSuccess":
      return { kind: state.kind, message: state.message };
    case "graphicsFailure":
      return { kind: state.kind, reason: state.reason };
    case "exportProgress":
      return {
        cancelRequested: state.cancelRequested,
        cancellable: state.cancellable,
        kind: state.kind,
        progress: fromIpcProjectDialogProgress(state.progress),
      };
    case "exportFailure":
      return {
        cancelled: state.cancelled,
        kind: state.kind,
        message: state.message,
        retryDisabled: state.retryDisabled,
      };
  }
  return assertNever(state);
}

function toIpcProjectDialogDetail(
  detail: ProjectDialogDetail,
): IpcProjectDialogDetail {
  return { label: detail.label, value: detail.value };
}

function fromIpcProjectDialogDetail(
  detail: IpcProjectDialogDetail,
): ProjectDialogDetail {
  return { label: detail.label, value: detail.value };
}

function toIpcProjectDialogProgress(
  progress: ProjectDialogProgress,
): IpcProjectDialogProgress {
  switch (progress.kind) {
    case "indeterminate":
      return { kind: progress.kind, status: progress.status };
    case "determinate":
      return {
        completed: progress.completed,
        kind: progress.kind,
        status: progress.status,
        total: progress.total,
      };
  }
  return assertNever(progress);
}

function fromIpcProjectDialogProgress(
  progress: IpcProjectDialogProgress,
): ProjectDialogProgress {
  switch (progress.kind) {
    case "indeterminate":
      return { kind: progress.kind, status: progress.status };
    case "determinate":
      return {
        completed: progress.completed,
        kind: progress.kind,
        status: progress.status,
        total: progress.total,
      };
  }
  return assertNever(progress);
}

export function parseInitialProjectDialogPreviewState(
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

export function parseInitialProjectDialogPresentation(
  search: string,
): ProjectDialogPresentation | null {
  const encoded = new URLSearchParams(search).get("presentation");
  if (!encoded) return null;
  try {
    return parseProjectDialogPresentation(JSON.parse(encoded));
  } catch {
    return null;
  }
}

function assertNever(value: never): never {
  throw new Error(`unsupported Project dialog contract: ${String(value)}`);
}
