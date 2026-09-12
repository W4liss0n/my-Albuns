import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SettingsSection } from "../application/photoshop";

export const closeSettings = () => { void invoke("close_application_settings"); };
export const onSettingsSection = (listener: (section: SettingsSection) => void) => listen<SettingsSection>("myalbuns://settings-section", (event) => {
  if (event.payload === "performance" || event.payload === "photoshop") listener(event.payload);
});
