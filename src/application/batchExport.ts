import type { ProjectLaunchOutcome } from "../global/application/globalProjectPort";
import type { BatchExportOptions } from "../platform/generated/BatchExportOptions";
import type { BatchExportProgress } from "../platform/generated/BatchExportProgress";
import type { BatchExportView } from "../platform/generated/BatchExportView";
import type { BatchRecoverySummary } from "../platform/generated/BatchRecoverySummary";
import type { ExportConflictPolicy } from "../platform/generated/ExportConflictPolicy";

export type { BatchExportOptions, BatchExportProgress, BatchExportView, BatchRecoverySummary, ExportConflictPolicy };

export interface BatchExportPort {
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
