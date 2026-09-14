import { vi } from "vitest";

import type {
  ExportAttempt,
  ExportCancelStatus,
  ExportOutcome,
  ExportPipelinePort,
  ExportProgressEvent,
} from "../application/projectPorts";

interface AttemptHarness {
  cancel: ReturnType<typeof vi.fn<() => Promise<ExportCancelStatus>>>;
  emit(event: ExportProgressEvent): void;
  reject(error: unknown): void;
  resolve(outcome: ExportOutcome): void;
}

export function createExportHarness() {
  const attempts: AttemptHarness[] = [];
  const startSheet = vi.fn<ExportPipelinePort["startSheet"]>((_selection, onEvent) => {
    let resolve!: (outcome: ExportOutcome) => void;
    let reject!: (error: unknown) => void;
    const completion = new Promise<ExportOutcome>((resolvePromise, rejectPromise) => {
      resolve = resolvePromise;
      reject = rejectPromise;
    });
    const cancel = vi.fn(async (): Promise<ExportCancelStatus> => "requested");
    const attempt: ExportAttempt = { completion, cancel };
    attempts.push({ cancel, emit: onEvent, reject, resolve });
    return attempt;
  });

  return {
    attempts,
    port: { defaultDestination: async () => "C:/Exportados/Album", chooseDestination: async () => null, startSheet } satisfies ExportPipelinePort,
    startSheet,
  };
}

