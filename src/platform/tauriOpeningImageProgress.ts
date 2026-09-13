import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { StartupImageProgress } from "./generated/StartupImageProgress";

export const OPENING_IMAGE_PROGRESS_EVENT = "myalbuns://opening-image-progress";

export async function subscribeOpeningImageProgress(
  receive: (progress: StartupImageProgress) => void,
) {
  let receivedLiveProgress = false;
  const unlisten = await listen<StartupImageProgress>(OPENING_IMAGE_PROGRESS_EVENT, ({ payload }) => {
    receivedLiveProgress = true;
    receive(payload);
  });
  try {
    const current = await invoke<StartupImageProgress | null>("opening_image_progress");
    if (current && !receivedLiveProgress) receive(current);
    return unlisten;
  } catch (error) {
    unlisten();
    throw error;
  }
}
