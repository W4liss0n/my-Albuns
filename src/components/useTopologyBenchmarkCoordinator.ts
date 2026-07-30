import { useEffect, useMemo, useRef, useState } from "react";

import type {
  TopologyBenchmarkBridge,
  TopologyBenchmarkConfig,
} from "../application/topologyBenchmark";
import { logReasonFromError } from "../application/logging";
import type { ExportPort } from "../application/projectPorts";
import type { CanvasPerformanceProbeRequest } from "./albumCanvasContract";

const GATE_POLL_INTERVAL_MS = 250;

interface TopologyBenchmarkCoordinatorInput {
  projectId: string;
  exportPort: ExportPort;
  topologyBridge: TopologyBenchmarkBridge;
  mediaPreviewsReady: boolean;
}

export function useTopologyBenchmarkCoordinator({
  projectId,
  exportPort,
  topologyBridge,
  mediaPreviewsReady,
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
    if (!config || !mediaPreviewsReady) return null;
    const key = `${projectId}:${config.probeKey}`;
    return {
      key,
      config: {
        warmupFrames: config.warmupFrames,
        panFrames: config.panFrames,
        zoomFrames: config.zoomFrames,
      },
      onReady: () => topologyBridge.reportCanvasReady(),
      onCompleted: async (measurement) => {
        if (completedKeysRef.current.has(key)) return;
        completedKeysRef.current.add(key);
        await topologyBridge.reportCanvas(measurement);
        if (config.runExport) {
          await waitForExportGate(
            topologyBridge,
            config.probeKey,
          );
          await exportPort.exportPreview();
        }
      },
      onFailed: (reason) =>
        reportFailureSafely(topologyBridge, reason),
    };
  }, [
    config,
    mediaPreviewsReady,
    exportPort,
    projectId,
    topologyBridge,
  ]);
}

async function waitForExportGate(
  bridge: TopologyBenchmarkBridge,
  probeKey: string,
) {
  const deadline = performance.now() + 5 * 60 * 1_000;
  while (performance.now() < deadline) {
    const config = await bridge.loadConfig();
    if (!config || config.probeKey !== probeKey) {
      throw new Error(
        "A configuração do benchmark mudou antes da Exportação.",
      );
    }
    if (config.exportGateOpen) return;
    await new Promise<void>((resolve) => {
      window.setTimeout(resolve, GATE_POLL_INTERVAL_MS);
    });
  }
  throw new Error(
    "O gate da Exportação não foi aberto dentro do limite.",
  );
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
