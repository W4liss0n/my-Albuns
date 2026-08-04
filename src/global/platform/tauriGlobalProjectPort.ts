import { invoke } from "@tauri-apps/api/core";

import type {
  GlobalProjectPort,
  OpenProjectFailure,
  OpenProjectOutcome,
  RecentProjectSummary,
} from "../application/globalProjectPort";

const fallbackFailure: OpenProjectFailure = {
  code: "open_project_unavailable",
  message: "Não foi possível iniciar a abertura do Projeto.",
  action: "Tente novamente. Se o problema continuar, reinicie o MyAlbuns.",
};

function toOpenProjectFailure(error: unknown): OpenProjectFailure {
  if (typeof error !== "object" || error === null) {
    return fallbackFailure;
  }

  const candidate = error as Record<string, unknown>;
  if (
    typeof candidate.code !== "string" ||
    typeof candidate.message !== "string"
  ) {
    return fallbackFailure;
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

function toOpenProjectOutcome(result: unknown): OpenProjectOutcome {
  if (typeof result !== "object" || result === null) {
    return { status: "failed", error: fallbackFailure };
  }

  const candidate = result as Record<string, unknown>;
  if (candidate.status === "opened" || candidate.status === "cancelled") {
    return { status: candidate.status };
  }
  if (candidate.status === "failed") {
    return {
      status: "failed",
      error: toOpenProjectFailure(candidate.error),
    };
  }

  return { status: "failed", error: fallbackFailure };
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

async function settleProjectOpening(
  attempt: () => Promise<unknown>,
): Promise<OpenProjectOutcome> {
  try {
    return toOpenProjectOutcome(await attempt());
  } catch (error) {
    return {
      status: "failed",
      error: toOpenProjectFailure(error),
    };
  }
}

export const tauriGlobalProjectPort: GlobalProjectPort = {
  openProject: () =>
    settleProjectOpening(() => invoke<unknown>("open_project")),
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
    settleProjectOpening(() =>
      invoke<unknown>("open_recent_project", { projectId: id }),
    ),
  startupOpenFailure: async () => {
    try {
      const result = await invoke<unknown>("startup_open_failure");
      return result === null ? null : toOpenProjectFailure(result);
    } catch {
      return null;
    }
  },
};
