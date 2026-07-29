import { useEffect, useMemo, useRef, useState } from "react";

import type {
  TopologyBenchmarkBridge,
  TopologyBenchmarkConfig,
} from "../application/topologyBenchmark";
import { logReasonFromError } from "../application/logging";
import type { ProjectBridge } from "../domain/project";
import type { CanvasPerformanceProbeRequest } from "./albumCanvasContract";

const GATE_POLL_INTERVAL_MS = 250;

interface TopologyBenchmarkCoordinatorInput {
  projectId: string;
  projectBridge: ProjectBridge;
  topologyBridge: TopologyBenchmarkBridge;
}

export function useTopologyBenchmarkCoordinator({
  projectId,
  projectBridge,
  topologyBridge,
}: TopologyBenchmarkCoordinatorInput) {
  const [config, setConfig] =
    useState<TopologyBenchmarkConfig | null>(null);
  const completedKeysRef = useRef(new Set<string>());

  useEffect(() => {
    let active = true;
    let timer: number | null = null;
    setConfig(null);
    completedKeysRef.current.clear();
    if (!projectId) {
      return () => {
        active = false;
      };
    }

    const poll = async () => {
      try {
        const next = await topologyBridge.loadConfig();
        if (!active || next === null) return;
        if (next.gateOpen) {
          setConfig(next);
          return;
        }
        timer = window.setTimeout(
          () => void poll(),
          GATE_POLL_INTERVAL_MS,
        );
      } catch (error: unknown) {
        if (active) {
          await reportFailureSafely(
            topologyBridge,
            logReasonFromError(error),
          );
        }
      }
    };

    void poll();
    return () => {
      active = false;
      if (timer !== null) window.clearTimeout(timer);
    };
  }, [projectId, topologyBridge]);

  return useMemo<CanvasPerformanceProbeRequest | null>(() => {
    if (!config) return null;
    const key = `${projectId}:${config.probeKey}`;
    return {
      key,
      config: {
        warmupFrames: config.warmupFrames,
        panFrames: config.panFrames,
        zoomFrames: config.zoomFrames,
      },
      onCompleted: async (measurement) => {
        if (completedKeysRef.current.has(key)) return;
        completedKeysRef.current.add(key);
        await topologyBridge.reportCanvas(measurement);
        if (config.runExport) {
          await projectBridge.exportPreview();
        }
      },
      onFailed: (reason) =>
        reportFailureSafely(topologyBridge, reason),
    };
  }, [config, projectBridge, projectId, topologyBridge]);
}

async function reportFailureSafely(
  bridge: TopologyBenchmarkBridge,
  reason: string,
) {
  try {
    await bridge.reportFailure(reason);
  } catch {
    // The host may already be shutting down after the original failure.
  }
}
