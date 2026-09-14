import type {
  ProjectDialogAction,
  ProjectDialogState,
} from "../../application/projectDialogPort";

export function defaultProjectDialogCloseAction(
  state: ProjectDialogState,
): ProjectDialogAction | null {
  switch (state.kind) {
    case "storageFull": return state.busy ? null : "cancelStorage";
    case "exportConfiguration": return state.busy ? null : "dismissExport";
    case "exportConflicts": return "dismissExport";
    case "mediaRemovalConfirmation": return state.busy ? null : "cancelMediaRemoval";
    case "layoutDeletionConfirmation":
      return state.busy ? null : "cancelLayoutDeletion";
    case "imageProcessingProgress":
      return null;
    case "albumInformationConfirmation":
      return state.busy ? null : "cancelAlbumInformation";
    case "projectCloseConfirmation":
      return state.busy ? null : "cancelProjectClose";
    case "projectCloseFailure":
      return "dismissProjectCloseFailure";
    case "imageProcessingProblems":
      return "dismissImageProcessingProblems";
    case "projectOperationFailure":
      return "dismissProjectOperationFailure";
    case "graphicsFailure":
      return "closeProjectAfterGraphicsFailure";
    case "exportProgress":
      return state.cancellable && !state.cancelRequested
        ? "cancelExport"
        : null;
    case "exportMediaProblems": return state.busy ? null : "dismissExport";
    case "exportFailure":
    case "exportProblems":
    case "exportSuccess":
      return "dismissExport";
  }

  return assertNever(state);
}

function assertNever(value: never): never {
  throw new Error(`unsupported Project dialog state: ${String(value)}`);
}
