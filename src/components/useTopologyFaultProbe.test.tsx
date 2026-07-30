import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import {
  disabledTopologyFaultProbeBridge,
  type TopologyFaultProbeAvailability,
  type TopologyFaultProbeBridge,
  type TopologyFaultProbeConfig,
  type TopologyFaultProbeResult,
} from "../application/topologyFaultProbe";
import type { ProjectSessionPort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import { representativeProjection } from "../test/projectFixtures";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";
import { useTopologyFaultProbe } from "./useTopologyFaultProbe";

afterEach(() => {
  vi.useRealTimers();
});

test("loads the canonical Project inside the shared queue before editing and persisting", async () => {
  const probeId = "global-main-down:project-spike-001";
  const renderedProjection = withLeadingPlaceholder(
    representativeProjection,
  );
  const canonicalProjection = withState(renderedProjection, {
    revision: 28,
  });
  const appliedProjection = withState(canonicalProjection, {
    revision: 29,
    dirty: true,
  });
  const savedProjection = withState(appliedProjection, {
    savedRevision: 29,
    dirty: false,
  });
  const calls: string[] = [];
  const port: ProjectSessionPort = {
    load: vi.fn(async () => {
      calls.push("load");
      return canonicalProjection;
    }),
    apply: vi.fn(async () => {
      calls.push("apply");
      return appliedProjection;
    }),
    undo: vi.fn(),
    redo: vi.fn(),
  };
  const persistAndReport = vi.fn(
    async (): Promise<TopologyFaultProbeResult> => {
      calls.push("persist");
      return persistedResult(probeId, savedProjection, 28, 29);
    },
  );
  const bridge = topologyBridge(
    async () =>
      available({
        probeId,
        expectedGlobalAvailable: false,
      }),
    persistAndReport,
  );
  const runProjectMutation = mutationRunner(port);
  const onProjectionChange = vi.fn();

  renderHook(() =>
    useTopologyFaultProbe({
      projection: renderedProjection,
      runProjectMutation,
      topologyBridge: bridge,
      onProjectionChange,
    }),
  );

  await act(async () => {
    await vi.waitFor(() => expect(persistAndReport).toHaveBeenCalled());
  });

  expect(runProjectMutation).toHaveBeenCalledOnce();
  expect(port.load).toHaveBeenCalledWith(
    `topology-fault-probe:${probeId}`,
  );
  expect(port.apply).toHaveBeenCalledWith({
    kind: "transformPhoto",
    frameId: "frame-001",
    deltaPanX: 0.01,
    deltaPanY: 0,
    deltaZoom: 0,
  });
  expect(persistAndReport).toHaveBeenCalledWith({
    probeId,
    previousRevision: 28,
    expectedRevision: 29,
  });
  expect(calls).toEqual(["load", "apply", "persist"]);
  expect(onProjectionChange).toHaveBeenCalledWith(savedProjection);
});

test("runs each probeId once and keeps polling for a later probe", async () => {
  vi.useFakeTimers();
  let currentProbeId = "project-host-down:001";
  const applied26 = withState(representativeProjection, {
    revision: 26,
    dirty: true,
  });
  const saved26 = withState(applied26, {
    savedRevision: 26,
    dirty: false,
  });
  const applied27 = withState(saved26, {
    revision: 27,
    dirty: true,
  });
  const saved27 = withState(applied27, {
    savedRevision: 27,
    dirty: false,
  });
  const port: ProjectSessionPort = {
    load: vi
      .fn<ProjectSessionPort["load"]>()
      .mockResolvedValueOnce(representativeProjection)
      .mockResolvedValueOnce(saved26),
    apply: vi
      .fn<ProjectSessionPort["apply"]>()
      .mockResolvedValueOnce(applied26)
      .mockResolvedValueOnce(applied27),
    undo: vi.fn(),
    redo: vi.fn(),
  };
  const persistAndReport = vi
    .fn<TopologyFaultProbeBridge["persistAndReport"]>()
    .mockResolvedValueOnce(
      persistedResult("project-host-down:001", saved26, 25, 26),
    )
    .mockResolvedValueOnce(
      persistedResult("project-host-down:002", saved27, 26, 27),
    );
  const bridge = topologyBridge(
    async () =>
      available({
        probeId: currentProbeId,
        expectedGlobalAvailable: true,
      }),
    persistAndReport,
  );

  renderHook(() =>
    useTopologyFaultProbe({
      projection: representativeProjection,
      runProjectMutation: mutationRunner(port),
      topologyBridge: bridge,
      onProjectionChange: vi.fn(),
    }),
  );

  await act(async () => {
    await vi.waitFor(() => expect(port.apply).toHaveBeenCalledTimes(1));
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(250);
  });
  expect(port.apply).toHaveBeenCalledTimes(1);

  currentProbeId = "project-host-down:002";
  await act(async () => {
    await vi.advanceTimersByTimeAsync(250);
    await vi.waitFor(() =>
      expect(port.apply).toHaveBeenCalledTimes(2),
    );
  });

  expect(persistAndReport).toHaveBeenNthCalledWith(2, {
    probeId: "project-host-down:002",
    previousRevision: 26,
    expectedRevision: 27,
  });
});

test("keeps polling while the shared gate is absent", async () => {
  vi.useFakeTimers();
  const probeId = "global-restarted:001";
  const applied = withState(representativeProjection, {
    revision: 26,
    dirty: true,
  });
  const saved = withState(applied, {
    savedRevision: 26,
    dirty: false,
  });
  const port = projectSessionPort(
    vi.fn(async () => applied),
    representativeProjection,
  );
  const bridge = topologyBridge(
    vi
      .fn<TopologyFaultProbeBridge["loadConfig"]>()
      .mockResolvedValueOnce(available(null))
      .mockResolvedValue(
        available({
          probeId,
          expectedGlobalAvailable: true,
        }),
      ),
    vi.fn(async () =>
      persistedResult(probeId, saved, 25, 26),
    ),
  );

  renderHook(() =>
    useTopologyFaultProbe({
      projection: representativeProjection,
      runProjectMutation: mutationRunner(port),
      topologyBridge: bridge,
      onProjectionChange: vi.fn(),
    }),
  );

  await act(async () => {
    await Promise.resolve();
  });
  expect(port.apply).not.toHaveBeenCalled();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(250);
    await vi.waitFor(() => expect(port.apply).toHaveBeenCalledOnce());
  });
});

test("retries polling after a transient configuration error", async () => {
  vi.useFakeTimers();
  const probeId = "global-restarted:after-transient-error";
  const applied = withState(representativeProjection, {
    revision: 26,
    dirty: true,
  });
  const saved = withState(applied, {
    savedRevision: 26,
    dirty: false,
  });
  const port = projectSessionPort(
    vi.fn(async () => applied),
    representativeProjection,
  );
  const loadConfig = vi
    .fn<TopologyFaultProbeBridge["loadConfig"]>()
    .mockRejectedValueOnce(new Error("host temporarily unavailable"))
    .mockResolvedValue(
      available({
        probeId,
        expectedGlobalAvailable: true,
      }),
    );
  const bridge = topologyBridge(
    loadConfig,
    vi.fn(async () =>
      persistedResult(probeId, saved, 25, 26),
    ),
  );

  renderHook(() =>
    useTopologyFaultProbe({
      projection: representativeProjection,
      runProjectMutation: mutationRunner(port),
      topologyBridge: bridge,
      onProjectionChange: vi.fn(),
    }),
  );

  await act(async () => {
    await Promise.resolve();
  });
  expect(loadConfig).toHaveBeenCalledOnce();
  expect(port.apply).not.toHaveBeenCalled();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(250);
    await vi.waitFor(() => expect(port.apply).toHaveBeenCalledOnce());
  });
  expect(loadConfig).toHaveBeenCalledTimes(2);
});

test("stops polling permanently when the backend disables the probe", async () => {
  vi.useFakeTimers();
  const loadConfig = vi.fn(async () => ({
    enabled: false,
    config: null,
  }));
  const bridge = topologyBridge(loadConfig, vi.fn());
  const runProjectMutation = mutationRunner(
    projectSessionPort(vi.fn(), representativeProjection),
  );

  renderHook(() =>
    useTopologyFaultProbe({
      projection: representativeProjection,
      runProjectMutation,
      topologyBridge: bridge,
      onProjectionChange: vi.fn(),
    }),
  );

  await act(async () => {
    await Promise.resolve();
    await vi.advanceTimersByTimeAsync(1_000);
  });

  expect(loadConfig).toHaveBeenCalledOnce();
  expect(runProjectMutation).not.toHaveBeenCalled();
});

test("reports a failed probe once without retrying the same probeId", async () => {
  vi.useFakeTimers();
  const probeId = "project-host-down:failed";
  const port = projectSessionPort(
    vi.fn(async () => {
      throw new Error("session unavailable");
    }),
    representativeProjection,
  );
  const reportFailure = vi.fn(async () => undefined);
  const bridge: TopologyFaultProbeBridge = {
    enabled: true,
    loadConfig: vi.fn(async () =>
      available({
        probeId,
        expectedGlobalAvailable: true,
      }),
    ),
    persistAndReport: vi.fn(),
    reportFailure,
  };

  renderHook(() =>
    useTopologyFaultProbe({
      projection: representativeProjection,
      runProjectMutation: mutationRunner(port),
      topologyBridge: bridge,
      onProjectionChange: vi.fn(),
    }),
  );

  await act(async () => {
    await vi.waitFor(() =>
      expect(reportFailure).toHaveBeenCalledWith({
        probeId,
        reason: "error",
      }),
    );
    await vi.advanceTimersByTimeAsync(750);
  });

  expect(port.apply).toHaveBeenCalledOnce();
  expect(reportFailure).toHaveBeenCalledOnce();
  expect(bridge.persistAndReport).not.toHaveBeenCalled();
});

test("does nothing when the frontend bridge is disabled", async () => {
  const loadConfig = vi.spyOn(
    disabledTopologyFaultProbeBridge,
    "loadConfig",
  );
  const runProjectMutation = mutationRunner(
    projectSessionPort(vi.fn(), representativeProjection),
  );

  renderHook(() =>
    useTopologyFaultProbe({
      projection: representativeProjection,
      runProjectMutation,
      topologyBridge: disabledTopologyFaultProbeBridge,
      onProjectionChange: vi.fn(),
    }),
  );
  await act(async () => {
    await Promise.resolve();
  });

  expect(loadConfig).not.toHaveBeenCalled();
  expect(runProjectMutation).not.toHaveBeenCalled();
});

test("waits for a loaded Project before polling", async () => {
  const loadConfig = vi.fn<
    TopologyFaultProbeBridge["loadConfig"]
  >();
  const bridge: TopologyFaultProbeBridge = {
    enabled: true,
    loadConfig,
    persistAndReport: vi.fn(),
    reportFailure: vi.fn(),
  };
  const runProjectMutation = mutationRunner(
    projectSessionPort(vi.fn(), representativeProjection),
  );

  renderHook(() =>
    useTopologyFaultProbe({
      projection: null,
      runProjectMutation,
      topologyBridge: bridge,
      onProjectionChange: vi.fn(),
    }),
  );
  await act(async () => {
    await Promise.resolve();
  });

  expect(loadConfig).not.toHaveBeenCalled();
  expect(runProjectMutation).not.toHaveBeenCalled();
});

function available(
  config: TopologyFaultProbeConfig | null,
): TopologyFaultProbeAvailability {
  return { enabled: true, config };
}

function topologyBridge(
  loadConfig: TopologyFaultProbeBridge["loadConfig"],
  persistAndReport: TopologyFaultProbeBridge["persistAndReport"],
): TopologyFaultProbeBridge {
  return {
    enabled: true,
    loadConfig,
    persistAndReport,
    reportFailure: vi.fn(async () => undefined),
  };
}

function mutationRunner(
  port: ProjectSessionPort,
): ProjectMutationRunner {
  return vi.fn<ProjectMutationRunner>(async (operation) => {
    try {
      return {
        status: "completed",
        projection: await operation(port),
      };
    } catch (error: unknown) {
      return { status: "failed", error };
    }
  });
}

function projectSessionPort(
  apply: ProjectSessionPort["apply"],
  projection: EditorProjection,
): ProjectSessionPort {
  return {
    load: vi.fn(async () => projection),
    apply,
    undo: vi.fn(),
    redo: vi.fn(),
  };
}

function withLeadingPlaceholder(
  projection: EditorProjection,
): EditorProjection {
  const firstSheet = projection.state.album.sheets[0];
  return {
    ...projection,
    state: {
      ...projection.state,
      album: {
        ...projection.state.album,
        sheets: [
          {
            ...firstSheet,
            frames: [
              {
                ...firstSheet.frames[0],
                id: "placeholder-001",
                photo: null,
              },
              ...firstSheet.frames,
            ],
          },
        ],
      },
    },
  };
}

function withState(
  projection: EditorProjection,
  state: Partial<EditorProjection["state"]>,
): EditorProjection {
  return {
    ...projection,
    state: {
      ...projection.state,
      ...state,
    },
  };
}

function persistedResult(
  probeId: string,
  projection: EditorProjection,
  previousRevision: number,
  persistedRevision: number,
): TopologyFaultProbeResult {
  return {
    projection,
    probeId,
    previousRevision,
    persistedRevision,
    bytes: 4_096,
    sha256: "7f83b1657ff1fc53b92dc18148a1d65dfa13514d",
    globalAvailable: true,
    globalProcessId: 1_234,
    globalRoundTripMs: 1.5,
  };
}
