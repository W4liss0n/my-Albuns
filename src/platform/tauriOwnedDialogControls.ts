import { invoke } from "@tauri-apps/api/core";

export function dismissOwnedWindow() {
  return invoke<void>("dismiss_owned_dialog");
}
