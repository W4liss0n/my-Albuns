export type ProjectDialogProgress =
  | {
      kind: "indeterminate";
      status: string;
    }
  | {
      completed: number;
      kind: "determinate";
      status: string;
      total: number;
    };

export interface ProjectDialogDetail {
  label: string;
  value: string;
}

export type ProjectDialogState =
  | {
      busy: boolean;
      details: readonly ProjectDialogDetail[];
      kind: "albumInformationConfirmation";
    }
  | {
      busy: boolean;
      kind: "projectCloseConfirmation";
    }
  | {
      kind: "projectCloseFailure";
      message: string;
    }
  | {
      kind: "projectOperationFailure";
      message: string;
    }
  | {
      kind: "graphicsFailure";
      reason: string;
    }
  | {
      cancelRequested: boolean;
      cancellable: boolean;
      kind: "exportProgress";
      progress: ProjectDialogProgress;
    }
  | {
      cancelled: boolean;
      kind: "exportFailure";
      message: string;
      retryDisabled: boolean;
    }
  | {
      kind: "exportSuccess";
      message: string;
    };

export type ProjectDialogAction =
  | "cancelAlbumInformation"
  | "cancelExport"
  | "cancelProjectClose"
  | "discardAndClose"
  | "confirmAlbumInformation"
  | "closeProjectAfterGraphicsFailure"
  | "dismissExport"
  | "dismissProjectCloseFailure"
  | "dismissProjectOperationFailure"
  | "retryExport"
  | "saveAndClose";

export interface ProjectDialogActionEvent {
  action: ProjectDialogAction;
  sessionId: string;
}

export interface ProjectDialogPresentation {
  sessionId: string;
  state: ProjectDialogState;
}

/**
 * Owns one logical Project dialog from its first projection until dismissal.
 * A released session is obsolete: later updates and dismissals are harmless.
 */
export interface ProjectDialogSession {
  dismiss(): Promise<void>;
  present(state: ProjectDialogState): Promise<void>;
}

export interface ProjectDialogPort {
  acquire(
    onAction: (action: ProjectDialogAction) => void,
  ): ProjectDialogSession;
}
