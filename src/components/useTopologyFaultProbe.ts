import { useEffect, useRef } from "react";

import { logReasonFromError } from "../application/logging";
import type { TopologyFaultProbeBridge } from "../application/topologyFaultProbe";
import type { EditorProjection } from "../domain/project";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

const POLL_INTERVAL_MS = 250;
const PROBE_PAN_DELTA = 0.01;

interface TopologyFaultProbeInput {
  projection: EditorProjection | null;
  runProjectMutation: ProjectMutationRunner;
  topologyBridge: TopologyFaultProbeBridge;
  onProjectionChange(projection: EditorProjection): void;
}

interface AttemptedProbeScope {
  projectId: string | null;
  probeIds: Set<string>;
}

export function useTopologyFaultProbe({
  projection,
  runProjectMutation,
  topologyBridge,
  onProjectionChange,
}: TopologyFaultProbeInput) {
  const projectId = projection?.state.projectId ?? null;
  const attemptedRef = useRef<AttemptedProbeScope>({
    projectId,
    probeIds: new Set(),
  });

  if (attemptedRef.current.projectId !== projectId) {
    attemptedRef.current = {
      projectId,
      probeIds: new Set(),
    };
  }

  useEffect(() => {
    if (!topologyBridge.enabled || projectId === null) return;

    let active = true;
    let timer: number | null = null;

    const scheduleNextPoll = () => {
      if (!active) return;
      timer = window.setTimeout(
        () => void poll(),
        POLL_INTERVAL_MS,
      );
    };

    const poll = async () => {
      try {
        const availability = await topologyBridge.loadConfig();
        if (!active) return;
        if (!availability.enabled) return;
        const config = availability.config;
        if (
          config === null ||
          attemptedRef.current.probeIds.has(config.probeId)
        ) {
          scheduleNextPoll();
          return;
        }

        attemptedRef.current.probeIds.add(config.probeId);
        const outcome = await runProjectMutation(async (port) => {
          const canonicalProjection = await port.load(config.probeId);
          const frameId = findFilledFrameId(canonicalProjection);
          if (frameId === null) {
            throw new Error(
              "A projeção atual não possui Frame preenchido.",
            );
          }
          const previousRevision =
            canonicalProjection.state.revision;
          const appliedProjection = await port.apply({
            kind: "transformPhoto",
            frameId,
            deltaPanX: PROBE_PAN_DELTA,
            deltaPanY: 0,
            deltaZoom: 0,
          });
          const result = await topologyBridge.persistAndReport({
            probeId: config.probeId,
            previousRevision,
            expectedRevision:
              appliedProjection.state.revision,
          });
          return result.projection;
        });
        if (outcome.status === "completed" && active) {
          onProjectionChange(outcome.projection);
        } else if (outcome.status === "failed") {
          await reportFailureSafely(topologyBridge, {
            probeId: config.probeId,
            reason: logReasonFromError(outcome.error),
          });
        }
        scheduleNextPoll();
      } catch {
        // The host providing the probe may have terminated.
        scheduleNextPoll();
      }
    };

    void poll();
    return () => {
      active = false;
      if (timer !== null) window.clearTimeout(timer);
    };
  }, [
    onProjectionChange,
    projectId,
    runProjectMutation,
    topologyBridge,
  ]);
}

function findFilledFrameId(
  projection: EditorProjection,
): string | null {
  for (const sheet of projection.state.album.sheets) {
    for (const frame of sheet.frames) {
      if (frame.photo !== null) return frame.id;
    }
  }
  return null;
}

async function reportFailureSafely(
  bridge: TopologyFaultProbeBridge,
  failure: Parameters<TopologyFaultProbeBridge["reportFailure"]>[0],
) {
  try {
    await bridge.reportFailure(failure);
  } catch {
    // The fault under test may also make reporting unavailable.
  }
}
