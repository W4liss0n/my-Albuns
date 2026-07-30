import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";

import type { EditorProjection } from "../domain/project";
import { representativeProjection } from "../test/projectFixtures";
import { tauriTopologyFaultProbeBridge } from "./tauriTopologyFaultProbeBridge";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
}));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

test("maps the topology fault probe bridge to its Tauri commands", async () => {
  const config = {
    probeId: "global-main-down:project-002",
    expectedGlobalAvailable: false,
  };
  const availability = {
    enabled: true,
    config,
  };
  const persisted = {
    projection: representativeProjection as EditorProjection,
    probeId: config.probeId,
    previousRevision: 25,
    persistedRevision: 26,
    bytes: 4_096,
    sha256: "7f83b1657ff1fc53b92dc18148a1d65dfa13514d",
    globalAvailable: false,
    globalProcessId: null,
    globalRoundTripMs: 1.5,
  };
  vi.mocked(invoke)
    .mockResolvedValueOnce(availability)
    .mockResolvedValueOnce(persisted)
    .mockResolvedValueOnce(undefined);

  await expect(
    tauriTopologyFaultProbeBridge.loadConfig(),
  ).resolves.toEqual(availability);
  await expect(
    tauriTopologyFaultProbeBridge.persistAndReport({
      probeId: config.probeId,
      previousRevision: 25,
      expectedRevision: 26,
    }),
  ).resolves.toEqual(persisted);
  await tauriTopologyFaultProbeBridge.reportFailure({
    probeId: config.probeId,
    reason: "project_apply_failed",
  });

  expect(invoke).toHaveBeenNthCalledWith(
    1,
    "topology_fault_probe_config",
  );
  expect(invoke).toHaveBeenNthCalledWith(
    2,
    "persist_topology_fault_probe",
    {
      probeId: config.probeId,
      previousRevision: 25,
      expectedRevision: 26,
    },
  );
  expect(invoke).toHaveBeenNthCalledWith(
    3,
    "report_topology_fault_probe_failure",
    {
      probeId: config.probeId,
      reason: "project_apply_failed",
    },
  );
});
