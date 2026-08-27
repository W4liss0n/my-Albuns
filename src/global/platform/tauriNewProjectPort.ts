import { invoke } from "@tauri-apps/api/core";

import type {
  NewProjectPort,
  ProjectLaunchFailure,
} from "../application/globalProjectPort";
import {
  settleConfigurationValidation,
  settleProjectLaunch,
  toProjectLaunchFailure,
  toProvisionalDecorativeSelection,
} from "./projectLaunchBridge";

const createFallbackFailure: ProjectLaunchFailure = {
  code: "create_project_unavailable",
  message: "Não foi possível iniciar a criação do Projeto.",
  action: "Tente novamente. Se o problema continuar, reinicie o MyAlbuns.",
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

export const tauriNewProjectPort: NewProjectPort = {
  validateProjectConfiguration: (configuration) =>
    settleConfigurationValidation(
      () =>
        invoke<unknown>("validate_project_configuration", {
          configuration,
        }),
      validationFallbackFailure,
    ),
  createProject: (configuration) =>
    settleProjectLaunch(
      () => invoke<unknown>("create_project", { configuration }),
      createFallbackFailure,
    ),
  chooseProvisionalDecorative: async () => {
    try {
      const result = await invoke<unknown>("choose_provisional_decorative");
      if (result === null) return { status: "cancelled" };
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
      // The process-scoped registry is also cleared when this window closes.
    }
  },
  clearProvisionalDecoratives: async () => {
    try {
      await invoke<void>("clear_provisional_decoratives");
    } catch {
      // The process-scoped registry is also cleared when this window closes.
    }
  },
};
