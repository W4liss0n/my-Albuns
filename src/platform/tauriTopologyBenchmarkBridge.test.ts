import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";

import type { CanvasPerformanceMeasurement } from "../components/canvasPerformanceProbe";
import { tauriTopologyBenchmarkBridge } from "./tauriTopologyBenchmarkBridge";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
}));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

test("maps the isolated topology benchmark bridge to its Tauri commands", async () => {
  const config = {
    probeKey: "independent-main",
    gateOpen: true,
    warmupFrames: 24,
    panFrames: 120,
    zoomFrames: 120,
    runExport: true,
  };
  const measurement: CanvasPerformanceMeasurement = {
    frameId: "frame-01-a",
    textureBacked: true,
    pan: {
      sampleCount: 120,
      durationMs: 2000,
      firstFrameLatencyMs: 16,
      meanFrameMs: 16.667,
      p50FrameMs: 16.5,
      p95FrameMs: 18,
      p99FrameMs: 24,
      maxFrameMs: 28,
      framesOver16Ms: 30,
      framesOver33Ms: 0,
    },
    zoom: {
      sampleCount: 120,
      durationMs: 2050,
      firstFrameLatencyMs: 17,
      meanFrameMs: 17.083,
      p50FrameMs: 16.7,
      p95FrameMs: 20,
      p99FrameMs: 26,
      maxFrameMs: 31,
      framesOver16Ms: 42,
      framesOver33Ms: 0,
    },
  };
  vi.mocked(invoke).mockResolvedValueOnce(config);

  await expect(
    tauriTopologyBenchmarkBridge.loadConfig(),
  ).resolves.toEqual(config);
  await tauriTopologyBenchmarkBridge.reportCanvas(measurement);
  await tauriTopologyBenchmarkBridge.reportFailure("probe_failed");

  expect(invoke).toHaveBeenNthCalledWith(
    1,
    "topology_benchmark_config",
  );
  expect(invoke).toHaveBeenNthCalledWith(
    2,
    "report_topology_canvas_benchmark",
    { measurement },
  );
  expect(invoke).toHaveBeenNthCalledWith(
    3,
    "report_topology_benchmark_failure",
    { reason: "probe_failed" },
  );
});
