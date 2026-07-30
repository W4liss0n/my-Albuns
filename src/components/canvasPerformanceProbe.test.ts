import { expect, test, vi } from "vitest";

import {
  runCanvasPerformanceProbe,
  type CanvasPerformanceClock,
  type CanvasPerformanceTarget,
} from "./canvasPerformanceProbe";

function clockWithFrameLatencies(
  latencies: readonly number[],
): {
  clock: CanvasPerformanceClock;
  nextRenderedFrame(): Promise<number>;
} {
  let now = 0;
  let index = 0;
  return {
    clock: {
      now: () => now,
    },
    nextRenderedFrame: async () => {
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
  const renderedFrames = clockWithFrameLatencies([
    16,
    10,
    20,
    30,
    8,
    16,
    24,
  ]);
  const target: CanvasPerformanceTarget = {
    frameId: "frame-01-a",
    textureBacked: true,
    decorativeMediaId: "decorative-overlay",
    decorativeTextureBacked: true,
    testedTexture: {
      mediaId: "decorative-overlay",
      widthPx: 1_600,
      heightPx: 1_200,
    },
    previewPan,
    previewZoom,
    nextRenderedFrame: renderedFrames.nextRenderedFrame,
    reset,
  };

  const measurement = await runCanvasPerformanceProbe(
    {
      warmupFrames: 1,
      panFrames: 3,
      zoomFrames: 3,
    },
    target,
    renderedFrames.clock,
  );

  expect(previewPan).toHaveBeenCalledTimes(4);
  expect(previewZoom).toHaveBeenCalledTimes(3);
  expect(reset).toHaveBeenCalledTimes(2);
  expect(measurement).toEqual({
    frameId: "frame-01-a",
    textureBacked: true,
    decorativeMediaId: "decorative-overlay",
    decorativeTextureBacked: true,
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
  const renderedFrames = clockWithFrameLatencies([
    16,
    16,
    16,
    16,
    16,
  ]);
  const target: CanvasPerformanceTarget = {
    frameId: "frame-01-a",
    textureBacked: false,
    decorativeMediaId: "decorative-overlay",
    decorativeTextureBacked: true,
    testedTexture: {
      mediaId: "decorative-overlay",
      widthPx: 1_600,
      heightPx: 1_200,
    },
    previewPan: vi.fn(),
    previewZoom: vi.fn(),
    nextRenderedFrame: renderedFrames.nextRenderedFrame,
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
      renderedFrames.clock,
    ),
  ).rejects.toThrow("textura real");
});

test("refuses a benchmark without a real Decorative Cache texture", async () => {
  const renderedFrames = clockWithFrameLatencies([16, 16, 16]);
  const target: CanvasPerformanceTarget = {
    frameId: "frame-01-a",
    textureBacked: true,
    decorativeMediaId: "decorative-overlay",
    decorativeTextureBacked: false,
    testedTexture: {
      mediaId: "decorative-overlay",
      widthPx: 1_600,
      heightPx: 1_200,
    },
    previewPan: vi.fn(),
    previewZoom: vi.fn(),
    nextRenderedFrame: renderedFrames.nextRenderedFrame,
    reset: vi.fn(),
  };

  await expect(
    runCanvasPerformanceProbe(
      {
        warmupFrames: 1,
        panFrames: 1,
        zoomFrames: 1,
      },
      target,
      renderedFrames.clock,
    ),
  ).rejects.toThrow("Decorativo");
});

test("includes synchronous preview work and waits for the Pixi-rendered frame", async () => {
  let now = 0;
  const renderedFrames = [5, 7, 11];
  let renderedIndex = 0;
  const target: CanvasPerformanceTarget = {
    frameId: "frame-01-a",
    textureBacked: true,
    decorativeMediaId: "decorative-overlay",
    decorativeTextureBacked: true,
    testedTexture: {
      mediaId: "decorative-overlay",
      widthPx: 1_600,
      heightPx: 1_200,
    },
    previewPan: () => {
      now += 3;
    },
    previewZoom: () => {
      now += 4;
    },
    nextRenderedFrame: async () => {
      now += renderedFrames[renderedIndex] ?? 0;
      renderedIndex += 1;
      return now;
    },
    reset: vi.fn(),
  };

  const measurement = await runCanvasPerformanceProbe(
    {
      warmupFrames: 1,
      panFrames: 1,
      zoomFrames: 1,
    },
    target,
    { now: () => now },
  );

  expect(measurement.pan.firstFrameLatencyMs).toBe(10);
  expect(measurement.zoom.firstFrameLatencyMs).toBe(15);
  expect(renderedIndex).toBe(3);
});
