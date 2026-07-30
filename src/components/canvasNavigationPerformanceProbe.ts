import type { CanvasNavigationMeasurement } from "../application/topologyBenchmark";
import {
  summarizeFrameTimings,
  type CanvasPerformanceClock,
} from "./canvasPerformanceProbe";

const MINIMUM_LONG_ALBUM_SHEETS = 100;

export interface CanvasNavigationPerformanceProbeConfig {
  cycles: number;
}

export interface CanvasNavigationRenderedFrame {
  renderedAt: number;
  residentSheetCount: number;
  residentTextureCount: number;
  residentTexturePixelCount: number;
}

export interface CanvasNavigationPerformanceTarget {
  sheetIds: readonly string[];
  navigateToSheet(
    sheetId: string,
    signal?: AbortSignal,
  ): Promise<CanvasNavigationRenderedFrame>;
}

const browserFrameClock: CanvasPerformanceClock = {
  now: () => performance.now(),
};

export async function runCanvasNavigationPerformanceProbe(
  config: CanvasNavigationPerformanceProbeConfig,
  target: CanvasNavigationPerformanceTarget,
  clock: CanvasPerformanceClock = browserFrameClock,
  signal?: AbortSignal,
): Promise<CanvasNavigationMeasurement> {
  validateConfig(config, target);
  const firstSheetId = target.sheetIds[0];
  const middleSheetId =
    target.sheetIds[Math.floor((target.sheetIds.length - 1) / 2)];
  const lastSheetId = target.sheetIds[target.sheetIds.length - 1];
  const route = [middleSheetId, lastSheetId, firstSheetId];
  const latencies: number[] = [];
  let maxResidentSheetCount = 0;
  let maxResidentTextureCount = 0;
  let maxResidentTexturePixelCount = 0;

  for (let cycle = 0; cycle < config.cycles; cycle += 1) {
    for (const sheetId of route) {
      throwIfAborted(signal);
      const requestedAt = clock.now();
      const rendered = await target.navigateToSheet(sheetId, signal);
      throwIfAborted(signal);
      latencies.push(Math.max(0, rendered.renderedAt - requestedAt));
      maxResidentSheetCount = Math.max(
        maxResidentSheetCount,
        rendered.residentSheetCount,
      );
      maxResidentTextureCount = Math.max(
        maxResidentTextureCount,
        rendered.residentTextureCount,
      );
      maxResidentTexturePixelCount = Math.max(
        maxResidentTexturePixelCount,
        rendered.residentTexturePixelCount,
      );
    }
  }

  return {
    sheetCount: target.sheetIds.length,
    cycleCount: config.cycles,
    targetSheetIds: [firstSheetId, middleSheetId, lastSheetId],
    maxResidentSheetCount,
    maxResidentTextureCount,
    maxResidentTexturePixelCount,
    timings: summarizeFrameTimings(latencies),
  };
}

function validateConfig(
  config: CanvasNavigationPerformanceProbeConfig,
  target: CanvasNavigationPerformanceTarget,
) {
  if (
    !Number.isInteger(config.cycles) ||
    config.cycles < 1 ||
    config.cycles > 100
  ) {
    throw new Error(
      "Configuração inválida do probe de navegação do Canvas.",
    );
  }
  if (target.sheetIds.length < MINIMUM_LONG_ALBUM_SHEETS) {
    throw new Error(
      "O probe de navegação exige um Álbum com ao menos 100 Lâminas.",
    );
  }
}

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new DOMException("O probe do Canvas foi cancelado.", "AbortError");
  }
}
