import { observeSnapshot } from "../application/observeSnapshot";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { StartupImageProgress } from "../contracts/generated/StartupImageProgress";

export const OPENING_IMAGE_PROGRESS_EVENT = "myalbuns://opening-image-progress";

export function subscribeOpeningImageProgress(
  receive: (progress: StartupImageProgress) => void,
) {
  return observeSnapshot({
    subscribe: listener => listen<StartupImageProgress>(OPENING_IMAGE_PROGRESS_EVENT,
      ({ payload }) => listener(payload), { target: "dialog-opening-progress" }),
    read: () => invoke<StartupImageProgress | null>("opening_image_progress"),
    receive: progress => { if (progress) receive(progress); },
  });
}
