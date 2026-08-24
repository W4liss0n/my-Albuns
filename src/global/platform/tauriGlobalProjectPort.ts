import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import {
  PROJECT_CONFIGURATION_VALIDATION_CODES,
  type GlobalProjectPort,
  type ProjectConfigurationValidationCode,
  type ProjectConfigurationValidationOutcome,
  type ProjectLaunchFailure,
  type ProjectLaunchOutcome,
  type ProvisionalDecorativeSelection,
  type RecentProjectSummary,
} from "../application/globalProjectPort";
import { hasOnlyIpcKeys, isIpcRecord } from "../../platform/ipcGuards";

export const GLOBAL_ACTIVATION_TERMINAL_EVENT =
  "myalbuns://global-activation-terminal";

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

const saveCopyFallbackFailure: ProjectLaunchFailure = {
  code: "save_copy_unavailable",
  message: "Não foi possível salvar a Cópia externa.",
  action: "Tente novamente. Se o problema continuar, reabra a cópia.",
};

const validationFallbackFailure: ProjectLaunchFailure = {
  code: "project_configuration_validation_unavailable",
  message: "Não foi possível validar as Dimensões do Projeto.",
  action: "Tente novamente. Se o problema continuar, reinicie o MyAlbuns.",
};

const decorativePickerFallbackFailure: ProjectLaunchFailure = {
  code: "decorative_picker_unavailable",
  message: "Não foi possível concluir o seletor de Imagem decorativa.",
  action: "Tente novamente.",
};

const graphicsGateFallbackFailure: ProjectLaunchFailure = {
  code: "graphics_gate_unavailable",
  message: "Não foi possível confirmar o requisito gráfico do editor.",
  action: "Reinicie o MyAlbuns e tente novamente.",
};

const validationCodes = new Set<ProjectConfigurationValidationCode>(
  PROJECT_CONFIGURATION_VALIDATION_CODES,
);

function parseProjectLaunchFailure(
  error: unknown,
): ProjectLaunchFailure | null {
  if (typeof error !== "object" || error === null) {
    return null;
  }

  const candidate = error as Record<string, unknown>;
  if (
    typeof candidate.code !== "string" ||
    typeof candidate.message !== "string"
  ) {
    return null;
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
  return parseProjectLaunchOutcome(result) ?? {
    status: "failed",
    error: fallback,
  };
}

interface GlobalActivationTerminal {
  sequence: number;
  outcome: ProjectLaunchOutcome;
}

function toProjectLaunchFailure(
  error: unknown,
  fallback: ProjectLaunchFailure,
): ProjectLaunchFailure {
  return parseProjectLaunchFailure(error) ?? fallback;
}

function parseProjectLaunchOutcome(
  value: unknown,
): ProjectLaunchOutcome | null {
  if (!isIpcRecord(value) || typeof value.status !== "string") {
    return null;
  }
  if (
    (value.status === "opened" ||
      value.status === "focused" ||
      value.status === "externalCopyNotWritable" ||
      value.status === "cancelled") &&
    hasOnlyIpcKeys(value, ["status"])
  ) {
    return { status: value.status };
  }
  if (
    value.status === "failed" &&
    hasOnlyIpcKeys(value, ["status", "error"])
  ) {
    const error = parseProjectLaunchFailure(value.error);
    return error ? { status: "failed", error } : null;
  }
  return null;
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

function toProvisionalDecorativeSelection(
  result: unknown,
): ProvisionalDecorativeSelection | null {
  if (result === null) return null;
  if (typeof result !== "object") return null;
  const candidate = result as Record<string, unknown>;
  if (
    typeof candidate.selectionId !== "string" ||
    candidate.selectionId.length === 0 ||
    candidate.selectionId.includes("/") ||
    typeof candidate.displayName !== "string" ||
    candidate.displayName.length === 0 ||
    typeof candidate.previewUrl !== "string" ||
    !(
      candidate.previewUrl.startsWith(
        "http://myalbuns-preview.localhost/",
      ) ||
      candidate.previewUrl.startsWith("myalbuns-preview://localhost/")
    )
  ) {
    return null;
  }
  return {
    selectionId: candidate.selectionId,
    displayName: candidate.displayName,
    previewUrl: candidate.previewUrl,
  };
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

async function validateProjectConfiguration(
  attempt: () => Promise<unknown>,
): Promise<ProjectConfigurationValidationOutcome> {
  try {
    const result = await attempt();
    if (typeof result !== "object" || result === null) {
      return { status: "failed", error: validationFallbackFailure };
    }
    const candidate = result as Record<string, unknown>;
    if (!Array.isArray(candidate.errors)) {
      return { status: "failed", error: validationFallbackFailure };
    }

    const errors = candidate.errors.flatMap((code) => {
      return typeof code === "string" &&
        validationCodes.has(code as ProjectConfigurationValidationCode)
        ? [code as ProjectConfigurationValidationCode]
        : [];
    });
    if (errors.length !== candidate.errors.length) {
      return { status: "failed", error: validationFallbackFailure };
    }
    return errors.length === 0
      ? { status: "valid" }
      : { status: "invalid", errors };
  } catch (error) {
    return {
      status: "failed",
      error: toProjectLaunchFailure(error, validationFallbackFailure),
    };
  }
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
  validateProjectConfiguration: (configuration) =>
    validateProjectConfiguration(() =>
      invoke<unknown>("validate_project_configuration", { configuration }),
    ),
  createProject: (configuration) =>
    settleProjectLaunch(
      () => invoke<unknown>("create_project", { configuration }),
      createFallbackFailure,
    ),
  chooseProvisionalDecorative: async () => {
    try {
      const result = await invoke<unknown>(
        "choose_provisional_decorative",
      );
      if (result === null) {
        return { status: "cancelled" };
      }
      const selection = toProvisionalDecorativeSelection(result);
      return selection
        ? { status: "selected", selection }
        : { status: "failed", error: decorativePickerFallbackFailure };
    } catch (error) {
      return {
        status: "failed",
        error: toProjectLaunchFailure(
          error,
          decorativePickerFallbackFailure,
        ),
      };
    }
  },
  releaseProvisionalDecorative: async (selectionId) => {
    try {
      await invoke<unknown>("release_provisional_decorative", {
        selectionId,
      });
    } catch {
      // The process-scoped registry is also discarded when Global exits.
    }
  },
  clearProvisionalDecoratives: async () => {
    try {
      await invoke<unknown>("clear_provisional_decoratives");
    } catch {
      // A fresh Global process starts with an empty registry.
    }
  },
  openProject: () =>
    settleProjectLaunch(
      () => invoke<unknown>("open_project"),
      openFallbackFailure,
    ),
  saveExternalCopyAs: () =>
    settleProjectLaunch(
      () => invoke<unknown>("save_external_copy_as"),
      saveCopyFallbackFailure,
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
