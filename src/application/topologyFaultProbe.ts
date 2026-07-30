import type { EditorProjection } from "../domain/project";

export interface TopologyFaultProbeConfig {
  probeId: string;
  expectedGlobalAvailable: boolean;
}

export interface TopologyFaultProbeAvailability {
  enabled: boolean;
  config: TopologyFaultProbeConfig | null;
}

export interface PersistTopologyFaultProbeRequest {
  probeId: string;
  previousRevision: number;
  expectedRevision: number;
}

export interface TopologyFaultProbeResult {
  projection: EditorProjection;
  probeId: string;
  previousRevision: number;
  persistedRevision: number;
  bytes: number;
  sha256: string;
  globalAvailable: boolean;
  globalProcessId: number | null;
  globalRoundTripMs: number;
}

export interface TopologyFaultProbeFailure {
  probeId: string;
  reason: string;
}

export interface TopologyFaultProbeBridge {
  enabled: boolean;
  loadConfig(): Promise<TopologyFaultProbeAvailability>;
  persistAndReport(
    request: PersistTopologyFaultProbeRequest,
  ): Promise<TopologyFaultProbeResult>;
  reportFailure(failure: TopologyFaultProbeFailure): Promise<void>;
}

export const disabledTopologyFaultProbeBridge: TopologyFaultProbeBridge = {
  enabled: false,
  loadConfig: async () => ({ enabled: false, config: null }),
  persistAndReport: async () => {
    throw new Error("O probe de continuidade está desabilitado.");
  },
  reportFailure: async () => undefined,
};
