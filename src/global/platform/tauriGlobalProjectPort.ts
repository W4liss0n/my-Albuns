import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  GlobalProjectPort,
  ProjectLaunchFailure,
  ProjectLaunchOutcome,
  RecentProjectSummary,
  RecentProjectFirstSheet,
} from "../application/globalProjectPort";
import { hasOnlyIpcKeys, isIpcRecord } from "../../platform/ipcGuards";
import {
  parseProjectLaunchOutcome,
  settleProjectLaunch,
  toProjectLaunchFailure,
  toProjectLaunchOutcome,
} from "./projectLaunchBridge";

export const GLOBAL_ACTIVATION_TERMINAL_EVENT =
  "myalbuns://global-activation-terminal";

const openFallbackFailure: ProjectLaunchFailure = {
  code: "open_project_unavailable",
  message: "Não foi possível iniciar a abertura do projeto.",
  action: "Tente novamente. Se o problema continuar, reinicie o MyAlbuns.",
};

const graphicsGateFallbackFailure: ProjectLaunchFailure = {
  code: "graphics_gate_unavailable",
  message: "Não foi possível confirmar o requisito gráfico do editor.",
  action: "Reinicie o MyAlbuns e tente novamente.",
};

interface GlobalActivationTerminal {
  sequence: number;
  outcome: ProjectLaunchOutcome;
}

function parseActivationTerminal(
  value: unknown,
): GlobalActivationTerminal | null {
  if (
    !isIpcRecord(value) ||
    !hasOnlyIpcKeys(value, ["sequence", "outcome"]) ||
    !Number.isSafeInteger(value.sequence) ||
    Number(value.sequence) <= 0
  ) {
    return null;
  }
  const outcome = parseProjectLaunchOutcome(value.outcome);
  return outcome
    ? { sequence: Number(value.sequence), outcome }
    : null;
}

function toRecentProjectSummaries(
  result: unknown,
): readonly RecentProjectSummary[] {
  if (!Array.isArray(result)) return [];
  return result.flatMap((item) => {
    if (typeof item !== "object" || item === null) return [];
    const candidate = item as Record<string, unknown>;
    if (typeof candidate.id !== "string" || typeof candidate.name !== "string") {
      return [];
    }
    const rawOpenedAt = candidate.lastOpenedAtMs;
    const lastOpenedAtMs = typeof rawOpenedAt === "number" &&
      Number.isSafeInteger(rawOpenedAt) && rawOpenedAt >= 0 &&
      Number.isFinite(new Date(rawOpenedAt).getTime())
      ? rawOpenedAt : null;
    return [{ id: candidate.id, name: candidate.name, lastOpenedAtMs }];
  });
}

export const tauriGlobalProjectPort: GlobalProjectPort = {
  onActivationTerminal: async (listener) => {
    let lastSequence = 0;
    const deliver = (value: unknown) => {
      const terminal = parseActivationTerminal(value);
      if (!terminal || terminal.sequence <= lastSequence) return;
      lastSequence = terminal.sequence;
      listener(terminal.outcome);
    };
    const unlisten = await listen<unknown>(
      GLOBAL_ACTIVATION_TERMINAL_EVENT,
      (event) => deliver(event.payload),
    );
    try {
      deliver(await invoke<unknown>("latest_global_activation_terminal"));
    } catch {
      // The live listener remains authoritative when the snapshot is unavailable.
    }
    return unlisten;
  },
  completeGraphicsGate: async (supported) => {
    try {
      const result = await invoke<unknown>("complete_graphics_gate", {
        report: { status: supported ? "supported" : "unsupported" },
      });
      return result === null
        ? null
        : toProjectLaunchOutcome(result, graphicsGateFallbackFailure);
    } catch (error) {
      return {
        status: "failed",
        error: toProjectLaunchFailure(error, graphicsGateFallbackFailure),
      };
    }
  },
  openProject: () =>
    settleProjectLaunch(
      () => invoke<unknown>("open_project"),
      openFallbackFailure,
    ),
  listRecentProjects: async () => {
    try {
      return toRecentProjectSummaries(
        await invoke<unknown>("recent_projects"),
      );
    } catch {
      return [];
    }
  },
  firstRecentProjectSheet: async (id) => {
    try {
      const result = await invoke<RecentProjectFirstSheet | null>(
        "first_recent_project_sheet", { projectId: id },
      );
      return result;
    } catch {
      return null;
    }
  },
  openRecentProject: (id) =>
    settleProjectLaunch(
      () => invoke<unknown>("open_recent_project", { projectId: id }),
      openFallbackFailure,
    ),
  startupOpenFailure: async () => {
    try {
      const result = await invoke<unknown>("startup_open_failure");
      return result === null
        ? null
        : toProjectLaunchFailure(result, openFallbackFailure);
    } catch {
      return null;
    }
  },
};
