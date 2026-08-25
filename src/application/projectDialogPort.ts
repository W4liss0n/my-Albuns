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

export type ProjectDialogState =
  | {
      busy: boolean;
      details: readonly string[];
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
  | "dismissExport"
  | "dismissProjectCloseFailure"
  | "dismissProjectOperationFailure"
  | "retryExport"
  | "saveAndClose";

export interface ProjectDialogPort {
  dismiss(): Promise<void>;
  onAction(
    listener: (action: ProjectDialogAction) => void,
  ): Promise<() => void>;
  present(state: ProjectDialogState): Promise<void>;
}
