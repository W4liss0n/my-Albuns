import type { ExportMediaProblem } from "./exportMedia";
import type { StorageFullPresentation, StorageRecoveryAction } from "./storageRecovery";
import type { NormalExportOptions, ExportSheetInfo } from "./normalExport";
import type { ImageProcessingProblem } from "./projectPorts";
import type { LayoutExportProblem } from "../domain/project";

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
  | StorageFullPresentation
  | { kind: "exportConfiguration"; sheets: ExportSheetInfo[]; options: NormalExportOptions; busy: boolean; message: string }
  | { kind: "exportConflicts"; files: string[] }
  | { kind: "exportMediaProblems"; projectName: string; problems: readonly ExportMediaProblem[]; busy: boolean; message: string }
  | { kind: "mediaRemovalConfirmation"; mediaKind: "photo" | "decorative"; count: number; usedCount: number; usageCount: number; busy: boolean }
  | { kind: "layoutDeletionConfirmation"; busy: boolean }
  | { kind: "edgeConversionConfirmation"; message: string }
  | { kind: "exportProblems"; projectName: string; problems: readonly LayoutExportProblem[] }
  | { kind: "imageProcessingProgress"; progress: ProjectDialogProgress }
  | {
      kind: "imageProcessingProblems";
      importedCount: number | null;
      problems: readonly ImageProcessingProblem[];
      operationProblem?: string | null;
    }
  | {
      busy: boolean;
      /** What applying does that the panel cannot show; never the changed values. */
      consequences: readonly string[];
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
  | StorageRecoveryAction
  | { configureExport: NormalExportOptions }
  | { chooseExportDestination: NormalExportOptions }
  | "confirmExportOverwrite"
  | "skipExportConflicts"
  | "relinkExportMedia" | "retryExportMedia"
  | "cancelMediaRemoval"
  | "removeAllMedia"
  | "removeMediaKeepFrames"
  | "cancelLayoutDeletion"
  | "confirmLayoutDeletion"
  | "cancelEdgeConversion"
  | "confirmEdgeConversion"
  | "cancelAlbumInformation"
  | "cancelExport"
  | "cancelProjectClose"
  | "discardAndClose"
  | "confirmAlbumInformation"
  | "closeProjectAfterGraphicsFailure"
  | "dismissExport"
  | "openExportProject"
  | "dismissProjectCloseFailure"
  | "dismissProjectOperationFailure"
  | "dismissImageProcessingProblems"
  | "retryExport"
  | "saveAndClose";

export interface ProjectDialogActionEvent {
  action: ProjectDialogAction;
  sessionId: string;
}

export interface ProjectDialogPresentation {
  sessionId: string;
  state: ProjectDialogState;
  windowWidth: number;
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
