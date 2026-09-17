export type { GenerationDecision } from "../contracts/generated/GenerationDecision";
import type { GenerationDecision } from "../contracts/generated/GenerationDecision";
export type { GenerationOptions } from "../contracts/generated/GenerationOptions";
import type { GenerationOptions } from "../contracts/generated/GenerationOptions";
export type { GenerationProgress } from "../contracts/generated/GenerationProgress";
import type { GenerationProgress } from "../contracts/generated/GenerationProgress";
export type { GenerationView } from "../contracts/generated/GenerationView";
import type { GenerationView } from "../contracts/generated/GenerationView";

export interface ProjectGenerationLauncher { open(): Promise<void>; }
export interface ProjectGenerationPort {
  model(): Promise<string>;
  chooseFolder(): Promise<string | null>;
  count(source: string): Promise<number>;
  current(): Promise<GenerationView | null>;
  /** Null means verification was cancelled; keep the current configuration. */
  prepare(options: GenerationOptions): Promise<GenerationView | null>;
  decide(id: string | null, decision: GenerationDecision): Promise<GenerationView>;
  recheck(): Promise<GenerationView | null>;
  run(): Promise<GenerationView | null>;
  progress(): Promise<GenerationProgress | null>;
  onView(callback: (view: GenerationView) => void): Promise<() => void>;
  onProgress(callback: (progress: GenerationProgress) => void): Promise<() => void>;
  resultReady(): Promise<void>;
  cancel(): Promise<void>;
  close(): Promise<void>;
}
