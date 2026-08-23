import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import {
  ProjectCloseError,
  type ProjectCloseChoice,
  type ProjectWindowPort,
} from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import type { ProjectCloseChoice as IpcProjectCloseChoice } from "./generated/ProjectCloseChoice";
import type { ProjectCloseRequestOutcome as IpcProjectCloseRequestOutcome } from "./generated/ProjectCloseRequestOutcome";
import type { ProjectCloseResolution as IpcProjectCloseResolution } from "./generated/ProjectCloseResolution";
import { hasOnlyIpcKeys, isIpcRecord } from "./ipcGuards";
import { parseProjectSaveFailure } from "./projectSaveFailure";

export const PROJECT_CLOSE_CONFIRMATION_EVENT =
  "myalbuns://project-close-confirmation-requested";

function isProjection(value: unknown): value is EditorProjection {
  return (
    isIpcRecord(value) &&
    isIpcRecord(value.state) &&
    typeof value.state.projectId === "string" &&
    Number.isSafeInteger(value.state.revision)
  );
}

function invalidCloseResponse() {
  return new ProjectCloseError(
    "invalid_response",
    "Não foi possível confirmar o estado do fechamento do Projeto.",
  );
}

function parseCloseRequestOutcome(value: unknown): IpcProjectCloseRequestOutcome {
  if (
    !isIpcRecord(value) ||
    !hasOnlyIpcKeys(value, ["kind"]) ||
    (value.kind !== "closed" && value.kind !== "confirmationRequired")
  ) {
    throw invalidCloseResponse();
  }
  return { kind: value.kind };
}

function parseCloseResolution(value: unknown): IpcProjectCloseResolution {
  if (!isIpcRecord(value) || typeof value.kind !== "string") {
    throw invalidCloseResponse();
  }
  if (value.kind === "closed" && hasOnlyIpcKeys(value, ["kind"])) {
    return { kind: "closed" };
  }
  if (
    value.kind === "cancelled" &&
    hasOnlyIpcKeys(value, ["kind", "projection"]) &&
    isProjection(value.projection)
  ) {
    return { kind: "cancelled", projection: value.projection };
  }
  throw invalidCloseResponse();
}

function normalizeCloseError(error: unknown) {
  if (error instanceof ProjectCloseError) return error;
  const failure = parseProjectSaveFailure(error);
  if (failure) {
    return new ProjectCloseError(failure.code, failure.message);
  }
  return new ProjectCloseError(
    "close_unavailable",
    "Não foi possível iniciar o fechamento do Projeto.",
  );
}

async function invokeClose<T>(
  request: () => Promise<unknown>,
  parse: (value: unknown) => T,
) {
  try {
    return parse(await request());
  } catch (error: unknown) {
    throw normalizeCloseError(error);
  }
}

export const tauriProjectWindowPort: ProjectWindowPort = {
  onCloseRequested: (listener) =>
    listen(PROJECT_CLOSE_CONFIRMATION_EVENT, listener),
  requestClose: () =>
    invokeClose(
      () => invoke<unknown>("request_project_close"),
      parseCloseRequestOutcome,
    ),
  resolveClose: (choice: ProjectCloseChoice) =>
    invokeClose(
      () =>
        invoke<unknown>("resolve_project_close", {
          choice: choice satisfies IpcProjectCloseChoice,
        }),
      parseCloseResolution,
    ),
};
