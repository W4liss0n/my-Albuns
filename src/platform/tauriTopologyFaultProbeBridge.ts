import { invoke } from "@tauri-apps/api/core";

import type {
  TopologyFaultProbeBridge,
  TopologyFaultProbeAvailability,
  TopologyFaultProbeResult,
} from "../application/topologyFaultProbe";

export const tauriTopologyFaultProbeBridge: TopologyFaultProbeBridge = {
  enabled: true,
  loadConfig: () =>
    invoke<TopologyFaultProbeAvailability>(
      "topology_fault_probe_config",
    ),
  persistAndReport: (request) =>
    invoke<TopologyFaultProbeResult>(
      "persist_topology_fault_probe",
      {
        probeId: request.probeId,
        previousRevision: request.previousRevision,
        expectedRevision: request.expectedRevision,
      },
    ),
  reportFailure: (failure) =>
    invoke("report_topology_fault_probe_failure", {
      probeId: failure.probeId,
      reason: failure.reason,
    }),
};
