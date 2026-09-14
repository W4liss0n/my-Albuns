import type { ProjectLaunchOutcome } from "../global/application/globalProjectPort";

export type ExportConflictPolicy = "ask" | "skip" | "replace";
export interface BatchExportOptions {
  sourceFolder: string;
  destinationFolder: string | null;
  format: { kind: "jpeg"; quality: number } | { kind: "png" } | { kind: "pdf" };
  mode: "sheet" | "page";
}
export interface BatchExportProgress { completed: number; total: number; percent: number }
export interface BatchRecoverySummary { id: string; sourceFolder: string; total: number; remaining: number }
export interface BatchExportView {
  id: string;
  options: BatchExportOptions;
  phase: "prepared" | "running" | "interrupted" | "finished";
  items: {
    id: string;
    name: string;
    projectPath: string;
    destination: string;
    status: "pending" | "completed" | "ignored" | "failed";
    problems: { kind: "placeholder" | "missingMedia" | "unavailable" | "invalidProject" | "changed" | "failed"; message: string; mediaId: string | null }[];
  }[];
  hasConflicts: boolean;
  canContinue: boolean;
}

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
