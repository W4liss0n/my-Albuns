export interface FrameTimingSummary {
  sampleCount: number;
  durationMs: number;
  firstFrameLatencyMs: number;
  meanFrameMs: number;
  p50FrameMs: number;
  p95FrameMs: number;
  p99FrameMs: number;
  maxFrameMs: number;
  framesOver16Ms: number;
  framesOver33Ms: number;
}

export interface CanvasPerformanceMeasurement {
  frameId: string;
  textureBacked: boolean;
  pan: FrameTimingSummary;
  zoom: FrameTimingSummary;
}

export interface TopologyBenchmarkConfig {
  probeKey: string;
  gateOpen: boolean;
  exportGateOpen: boolean;
  warmupFrames: number;
  panFrames: number;
  zoomFrames: number;
  runExport: boolean;
}

export interface TopologyBenchmarkBridge {
  loadConfig(): Promise<TopologyBenchmarkConfig | null>;
  reportCanvasReady(): Promise<void>;
  reportCanvas(
    measurement: CanvasPerformanceMeasurement,
  ): Promise<void>;
  reportFailure(reason: string): Promise<void>;
}

export const disabledTopologyBenchmarkBridge: TopologyBenchmarkBridge = {
  loadConfig: async () => null,
  reportCanvasReady: async () => undefined,
  reportCanvas: async () => undefined,
  reportFailure: async () => undefined,
};
