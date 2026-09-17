import type { ProjectLaunchOutcome } from "../global/application/globalProjectPort";
export type { ExportConflictPolicy } from "../contracts/generated/ExportConflictPolicy";
import type { ExportConflictPolicy } from "../contracts/generated/ExportConflictPolicy";
export type { BatchExportOptions } from "../contracts/generated/BatchExportOptions";
import type { BatchExportOptions } from "../contracts/generated/BatchExportOptions";
export type { BatchExportProgress } from "../contracts/generated/BatchExportProgress";
import type { BatchExportProgress } from "../contracts/generated/BatchExportProgress";
export type { BatchRecoverySummary } from "../contracts/generated/BatchRecoverySummary";
import type { BatchRecoverySummary } from "../contracts/generated/BatchRecoverySummary";
export type { BatchExportView } from "../contracts/generated/BatchExportView";
import type { BatchExportView } from "../contracts/generated/BatchExportView";

export interface BatchExportPort {
  storageRecovery?: import("./storageRecovery").StorageRecoveryPort;
  current(): Promise<BatchExportView | null>;
  recoveries(): Promise<BatchRecoverySummary[]>;
  chooseFolder(): Promise<string | null>;
  countProjects(source: string): Promise<number>;
  prepare(options: BatchExportOptions): Promise<BatchExportView>;
  recheck(): Promise<BatchExportView>;
  ignore(itemId: string): Promise<BatchExportView>;
  relink(itemId: string | null): Promise<BatchExportView | null>;
  openProject(itemId: string): Promise<ProjectLaunchOutcome>;
  run(policy: ExportConflictPolicy): Promise<BatchExportView>;
  cancel(): Promise<void>;
  resume(id: string): Promise<BatchExportView>;
  end(id: string): Promise<void>;
  close(): Promise<void>;
  resultReady(): Promise<void>;
  progress(): Promise<BatchExportProgress | null>;
  onView(listener: (view: BatchExportView) => void): Promise<() => void>;
  onProgress(listener: (progress: BatchExportProgress) => void): Promise<() => void>;
}
