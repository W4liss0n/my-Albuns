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

export interface CanvasInteractionPerformanceMeasurement {
  frameId: string;
  textureBacked: boolean;
  decorativeMediaId: string;
  decorativeTextureBacked: boolean;
  pan: FrameTimingSummary;
  zoom: FrameTimingSummary;
}

export interface CanvasNavigationMeasurement {
  sheetCount: number;
  cycleCount: number;
  targetSheetIds: [string, string, string];
  maxResidentSheetCount: number;
  maxResidentTextureCount: number;
  timings: FrameTimingSummary;
}

export interface CanvasPerformanceMeasurement
  extends CanvasInteractionPerformanceMeasurement {
  navigation: CanvasNavigationMeasurement;
}

export interface TopologyBenchmarkConfig {
  probeKey: string;
  gateOpen: boolean;
  exportGateOpen: boolean;
  warmupFrames: number;
  panFrames: number;
  zoomFrames: number;
  navigationCycles: number;
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
