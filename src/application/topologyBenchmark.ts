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
  maxResidentTexturePixelCount: number;
  timings: FrameTimingSummary;
}

export interface CanvasTestedTexture {
  mediaId: string;
  widthPx: number;
  heightPx: number;
}

export interface CanvasGraphicsMeasurement {
  webGlVersion: 2;
  maxTextureSizePx: number;
  maxRenderbufferSizePx: number;
  maxTextureImageUnits: number;
  testedTexture: CanvasTestedTexture;
  contextRecovery: {
    mechanism: "webgl_lose_context";
    contextLost: boolean;
    contextRestored: boolean;
    recoveryDurationMs: number;
    restoredFrameLatencyMs: number;
    glError: number;
    textureBacked: boolean;
    decorativeTextureBacked: boolean;
  };
}

export interface CanvasPerformanceMeasurement
  extends CanvasInteractionPerformanceMeasurement {
  navigation: CanvasNavigationMeasurement;
  graphics: CanvasGraphicsMeasurement;
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
