import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Replay the latest activation after subscribing so startup cannot lose it. */
export async function onNewProjectRequest(listener: () => void): Promise<() => void> {
  let lastSequence = 0;
  const deliver = (value: unknown) => {
    if (typeof value !== "number" || !Number.isSafeInteger(value) || value <= lastSequence) return;
    lastSequence = value;
    listener();
  };
  const unlisten = await listen<unknown>("myalbuns://new-project-requested", (event) => deliver(event.payload));
  try {
    deliver(await invoke("latest_new_project_request"));
  } catch {
    // A live activation can still be delivered if the startup snapshot failed.
  }
  return unlisten;
}
