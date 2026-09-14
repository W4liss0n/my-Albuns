export type { GenerationView } from "../platform/generated/GenerationView";
export type { GenerationOptions } from "../platform/generated/GenerationOptions";
export type { GenerationProgress } from "../platform/generated/GenerationProgress";
export type { GenerationDecision } from "../platform/generated/GenerationDecision";
import type { GenerationView } from "../platform/generated/GenerationView";
import type { GenerationOptions } from "../platform/generated/GenerationOptions";
import type { GenerationProgress } from "../platform/generated/GenerationProgress";
import type { GenerationDecision } from "../platform/generated/GenerationDecision";

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
