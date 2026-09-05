import path from "node:path";

// This scenario must exercise the normal bootstrap and Save As WebView transition.
// The product checks TAURI_WEBVIEW_AUTOMATION by presence, not by its string value.
export function createProjectCloseEnvironment(inherited, { scratch, label, globalPort, hostPort, dialogPort, saveAsPort }) {
  const environment = { ...inherited,
    MYALBUNS_PROCESS_GATE_DATA_ROOT: path.join(scratch, "process-data"),
    MYALBUNS_DEV_GLOBAL_WEBVIEW_DEBUG_PORT: String(globalPort),
    MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT: String(hostPort),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DEBUG_PORT: String(dialogPort),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DATA_DIRECTORY: path.join(scratch, `${label}-dialog-webview`),
    MYALBUNS_DEV_SAVE_AS_WEBVIEW_DEBUG_PORT: String(saveAsPort),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${globalPort}`,
  };
  delete environment.TAURI_WEBVIEW_AUTOMATION;
  delete environment.MYALBUNS_TAURI_WEBDRIVER_PROJECT;
  delete environment.MYALBUNS_DEV_ALTERNATE_HOST_WEBVIEW_DEBUG_PORT;
  return environment;
}
