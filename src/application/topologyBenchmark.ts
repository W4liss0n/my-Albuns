import type { CanvasPerformanceMeasurement } from "../components/canvasPerformanceProbe";

export interface TopologyBenchmarkConfig {
  probeKey: string;
  gateOpen: boolean;
  warmupFrames: number;
  panFrames: number;
  zoomFrames: number;
  runExport: boolean;
}

export interface TopologyBenchmarkBridge {
  loadConfig(): Promise<TopologyBenchmarkConfig | null>;
  reportCanvas(
    measurement: CanvasPerformanceMeasurement,
  ): Promise<void>;
  reportFailure(reason: string): Promise<void>;
}

export const disabledTopologyBenchmarkBridge: TopologyBenchmarkBridge = {
  loadConfig: async () => null,
  reportCanvas: async () => undefined,
  reportFailure: async () => undefined,
};
