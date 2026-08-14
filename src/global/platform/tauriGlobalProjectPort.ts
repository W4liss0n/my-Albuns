import { invoke } from "@tauri-apps/api/core";

import type {
  GlobalProjectPort,
  ProjectLaunchFailure,
  RecentProjectSummary,
} from "../application/globalProjectPort";
import {
  settleProjectLaunch,
  toProjectLaunchFailure,
  toProjectLaunchOutcome,
} from "./projectLaunchBridge";

const openFallbackFailure: ProjectLaunchFailure = {
  code: "open_project_unavailable",
  message: "Não foi possível iniciar a abertura do Projeto.",
  action: "Tente novamente. Se o problema continuar, reinicie o MyAlbuns.",
};

const graphicsGateFallbackFailure: ProjectLaunchFailure = {
  code: "graphics_gate_unavailable",
  message: "Não foi possível confirmar o requisito gráfico do editor.",
  action: "Reinicie o MyAlbuns e tente novamente.",
};

function toRecentProjectSummaries(
  result: unknown,
): readonly RecentProjectSummary[] {
  if (!Array.isArray(result)) return [];
  return result.flatMap((item) => {
    if (typeof item !== "object" || item === null) return [];
    const candidate = item as Record<string, unknown>;
    return typeof candidate.id === "string" &&
      typeof candidate.name === "string"
      ? [{ id: candidate.id, name: candidate.name }]
      : [];
  });
}

export const tauriGlobalProjectPort: GlobalProjectPort = {
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
  showLaunchFailure: async (error) => {
    try {
      await invoke<void>("show_project_failure_dialog", { error });
    } catch {
      // Logging and the safe fallback remain owned by the native host.
    }
  },
};
