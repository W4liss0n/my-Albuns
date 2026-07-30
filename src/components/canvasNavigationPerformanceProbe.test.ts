import { expect, test, vi } from "vitest";

import {
  runCanvasNavigationPerformanceProbe,
  type CanvasNavigationPerformanceTarget,
} from "./canvasNavigationPerformanceProbe";

test("measures repeated navigation through the middle, end, and start of a long Album", async () => {
  const sheetIds = Array.from(
    { length: 100 },
    (_, index) => `sheet-${String(index + 1).padStart(3, "0")}`,
  );
  const latencies = [40, 30, 20, 14, 18, 16];
  let now = 0;
  let sampleIndex = 0;
  const navigateToSheet = vi.fn(
    async (
      _sheetId: string,
    ): Promise<{
      renderedAt: number;
      residentSheetCount: number;
      residentTextureCount: number;
      residentTexturePixelCount: number;
    }> => {
      now += latencies[sampleIndex] ?? 16;
      sampleIndex += 1;
      return {
        renderedAt: now,
        residentSheetCount: sampleIndex === 2 ? 8 : 7,
        residentTextureCount: sampleIndex === 5 ? 16 : 14,
        residentTexturePixelCount:
          sampleIndex === 4 ? 24_000_000 : 20_000_000,
      };
    },
  );
  const target: CanvasNavigationPerformanceTarget = {
    sheetIds,
    navigateToSheet,
  };

  const measurement = await runCanvasNavigationPerformanceProbe(
    { cycles: 2 },
    target,
    { now: () => now },
  );

  expect(navigateToSheet.mock.calls.map(([sheetId]) => sheetId)).toEqual([
    "sheet-050",
    "sheet-100",
    "sheet-001",
    "sheet-050",
    "sheet-100",
    "sheet-001",
  ]);
  expect(measurement).toEqual({
    sheetCount: 100,
    cycleCount: 2,
    targetSheetIds: ["sheet-001", "sheet-050", "sheet-100"],
    maxResidentSheetCount: 8,
    maxResidentTextureCount: 16,
    maxResidentTexturePixelCount: 24_000_000,
    timings: {
      sampleCount: 6,
      durationMs: 138,
      firstFrameLatencyMs: 40,
      meanFrameMs: 23,
      p50FrameMs: 18,
      p95FrameMs: 40,
      p99FrameMs: 40,
      maxFrameMs: 40,
      framesOver16Ms: 4,
      framesOver33Ms: 1,
    },
  });
});
