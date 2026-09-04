import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";

import {
  aliveProcessInstances,
  processInstancesByExecutable,
  sameProcessInstance,
} from "./DevLifecycleProcessInstances.mjs";

export const delay = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

export function createNativeGateRuntime({
  applicationPath,
  defaultTimeoutMilliseconds,
  nativeDialogDriver,
  operationTimeoutMilliseconds = defaultTimeoutMilliseconds,
  processDataRoot,
  workspace,
}) {
  if (!applicationPath || !processDataRoot) {
    throw new Error(
      "The native gate runtime requires application and process-data roots",
    );
  }

  async function waitFor(
    label,
    predicate,
    timeout = defaultTimeoutMilliseconds,
  ) {
    const deadline = Date.now() + timeout;
    let observation;
    let lastError;
    while (Date.now() < deadline) {
      try {
        observation = await predicate();
        if (observation) return observation;
      } catch (error) {
        lastError = error;
      }
      await delay(50);
    }
    throw new Error(
      `${label} was not observed: ${JSON.stringify(observation)}; ${lastError ?? "no error"}`,
      { cause: lastError },
    );
  }

  function applicationProcesses() {
    return processInstancesByExecutable(
      applicationPath,
      path.basename(applicationPath),
    );
  }

  async function httpAvailable(url) {
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(500) });
      await response.arrayBuffer();
      return response.ok;
    } catch {
      return false;
    }
  }

  async function waitForNewApplication(predicate, known, label) {
    return waitFor(
      label,
      () =>
        applicationProcesses().find(
          (instance) =>
            predicate(instance) &&
            !known.some((candidate) =>
              sameProcessInstance(instance, candidate),
            ),
        ),
      operationTimeoutMilliseconds,
    );
  }

  async function waitForExit(instance, label) {
    await waitFor(
      label,
      () => aliveProcessInstances([instance]).length === 0,
      operationTimeoutMilliseconds,
    );
  }

  function collectChildOutput(child) {
    let output = "";
    child.stdout?.on("data", (chunk) => {
      output += chunk.toString();
    });
    child.stderr?.on("data", (chunk) => {
      output += chunk.toString();
    });
    return () => output;
  }

  function logRecords() {
    const directory = path.join(processDataRoot, "Local", "MyAlbuns2", "Logs");
    if (!existsSync(directory)) return [];
    return readdirSync(directory)
      .filter((name) => name.endsWith(".jsonl"))
      .flatMap((name) =>
        readFileSync(path.join(directory, name), "utf8")
          .split(/\r?\n/u)
          .filter(Boolean)
          .flatMap((line) => {
            try {
              return [JSON.parse(line)];
            } catch {
              return [];
            }
          }),
      )
      .sort((left, right) =>
        String(left.timestamp).localeCompare(String(right.timestamp)),
      );
  }

  function recordsFor(event) {
    return logRecords().filter((record) => record.event === event);
  }

  async function waitForLogEvent(event, count, label) {
    return waitFor(
      label,
      () => {
        const records = recordsFor(event);
        return records.length >= count ? records : undefined;
      },
      operationTimeoutMilliseconds,
    );
  }

  function driveNativeDialog(instance, action, title, destination) {
    if (!nativeDialogDriver || !workspace) {
      throw new Error("The native dialog driver is not configured");
    }
    const arguments_ = [
      "-NoProfile",
      "-NonInteractive",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      nativeDialogDriver,
      "-Action",
      action,
      "-ProcessId",
      String(instance.processId),
      "-CreationTimeUtc",
      instance.creationTimeUtc,
      "-DialogTitle",
      title,
      "-TimeoutSeconds",
      "30",
    ];
    if (destination) arguments_.push("-DestinationPath", destination);
    const result = spawnSync("powershell.exe", arguments_, {
      cwd: workspace,
      windowsHide: true,
      encoding: "utf8",
    });
    if (result.status !== 0) {
      throw new Error(
        `Native dialog automation failed: ${result.stderr || result.stdout}`,
      );
    }
    return JSON.parse(result.stdout.trim());
  }

  return {
    applicationProcesses,
    collectChildOutput,
    driveNativeDialog,
    httpAvailable,
    logRecords,
    logText: () =>
      logRecords()
        .map((record) => JSON.stringify(record))
        .join("\n"),
    recordsFor,
    waitFor,
    waitForExit,
    waitForLogEvent,
    waitForNewApplication,
  };
}

export async function readOwnedDialogPresentation(driver) {
  return driver.request("POST", `/session/${driver.sessionId}/execute/sync`, {
    script: `
        const dialog = document.querySelector('[role="dialog"]');
        const titleId = dialog?.getAttribute('aria-labelledby');
        return {
          actions: Array.from(dialog?.querySelectorAll('button') ?? [])
            .map((button) => button.textContent.trim()),
          ariaModal: dialog?.getAttribute('aria-modal') ?? null,
          dialogCount: document.querySelectorAll('[role="dialog"]').length,
          initialFocus: document.activeElement?.textContent?.trim() ?? null,
          title: titleId
            ? document.getElementById(titleId)?.textContent?.trim() ?? null
            : null,
        };
      `,
    args: [],
  });
}

export async function readProjectInteractionState(driver) {
  return driver.request("POST", `/session/${driver.sessionId}/execute/sync`, {
    script: `
        const menu = document.querySelector("nav[aria-label='Menu principal']");
        const workspace = document.querySelector('.workspace-grid');
        const commandButtons = Array.from(menu?.querySelectorAll(':scope > [role="none"] > button') ?? []);
        return {
          activeElement: document.activeElement?.textContent?.trim() ?? null,
          alerts: Array.from(document.querySelectorAll('[role="alert"]'))
            .map((node) => node.textContent?.trim() ?? ''),
          commandMenus: commandButtons.map((button) => ({
            disabled: button.disabled,
            expanded: button.getAttribute('aria-expanded'),
            label: button.textContent?.trim() ?? '',
            rectangle: button.getBoundingClientRect().toJSON(),
          })),
          dialogCount: document.querySelectorAll('[role="dialog"]').length,
          readyState: document.readyState,
          viewport: { width: innerWidth, height: innerHeight },
          scroll: { x: scrollX, y: scrollY },
          url: window.location.href,
          workspaceBusy: workspace?.getAttribute('aria-busy') ?? null,
          workspaceInert: workspace?.hasAttribute('inert') ?? false,
        };
      `,
    args: [],
  });
}
