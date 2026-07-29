import { invoke } from "@tauri-apps/api/core";

import type {
  TopologyBenchmarkBridge,
  TopologyBenchmarkConfig,
} from "../application/topologyBenchmark";

export const tauriTopologyBenchmarkBridge: TopologyBenchmarkBridge = {
  loadConfig: () =>
    invoke<TopologyBenchmarkConfig | null>(
      "topology_benchmark_config",
    ),
  reportCanvasReady: () =>
    invoke("report_topology_canvas_ready"),
  reportCanvas: (measurement) =>
    invoke("report_topology_canvas_benchmark", { measurement }),
  reportFailure: (reason) =>
    invoke("report_topology_benchmark_failure", { reason }),
};
