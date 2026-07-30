import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type {
  CanvasPerformanceMeasurement,
  TopologyBenchmarkBridge,
  TopologyBenchmarkConfig,
} from "../application/topologyBenchmark";
import type { ExportPort } from "../application/projectPorts";
import { useTopologyBenchmarkCoordinator } from "./useTopologyBenchmarkCoordinator";

const measurement: CanvasPerformanceMeasurement = {
  frameId: "frame-01-a",
  textureBacked: true,
  decorativeMediaId: "decorative-overlay",
  decorativeTextureBacked: true,
  pan: {
    sampleCount: 1,
    durationMs: 12,
    firstFrameLatencyMs: 12,
    meanFrameMs: 12,
    p50FrameMs: 12,
    p95FrameMs: 12,
    p99FrameMs: 12,
    maxFrameMs: 12,
    framesOver16Ms: 0,
    framesOver33Ms: 0,
  },
  zoom: {
    sampleCount: 1,
    durationMs: 14,
    firstFrameLatencyMs: 14,
    meanFrameMs: 14,
    p50FrameMs: 14,
    p95FrameMs: 14,
    p99FrameMs: 14,
    maxFrameMs: 14,
    framesOver16Ms: 0,
    framesOver33Ms: 0,
  },
  navigation: {
    sheetCount: 100,
    cycleCount: 1,
    targetSheetIds: ["lamina-01", "lamina-50", "lamina-100"],
    maxResidentSheetCount: 8,
    maxResidentTextureCount: 16,
    maxResidentTexturePixelCount: 30_720_000,
    timings: {
      sampleCount: 3,
      durationMs: 72,
      firstFrameLatencyMs: 24,
      meanFrameMs: 24,
      p50FrameMs: 24,
      p95FrameMs: 24,
      p99FrameMs: 24,
      maxFrameMs: 24,
      framesOver16Ms: 3,
      framesOver33Ms: 0,
    },
  },
  graphics: {
    webGlVersion: 2,
    maxTextureSizePx: 16_384,
    maxRenderbufferSizePx: 16_384,
    maxTextureImageUnits: 16,
    testedTexture: {
      mediaId: "decorative-overlay",
      widthPx: 1_600,
      heightPx: 1_200,
    },
    contextRecovery: {
      mechanism: "webgl_lose_context",
      contextLost: true,
      contextRestored: true,
      recoveryDurationMs: 35,
      restoredFrameLatencyMs: 12,
      glError: 0,
      textureBacked: true,
      decorativeTextureBacked: true,
    },
  },
};

const baseConfig: TopologyBenchmarkConfig = {
  probeKey: "topology-probe",
  gateOpen: true,
  exportGateOpen: false,
  warmupFrames: 1,
  panFrames: 1,
  zoomFrames: 1,
  navigationCycles: 1,
  runExport: true,
};

function exportPort(
  exportPreview: ExportPort["exportPreview"],
): ExportPort {
  return {
    exportPreview,
  };
}

afterEach(() => {
  vi.useRealTimers();
});

test("waits until every Canvas probe reached the export barrier", async () => {
  vi.useFakeTimers();
  let exportGateOpen = false;
  const reportCanvasReady = vi.fn(async () => undefined);
  const reportCanvas = vi.fn(async () => undefined);
  const exportPreview = vi.fn(async () => ({
    outputPath: "C:\\Temp\\Album-Horizonte_001.png",
    widthPx: 600,
    heightPx: 300,
  }));
  const topologyBridge: TopologyBenchmarkBridge = {
    loadConfig: vi.fn(async () => ({
      ...baseConfig,
      exportGateOpen,
    })),
    reportCanvasReady,
    reportCanvas,
    reportFailure: vi.fn(async () => undefined),
  };

  const { result } = renderHook(() =>
    useTopologyBenchmarkCoordinator({
      projectId: "project-spike-001",
      exportPort: exportPort(exportPreview),
      topologyBridge,
      mediaPreviewsReady: true,
    }),
  );

  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
  const probe = result.current;
  expect(probe).not.toBeNull();

  await act(async () => {
    await probe?.onReady();
  });
  expect(reportCanvasReady).toHaveBeenCalledOnce();

  let completion: Promise<void> | undefined;
  await act(async () => {
    completion = Promise.resolve(probe?.onCompleted(measurement));
    await Promise.resolve();
    await Promise.resolve();
  });

  expect(reportCanvas).toHaveBeenCalledWith(measurement);
  expect(exportPreview).not.toHaveBeenCalled();

  exportGateOpen = true;
  await act(async () => {
    await vi.advanceTimersByTimeAsync(250);
    await completion;
  });

  expect(exportPreview).toHaveBeenCalledOnce();
});

test("does not expose the probe before media previews are ready", async () => {
  const topologyBridge: TopologyBenchmarkBridge = {
    loadConfig: vi.fn(async () => baseConfig),
    reportCanvasReady: vi.fn(async () => undefined),
    reportCanvas: vi.fn(async () => undefined),
    reportFailure: vi.fn(async () => undefined),
  };

  const { result } = renderHook(() =>
    useTopologyBenchmarkCoordinator({
      projectId: "project-spike-001",
      exportPort: exportPort(vi.fn()),
      topologyBridge,
      mediaPreviewsReady: false,
    }),
  );

  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });

  expect(result.current).toBeNull();
});
