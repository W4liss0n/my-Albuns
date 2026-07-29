import { expect, test, vi } from "vitest";

import {
  runCanvasPerformanceProbe,
  type CanvasPerformanceClock,
  type CanvasPerformanceTarget,
} from "./canvasPerformanceProbe";

function clockWithFrameLatencies(
  latencies: readonly number[],
): CanvasPerformanceClock {
  let now = 0;
  let index = 0;
  return {
    now: () => now,
    nextFrame: async () => {
      now += latencies[index] ?? 16;
      index += 1;
      return now;
    },
  };
}

test("measures Pan and Zoom frame latency through one texture-backed Canvas target", async () => {
  const previewPan = vi.fn();
  const previewZoom = vi.fn();
  const reset = vi.fn();
  const target: CanvasPerformanceTarget = {
    frameId: "frame-01-a",
    textureBacked: true,
    previewPan,
    previewZoom,
    reset,
  };
  const clock = clockWithFrameLatencies([
    16,
    10,
    20,
    30,
    8,
    16,
    24,
  ]);

  const measurement = await runCanvasPerformanceProbe(
    {
      warmupFrames: 1,
      panFrames: 3,
      zoomFrames: 3,
    },
    target,
    clock,
  );

  expect(previewPan).toHaveBeenCalledTimes(4);
  expect(previewZoom).toHaveBeenCalledTimes(3);
  expect(reset).toHaveBeenCalledTimes(2);
  expect(measurement).toEqual({
    frameId: "frame-01-a",
    textureBacked: true,
    pan: {
      sampleCount: 3,
      durationMs: 60,
      firstFrameLatencyMs: 10,
      meanFrameMs: 20,
      p50FrameMs: 20,
      p95FrameMs: 30,
      p99FrameMs: 30,
      maxFrameMs: 30,
      framesOver16Ms: 2,
      framesOver33Ms: 0,
    },
    zoom: {
      sampleCount: 3,
      durationMs: 48,
      firstFrameLatencyMs: 8,
      meanFrameMs: 16,
      p50FrameMs: 16,
      p95FrameMs: 24,
      p99FrameMs: 24,
      maxFrameMs: 24,
      framesOver16Ms: 1,
      framesOver33Ms: 0,
    },
  });
});

test("refuses to report a Canvas target that is not backed by a real preview texture", async () => {
  const target: CanvasPerformanceTarget = {
    frameId: "frame-01-a",
    textureBacked: false,
    previewPan: vi.fn(),
    previewZoom: vi.fn(),
    reset: vi.fn(),
  };

  await expect(
    runCanvasPerformanceProbe(
      {
        warmupFrames: 1,
        panFrames: 2,
        zoomFrames: 2,
      },
      target,
      clockWithFrameLatencies([16, 16, 16, 16, 16]),
    ),
  ).rejects.toThrow("textura real");
});
