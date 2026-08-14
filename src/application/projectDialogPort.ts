export type ProjectDialogProgress =
  | {
      kind: "indeterminate";
      note?: string;
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
      kind: "projectCloseConfirmation";
    }
  | {
      kind: "projectCloseFailure";
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
    };

export type ProjectDialogAction =
  | "cancelExport"
  | "cancelProjectClose"
  | "discardAndClose"
  | "dismissExport"
  | "dismissProjectCloseFailure"
  | "retryExport"
  | "saveAndClose";

export interface ProjectDialogPort {
  dismiss(): Promise<void>;
  onAction(
    listener: (action: ProjectDialogAction) => void,
  ): Promise<() => void>;
  present(state: ProjectDialogState): Promise<void>;
}
