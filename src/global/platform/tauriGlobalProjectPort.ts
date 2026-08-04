import { invoke } from "@tauri-apps/api/core";

import type {
  GlobalProjectPort,
  ProjectLaunchFailure,
  ProjectLaunchOutcome,
  RecentProjectSummary,
} from "../application/globalProjectPort";

const openFallbackFailure: ProjectLaunchFailure = {
  code: "open_project_unavailable",
  message: "Não foi possível iniciar a abertura do Projeto.",
  action: "Tente novamente. Se o problema continuar, reinicie o MyAlbuns.",
};

const createFallbackFailure: ProjectLaunchFailure = {
  code: "create_project_unavailable",
  message: "Não foi possível iniciar a criação do Projeto.",
  action: "Tente novamente. Se o problema continuar, reinicie o MyAlbuns.",
};

function toProjectLaunchFailure(
  error: unknown,
  fallback: ProjectLaunchFailure,
): ProjectLaunchFailure {
  if (typeof error !== "object" || error === null) {
    return fallback;
  }

  const candidate = error as Record<string, unknown>;
  if (
    typeof candidate.code !== "string" ||
    typeof candidate.message !== "string"
  ) {
    return fallback;
  }

  return {
    code: candidate.code,
    ...(typeof candidate.stage === "string"
      ? { stage: candidate.stage }
      : {}),
    message: candidate.message,
    ...(typeof candidate.action === "string"
      ? { action: candidate.action }
      : {}),
  };
}

function toProjectLaunchOutcome(
  result: unknown,
  fallback: ProjectLaunchFailure,
): ProjectLaunchOutcome {
  if (typeof result !== "object" || result === null) {
    return { status: "failed", error: fallback };
  }

  const candidate = result as Record<string, unknown>;
  if (candidate.status === "opened" || candidate.status === "cancelled") {
    return { status: candidate.status };
  }
  if (candidate.status === "failed") {
    return {
      status: "failed",
      error: toProjectLaunchFailure(candidate.error, fallback),
    };
  }

  return { status: "failed", error: fallback };
}

function toRecentProjectSummaries(
  result: unknown,
): readonly RecentProjectSummary[] {
  if (!Array.isArray(result)) {
    return [];
  }

  return result.flatMap((item) => {
    if (typeof item !== "object" || item === null) {
      return [];
    }
    const candidate = item as Record<string, unknown>;
    if (
      typeof candidate.id !== "string" ||
      typeof candidate.name !== "string"
    ) {
      return [];
    }
    return [{ id: candidate.id, name: candidate.name }];
  });
}

async function settleProjectLaunch(
  attempt: () => Promise<unknown>,
  fallback: ProjectLaunchFailure,
): Promise<ProjectLaunchOutcome> {
  try {
    return toProjectLaunchOutcome(await attempt(), fallback);
  } catch (error) {
    return {
      status: "failed",
      error: toProjectLaunchFailure(error, fallback),
    };
  }
}

export const tauriGlobalProjectPort: GlobalProjectPort = {
  createProject: (preset) =>
    settleProjectLaunch(
      () => invoke<unknown>("create_project", { preset }),
      createFallbackFailure,
    ),
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
};
