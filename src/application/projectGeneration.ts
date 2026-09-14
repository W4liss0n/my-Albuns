export type GenerationDecision = "replace" | "ignore";
export interface GenerationOptions { sourceFolder: string; destinationFolder: string; }
export interface GenerationProgress { completed: number; total: number | null; }
export interface GenerationView {
  id: string; options: GenerationOptions;
  phase: "prepared" | "running" | "finished" | "cancelled";
  canContinue: boolean;
  items: { id: string; name: string; destination: string; status: "pending" | "completed" | "ignored" | "failed"; problems: string[]; conflict: boolean; canReplace: boolean; decision: GenerationDecision | null; }[];
}

export interface ProjectGenerationLauncher { open(): Promise<void>; }
export interface ProjectGenerationPort {
  model(): Promise<string>;
  chooseFolder(): Promise<string | null>;
  count(source: string): Promise<number>;
  current(): Promise<GenerationView | null>;
  prepare(options: GenerationOptions): Promise<GenerationView>;
  decide(id: string | null, decision: GenerationDecision): Promise<GenerationView>;
  recheck(): Promise<GenerationView>;
  run(): Promise<GenerationView>;
  progress(): Promise<GenerationProgress | null>;
  onView(callback: (view: GenerationView) => void): Promise<() => void>;
  onProgress(callback: (progress: GenerationProgress) => void): Promise<() => void>;
  resultReady(): Promise<void>;
  cancel(): Promise<void>;
  close(): Promise<void>;
}
