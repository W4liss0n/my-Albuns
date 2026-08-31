import { spawn, spawnSync } from "node:child_process";
import {
  existsSync,
  readFileSync,
  readdirSync,
} from "node:fs";
import path from "node:path";

import {
  aliveProcessInstances,
  assertNoPreexistingProcessInstances,
  processInstancesByExecutable,
  sameProcessInstance,
  terminateProcessInstance,
  waitForProcessInstance,
} from "./DevLifecycleProcessInstances.mjs";
import { FOCUSED_OWNED_DIALOG_SCENARIOS } from "./FocusedOwnedDialogScenarios.mjs";
import {
  attachWebView2Driver,
  disposeConfirmedWebDriver,
  findFreeTcpPortInRange,
  switchToWebDriverWindow,
  webViewDevToolsTargets,
} from "./GateWebDriver.mjs";
import {
  nativeOwnedWindowState,
  nativeWindowTitle,
} from "./NativeWindowObservation.mjs";

const [workspaceArgument, scratchArgument, applicationArgument, driverArgument] =
  process.argv.slice(2);
if (
  !workspaceArgument ||
  !scratchArgument ||
  !applicationArgument ||
  !driverArgument
) {
  throw new Error(
    "Usage: Run-FocusedOwnedDialogGate.mjs <workspace> <scratch> <application> <native-driver>",
  );
}

const workspace = path.resolve(workspaceArgument);
const scratch = path.resolve(scratchArgument);
const applicationPath = path.resolve(applicationArgument);
const nativeDriverPath = path.resolve(driverArgument);
const processDataRoot = path.join(scratch, "process-data");
const fixture = JSON.parse(
  readFileSync(
    path.join(scratch, "focused-owned-dialog-fixture.json"),
    "utf8",
  ),
);
const originalPath = path.resolve(fixture.originalPath);
const externalCopyPath = path.resolve(fixture.externalCopyPath);
const sourceRevision = Number(fixture.sourceRevision);
const nativeDialogDriver = path.join(
  workspace,
  "scripts",
  "Drive-NativeSaveDialog.ps1",
);
const timeoutMilliseconds = Number(
  process.env.MYALBUNS_FOCUSED_DIALOG_TIMEOUT_MS ?? "90000",
);

if (
  FOCUSED_OWNED_DIALOG_SCENARIOS.length !== 2 ||
  !existsSync(applicationPath) ||
  !existsSync(nativeDriverPath) ||
  !existsSync(originalPath) ||
  !existsSync(externalCopyPath) ||
  !Number.isSafeInteger(sourceRevision)
) {
  throw new Error("The focused owned-dialog fixture is incomplete");
}

const delay = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

async function waitFor(label, predicate, timeout = timeoutMilliseconds) {
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

async function waitForNativeWindowState(label, instance, predicate) {
  let lastState;
  try {
    return await waitFor(label, () => {
      lastState = nativeOwnedWindowState(instance);
      return predicate(lastState) ? lastState : undefined;
    });
  } catch (error) {
    throw new Error(
      `${label} failed with last native state: ${JSON.stringify(lastState)}`,
      { cause: error },
    );
  }
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

function applicationProcesses() {
  return processInstancesByExecutable(
    applicationPath,
    path.basename(applicationPath),
  );
}

function isBootstrapHost(instance) {
  return instance.commandLine.includes("--myalbuns-project-host");
}

async function waitForNewApplication(predicate, known, label) {
  return waitFor(label, () =>
    applicationProcesses().find(
      (instance) =>
        predicate(instance) &&
        !known.some((candidate) => sameProcessInstance(instance, candidate)),
    ),
  );
}

async function waitForExit(instance, label) {
  await waitFor(label, () => aliveProcessInstances([instance]).length === 0);
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
    );
}

function recordsFor(event) {
  return logRecords().filter((record) => record.event === event);
}

async function waitForLogEvent(event, count, label) {
  return waitFor(label, () =>
    recordsFor(event).length >= count ? recordsFor(event) : undefined,
  );
}

async function findElement(driver, using, value, label) {
  return waitFor(label, async () => {
    try {
      const element = await driver.request(
        "POST",
        `/session/${driver.sessionId}/element`,
        { using, value },
      );
      return element["element-6066-11e4-a52e-4f735466cecf"];
    } catch {
      return undefined;
    }
  });
}

async function click(driver, using, value, label) {
  const element = await findElement(driver, using, value, label);
  await driver.request(
    "POST",
    `/session/${driver.sessionId}/element/${encodeURIComponent(element)}/click`,
  );
}

function driveNativeDialog(instance, action, title) {
  const result = spawnSync(
    "powershell.exe",
    [
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
    ],
    {
      cwd: workspace,
      windowsHide: true,
      encoding: "utf8",
    },
  );
  if (result.status !== 0) {
    throw new Error(
      `Native dialog automation failed: ${result.stderr || result.stdout}`,
    );
  }
  return JSON.parse(result.stdout.trim());
}

function readSourceRevision() {
  return Number(JSON.parse(readFileSync(externalCopyPath, "utf8")).revision);
}

function openingOwnerIsBlocked(state) {
  return (
    state.dialogCount === 1 &&
    state.dialog?.visible === true &&
    state.dialog?.enabled === true &&
    state.owner !== null &&
    state.dialog.ownerHwnd === state.owner.hwnd &&
    state.owner.enabled === false
  );
}

function projectOwnerIsBlocked(state) {
  return openingOwnerIsBlocked(state) && state.owner.visible === true;
}

async function observeExternalCopyScenario() {
  const globalDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
  const hostDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
  const projectDialogDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
  const environment = {
    ...process.env,
    MYALBUNS_PROCESS_GATE_DATA_ROOT: processDataRoot,
    MYALBUNS_DEV_GLOBAL_WEBVIEW_DEBUG_PORT: String(globalDebugPort),
    MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT: String(hostDebugPort),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DEBUG_PORT: String(
      projectDialogDebugPort,
    ),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DATA_DIRECTORY: path.join(
      scratch,
      "external-copy-project-dialog-webview",
    ),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${globalDebugPort}`,
  };
  const child = spawn(applicationPath, [externalCopyPath], {
    cwd: workspace,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
    env: environment,
  });
  const childOutput = collectChildOutput(child);
  const globalInstance = await waitForProcessInstance(
    child.pid,
    "external-copy Global",
  );
  let hostInstance;
  let driver;
  try {
    const target = await waitFor("external-copy decision target", async () => {
      const targets = await webViewDevToolsTargets(
        globalDebugPort,
        "external-copy opening owner",
      );
      const decisions = targets.filter((candidate) => {
        try {
          const url = new URL(candidate.url);
          return (
            candidate.type === "page" &&
            url.pathname.endsWith("/dialog.html") &&
            url.searchParams.get("kind") === "external-copy"
          );
        } catch {
          return false;
        }
      });
      return decisions.length === 1 ? decisions[0] : undefined;
    });
    const attemptId = new URL(target.url).searchParams.get("attemptId");
    if (!attemptId) throw new Error("The external-copy decision has no attemptId");
    hostInstance = await waitForNewApplication(
      isBootstrapHost,
      [],
      "external-copy pending Host",
    );

    driver = await attachWebView2Driver({
      debugPort: globalDebugPort,
      label: "external-copy opening owner",
      nativeDriverPath,
      sessionTimeoutMilliseconds: 60_000,
      workingDirectory: workspace,
    });
    await switchToWebDriverWindow(
      driver,
      (url) => {
        const parsed = new URL(url);
        return (
          parsed.pathname.endsWith("/dialog.html") &&
          parsed.searchParams.get("kind") === "external-copy"
        );
      },
      "external-copy decision",
    );
    await findElement(
      driver,
      "css selector",
      ".ui-owned-window-shell [role='dialog']",
      "external-copy dialog content",
    );
    const presentation = await driver.request(
      "POST",
      `/session/${driver.sessionId}/execute/sync`,
      {
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
      },
    );
    const nativeBeforePicker = await waitForNativeWindowState(
      "external-copy native owner",
      globalInstance,
      openingOwnerIsBlocked,
    );
    const hostHiddenBeforeDecision =
      nativeWindowTitle(hostInstance) === "" &&
      !(await httpAvailable(`http://127.0.0.1:${hostDebugPort}/json/version`));
    const initialSourceRevision = readSourceRevision();
    const exactPendingHostBeforePicker =
      applicationProcesses().filter(isBootstrapHost).length === 1 &&
      aliveProcessInstances([hostInstance]).length === 1;
    if (
      presentation.dialogCount !== 1 ||
      presentation.ariaModal !== "true" ||
      presentation.title !== "Cópia externa somente leitura" ||
      presentation.initialFocus !== "Salvar cópia como…" ||
      JSON.stringify(presentation.actions) !==
        JSON.stringify(["Cancelar", "Salvar cópia como…"]) ||
      !hostHiddenBeforeDecision ||
      !exactPendingHostBeforePicker ||
      initialSourceRevision !== sourceRevision
    ) {
      throw new Error(
        `The external-copy owner contract failed: ${JSON.stringify({
          exactPendingHostBeforePicker,
          hostHiddenBeforeDecision,
          initialSourceRevision,
          presentation,
          sourceRevision,
        })}`,
      );
    }

    const nativePickerOpeningCount = recordsFor(
      "native_save_dialog_opening",
    ).length;
    await click(
      driver,
      "xpath",
      "//button[normalize-space()='Salvar cópia como…']",
      "external-copy save action",
    );
    await waitForLogEvent(
      "native_save_dialog_opening",
      nativePickerOpeningCount + 1,
      "external-copy native picker",
    );
    const cancelledPicker = driveNativeDialog(
      globalInstance,
      "cancel",
      "Criar Projeto MyAlbuns",
    );
    await findElement(
      driver,
      "css selector",
      ".ui-owned-window-shell [role='dialog']",
      "external-copy dialog after picker cancellation",
    );
    const attemptAfterPicker = await driver.request(
      "POST",
      `/session/${driver.sessionId}/execute/sync`,
      {
        script:
          "return new URLSearchParams(window.location.search).get('attemptId');",
        args: [],
      },
    );
    const nativeAfterPicker = nativeOwnedWindowState(globalInstance);
    const currentHost = applicationProcesses().find(isBootstrapHost);
    const samePendingHostAndRevision =
      cancelledPicker.action === "cancel" &&
      cancelledPicker.exactProcess === true &&
      attemptAfterPicker === attemptId &&
      currentHost !== undefined &&
      sameProcessInstance(currentHost, hostInstance) &&
      readSourceRevision() === sourceRevision &&
      nativeAfterPicker.dialog?.hwnd === nativeBeforePicker.dialog?.hwnd &&
      nativeAfterPicker.owner?.hwnd === nativeBeforePicker.owner?.hwnd &&
      openingOwnerIsBlocked(nativeAfterPicker);
    if (!samePendingHostAndRevision) {
      throw new Error(
        "The external-copy picker did not return to the same Host, attempt, revision, and owner",
      );
    }

    await click(
      driver,
      "xpath",
      "//button[normalize-space()='Cancelar']",
      "external-copy terminal cancellation",
    );
    driver = await disposeConfirmedWebDriver(driver);
    await waitForExit(hostInstance, "external-copy pending Host cleanup");
    const restoredOwner = await waitForNativeWindowState(
      "external-copy Global restoration",
      globalInstance,
      (state) =>
        state.dialogCount === 0 &&
          state.windows.some(
            (window) =>
              window.ownerHwnd === 0 && window.visible && window.enabled,
          ),
    );
    const terminalCleaned =
      restoredOwner.dialogCount === 0 &&
      aliveProcessInstances([hostInstance]).length === 0;
    terminateProcessInstance(globalInstance);
    await waitForExit(globalInstance, "external-copy Global gate cleanup");

    return {
      scenario: FOCUSED_OWNED_DIALOG_SCENARIOS[0],
      attemptId,
      sourceRevision,
      ownerProcess: globalInstance,
      pendingHostProcess: hostInstance,
      dialogHwnd: nativeBeforePicker.dialog.hwnd,
      ownerHwnd: nativeBeforePicker.owner.hwnd,
      ownerDisabled: nativeBeforePicker.owner.enabled === false,
      oneVisibleDialog: nativeBeforePicker.dialogCount === 1,
      exactPickerOwner: cancelledPicker.exactProcess === true,
      samePendingHostAndRevision,
      terminalCleaned,
      childOutputTail: childOutput().slice(-500),
    };
  } finally {
    if (driver) driver = await disposeConfirmedWebDriver(driver);
    for (const instance of [hostInstance, globalInstance]) {
      if (aliveProcessInstances([instance]).length !== 0) {
        terminateProcessInstance(instance);
        await waitForExit(instance, "external-copy fallback cleanup");
      }
    }
  }
}

async function observeGraphicsScenario() {
  const hostDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
  const projectDialogDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
  const environment = {
    ...process.env,
    MYALBUNS_PROCESS_GATE_DATA_ROOT: processDataRoot,
    MYALBUNS_TAURI_WEBDRIVER_PROJECT: originalPath,
    TAURI_WEBVIEW_AUTOMATION: "true",
    MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT: String(hostDebugPort),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DEBUG_PORT: String(
      projectDialogDebugPort,
    ),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DATA_DIRECTORY: path.join(
      scratch,
      "graphics-project-dialog-webview",
    ),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${hostDebugPort}`,
  };
  const child = spawn(applicationPath, [], {
    cwd: workspace,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
    env: environment,
  });
  const childOutput = collectChildOutput(child);
  const projectInstance = await waitForProcessInstance(
    child.pid,
    "focused Project owner",
  );
  let projectDriver;
  let dialogDriver;
  try {
    projectDriver = await attachWebView2Driver({
      debugPort: hostDebugPort,
      label: "focused Project owner",
      nativeDriverPath,
      projectDialogDebugPort,
      sessionTimeoutMilliseconds: 60_000,
      workingDirectory: workspace,
    });
    await findElement(
      projectDriver,
      "css selector",
      "canvas.pixi-canvas",
      "focused productive Canvas",
    );
    const contextLostCount = recordsFor("canvas_context_lost").length;
    const contextRestoreFailedCount = recordsFor(
      "canvas_context_restore_failed",
    ).length;
    const contextLossDispatched = await projectDriver.request(
      "POST",
      `/session/${projectDriver.sessionId}/execute/sync`,
      {
        script: `
          const canvas = document.querySelector('canvas.pixi-canvas');
          if (!canvas) return false;
          return canvas.dispatchEvent(
            new Event('webglcontextlost', { bubbles: false, cancelable: true }),
          ) === false;
        `,
        args: [],
      },
    );
    if (!contextLossDispatched) {
      throw new Error("The focused Canvas did not accept context loss");
    }
    await waitForLogEvent(
      "canvas_context_lost",
      contextLostCount + 1,
      "focused Canvas context loss",
    );
    await waitForLogEvent(
      "canvas_context_restore_failed",
      contextRestoreFailedCount + 1,
      "late graphics failure",
    );
    const blockedProject = await waitFor(
      "noninteractive Project owner",
      async () => {
        const state = await projectDriver.request(
          "POST",
          `/session/${projectDriver.sessionId}/execute/sync`,
          {
            script: `
              const workspace = document.querySelector('.workspace-grid');
              return {
                canvasMounted: document.querySelector('canvas.pixi-canvas') !== null,
                inlineAlertCount: document.querySelectorAll('.startup-surface [role="alert"]').length,
                workspaceBusy: workspace?.getAttribute('aria-busy') === 'true',
                workspaceInert: workspace?.hasAttribute('inert') === true,
              };
            `,
            args: [],
          },
        );
        return state.workspaceInert && state.workspaceBusy ? state : undefined;
      },
    );
    const target = await waitFor(
      "graphics Project dialog target",
      async () => {
        const targets = await webViewDevToolsTargets(
          projectDialogDebugPort,
          "graphics Project dialog",
        );
        const matches = targets.filter((candidate) => {
          try {
            return (
              candidate.type === "page" &&
              new URL(candidate.url).pathname.endsWith("/project-dialog.html") &&
              decodeURIComponent(candidate.url).includes(
                '\"kind\":\"graphicsFailure\"',
              )
            );
          } catch {
            return false;
          }
        });
        return matches.length === 1 ? matches[0] : undefined;
      },
    );
    dialogDriver = await attachWebView2Driver({
      debugPort: projectDialogDebugPort,
      label: "graphics Project dialog",
      nativeDriverPath,
      sessionTimeoutMilliseconds: 60_000,
      workingDirectory: workspace,
    });
    await switchToWebDriverWindow(
      dialogDriver,
      (url) => new URL(url).pathname.endsWith("/project-dialog.html"),
      "graphics Project dialog",
    );
    await findElement(
      dialogDriver,
      "css selector",
      "[role='dialog']",
      "graphics dialog content",
    );
    const presentation = await dialogDriver.request(
      "POST",
      `/session/${dialogDriver.sessionId}/execute/sync`,
      {
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
      },
    );
    const nativeState = await waitForNativeWindowState(
      "graphics native owner",
      projectInstance,
      projectOwnerIsBlocked,
    );
    const currentProject = applicationProcesses().find((instance) =>
      sameProcessInstance(instance, projectInstance),
    );
    if (
      target === undefined ||
      currentProject === undefined ||
      presentation.dialogCount !== 1 ||
      presentation.ariaModal !== "true" ||
      presentation.title !== "O Canvas não pôde ser iniciado" ||
      presentation.initialFocus !== "Fechar Projeto" ||
      JSON.stringify(presentation.actions) !== JSON.stringify(["Fechar Projeto"]) ||
      !blockedProject.canvasMounted ||
      blockedProject.inlineAlertCount !== 0
    ) {
      throw new Error(
        `The late graphics dialog contract failed: ${JSON.stringify({
          blockedProject,
          currentProject,
          presentation,
        })}`,
      );
    }

    await click(
      dialogDriver,
      "xpath",
      "//button[normalize-space()='Fechar Projeto']",
      "graphics failure terminal",
    );
    dialogDriver = await disposeConfirmedWebDriver(dialogDriver);
    projectDriver = await disposeConfirmedWebDriver(projectDriver);
    await waitForExit(projectInstance, "graphics Project owner cleanup");
    const terminalCleaned =
      aliveProcessInstances([projectInstance]).length === 0;

    return {
      scenario: FOCUSED_OWNED_DIALOG_SCENARIOS[1],
      projectProcess: projectInstance,
      dialogHwnd: nativeState.dialog.hwnd,
      ownerHwnd: nativeState.owner.hwnd,
      ownerDisabled: nativeState.owner.enabled === false,
      oneVisibleDialog: nativeState.dialogCount === 1,
      workspaceInert: blockedProject.workspaceInert,
      exactAction: presentation.actions[0],
      terminalCleaned,
      childOutputTail: childOutput().slice(-500),
    };
  } finally {
    if (dialogDriver) dialogDriver = await disposeConfirmedWebDriver(dialogDriver);
    if (projectDriver) projectDriver = await disposeConfirmedWebDriver(projectDriver);
    if (aliveProcessInstances([projectInstance]).length !== 0) {
      terminateProcessInstance(projectInstance);
      await waitForExit(projectInstance, "graphics fallback cleanup");
    }
  }
}

assertNoPreexistingProcessInstances(applicationPath, path.basename(applicationPath));
assertNoPreexistingProcessInstances(nativeDriverPath, path.basename(nativeDriverPath));

let externalCopy;
let graphicsFailure;
try {
  externalCopy = await observeExternalCopyScenario();
  graphicsFailure = await observeGraphicsScenario();
} finally {
  for (const instance of applicationProcesses()) {
    terminateProcessInstance(instance);
  }
  await waitFor(
    "focused gate application cleanup",
    () => applicationProcesses().length === 0,
  );
  if (existsSync(externalCopyPath)) {
    spawnSync("attrib.exe", ["-R", externalCopyPath], {
      cwd: workspace,
      windowsHide: true,
      stdio: "ignore",
    });
  }
}

console.log(
  JSON.stringify({
    schemaVersion: 1,
    gate: "focused-owned-dialogs",
    scenarios: FOCUSED_OWNED_DIALOG_SCENARIOS,
    externalCopy,
    graphicsFailure,
    cleanupCompleted: applicationProcesses().length === 0,
  }),
);
