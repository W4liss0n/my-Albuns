import { observeSnapshot } from "../application/observeSnapshot";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { OpeningProgress } from "../contracts/generated/OpeningProgress";
import type { OpeningProjectProgress } from "../contracts/generated/OpeningProjectProgress";
import type { OpeningProjectState } from "../contracts/generated/OpeningProjectState";
import { isIpcRecord } from "./ipcGuards";

export const OPENING_PROGRESS_EVENT = "myalbuns://opening-progress";

const projectStates: readonly OpeningProjectState[] = [
  "starting", "preparing", "deciding", "ready", "failed", "cancelled",
];

function isCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0;
}

function parseProject(value: unknown): OpeningProjectProgress | null {
  if (!isIpcRecord(value) || typeof value.name !== "string" ||
    !projectStates.includes(value.state as OpeningProjectState) ||
    !isCount(value.completedFiles) || !isCount(value.totalFiles)) {
    return null;
  }
  return {
    name: value.name,
    state: value.state as OpeningProjectState,
    completedFiles: value.completedFiles,
    totalFiles: value.totalFiles,
  };
}

/** Keeps only a well-formed list of rows; anything else is ignored. */
export function parseOpeningProgress(value: unknown): OpeningProgress | null {
  if (!isIpcRecord(value) || !Array.isArray(value.projects)) return null;
  const projects = value.projects.map(parseProject);
  return projects.every((project) => project !== null)
    ? { projects: projects as OpeningProjectProgress[] }
    : null;
}

export function subscribeOpeningProgress(
  receive: (progress: OpeningProgress) => void,
) {
  return observeSnapshot<OpeningProgress>({
    subscribe: listener => listen<unknown>(OPENING_PROGRESS_EVENT, ({ payload }) => {
      const progress = parseOpeningProgress(payload);
      if (progress) listener(progress);
    }, { target: "dialog-opening-progress" }),
    read: async () => parseOpeningProgress(await invoke<unknown>("opening_progress")),
    receive: progress => { if (progress) receive(progress); },
  });
}
