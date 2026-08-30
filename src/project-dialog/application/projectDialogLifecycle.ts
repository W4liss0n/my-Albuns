import type {
  ProjectDialogAction,
  ProjectDialogState,
} from "../../application/projectDialogPort";

export function defaultProjectDialogCloseAction(
  state: ProjectDialogState,
): ProjectDialogAction | null {
  switch (state.kind) {
    case "albumInformationConfirmation":
      return state.busy ? null : "cancelAlbumInformation";
    case "projectCloseConfirmation":
      return state.busy ? null : "cancelProjectClose";
    case "projectCloseFailure":
      return "dismissProjectCloseFailure";
    case "projectOperationFailure":
      return "dismissProjectOperationFailure";
    case "graphicsFailure":
      return "closeProjectAfterGraphicsFailure";
    case "exportProgress":
      return state.cancellable && !state.cancelRequested
        ? "cancelExport"
        : null;
    case "exportFailure":
    case "exportSuccess":
      return "dismissExport";
  }

  return assertNever(state);
}

function assertNever(value: never): never {
  throw new Error(`unsupported Project dialog state: ${String(value)}`);
}
