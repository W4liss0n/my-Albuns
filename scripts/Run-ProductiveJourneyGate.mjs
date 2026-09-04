import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

import {
  aliveProcessInstances,
  assertNoPreexistingProcessInstances,
  powershellJson,
  sameProcessInstance,
  terminateProcessInstance,
  waitForProcessInstance,
} from "./DevLifecycleProcessInstances.mjs";
import { createOwnedCacheGuard } from "./ProductiveJourneyCacheSafety.mjs";
import {
  assertEmptyCacheExport,
  assertCausalProjectHandoff,
  assertCorrelatedJourneyTerminals,
  assertDistinguishableSheetExport,
  assertPhysicalAlbumProjectCoreEvents,
  assertReopenedHostExport,
  eventCount,
} from "./ProductiveJourneyObservations.mjs";
import {
  attachWebView2Driver,
  disposeConfirmedWebDriver,
  findFreeTcpPortInRange,
  switchToWebDriverWindow,
} from "./GateWebDriver.mjs";
import {
  nativeOwnedWindowState,
  nativeWindowTitle,
} from "./NativeWindowObservation.mjs";
import {
  buildCapturedPointerGestureActions,
  measureVisiblePointerGeometryScript,
  scrollIntoPointerViewportScript,
} from "./WebDriverPointerGestures.mjs";
import {
  createNativeGateRuntime,
  delay,
  readProjectInteractionState,
} from "./NativeGateRuntime.mjs";

const [
  workspaceArgument,
  scratchArgument,
  applicationArgument,
  driverArgument,
] = process.argv.slice(2);
if (
  !workspaceArgument ||
  !scratchArgument ||
  !applicationArgument ||
  !driverArgument
) {
  throw new Error(
    "Usage: Run-ProductiveJourneyGate.mjs <workspace> <scratch> <application> <native-driver>",
  );
}

const workspace = path.resolve(workspaceArgument);
const scratch = path.resolve(scratchArgument);
const applicationPath = path.resolve(applicationArgument);
const nativeDriverPath = path.resolve(driverArgument);
const processDataRoot = path.join(scratch, "process-data");
const { purgeOwnedCache, summarizeOwnedCache } = createOwnedCacheGuard({
  scratch,
  processDataRoot,
});
const projectPath = path.join(scratch, "Jornada produtiva.myalbuns");
const saveAsPath = path.join(scratch, "Jornada produtiva - Cópia.myalbuns");
const externalCopyPath = path.join(
  scratch,
  "Jornada produtiva - Cópia externa somente leitura.myalbuns",
);
const externalSavedCopyPath = path.join(
  scratch,
  "Jornada produtiva - Cópia externa editável.myalbuns",
);
const exportPath = path.join(scratch, "Jornada produtiva_002.jpg");
const missingOriginalExportPath = path.join(
  scratch,
  "Jornada produtiva_original-ausente.jpg",
);
const photoFixturePath = path.join(
  workspace,
  "crates",
  "myalbuns-imaging",
  "tests",
  "fixtures",
  "progressive-420-dri.jpg",
);
const photoPath = path.join(scratch, "Foto da jornada.jpg");
const screenshotPath = path.join(scratch, "project-canvas.png");
const personalizedBackgroundRgb = "#204060";
const nativeDialogDriver = path.join(
  workspace,
  "scripts",
  "Drive-NativeSaveDialog.ps1",
);
const timeoutMilliseconds = Number(
  process.env.MYALBUNS_PRODUCTIVE_JOURNEY_TIMEOUT_MS ?? "180000",
);
const webDriverSessionTimeoutMilliseconds = Math.min(
  timeoutMilliseconds,
  60_000,
);
const driverTerminationTimeoutMilliseconds = 30_000;
const {
  applicationProcesses,
  driveNativeDialog,
  httpAvailable,
  logRecords,
  logText,
  recordsFor,
  waitFor,
  waitForExit,
  waitForLogEvent,
  waitForNewApplication,
} = createNativeGateRuntime({
  applicationPath,
  defaultTimeoutMilliseconds: 30_000,
  nativeDialogDriver,
  operationTimeoutMilliseconds: timeoutMilliseconds,
  processDataRoot,
  workspace,
});

for (const [label, candidate] of [
  ["workspace", workspace],
  ["application", applicationPath],
  ["native WebDriver", nativeDriverPath],
  ["native dialog driver", nativeDialogDriver],
  ["JPEG Photo fixture", photoFixturePath],
]) {
  if (!existsSync(candidate)) {
    throw new Error(`${label} was not found`);
  }
}
mkdirSync(scratch, { recursive: true });
for (const knownFolder of ["Roaming", "Local", "Temporary"]) {
  mkdirSync(path.join(processDataRoot, knownFolder), { recursive: true });
}
if (
  existsSync(projectPath) ||
  existsSync(saveAsPath) ||
  existsSync(externalCopyPath) ||
  existsSync(externalSavedCopyPath) ||
  existsSync(exportPath) ||
  existsSync(missingOriginalExportPath) ||
  existsSync(photoPath)
) {
  throw new Error("The productive journey requires absent CreateOnly targets");
}
copyFileSync(photoFixturePath, photoPath);
const originalPhoto = readFileSync(photoPath);

async function waitForHttp(url, label, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, {
        signal: AbortSignal.timeout(1_000),
      });
      if (response.ok) return;
      lastError = new Error(`${label} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(50);
  }
  throw new Error(
    `${label} did not become ready: ${lastError ?? "unknown error"}`,
    {
      cause: lastError,
    },
  );
}

async function devToolsTargets(debugPort, label) {
  await waitForHttp(
    `http://127.0.0.1:${debugPort}/json/list`,
    `${label} DevTools targets`,
  );
  const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
  return response.json();
}

async function waitForProjectDialogStateTarget(
  debugPort,
  stateKind,
  label,
  excludedTargetIds = [],
) {
  return waitFor(
    label,
    async () => {
      try {
        const response = await fetch(
          `http://127.0.0.1:${debugPort}/json/list`,
          {
            signal: AbortSignal.timeout(500),
          },
        );
        if (!response.ok) return undefined;
        const targets = await response.json();
        const matches = targets.filter((target) => {
          try {
            return (
              target.type === "page" &&
              new URL(target.url).pathname.endsWith("/project-dialog.html") &&
              decodeURIComponent(target.url).includes(
                `\"kind\":\"${stateKind}\"`,
              ) &&
              !excludedTargetIds.includes(target.id)
            );
          } catch {
            return false;
          }
        });
        return matches.length === 1 ? matches[0] : undefined;
      } catch {
        return undefined;
      }
    },
    timeoutMilliseconds,
  );
}

async function startAttachedWebDriver(
  debugPort,
  label,
  projectDialogDebugPort,
) {
  return attachWebView2Driver({
    debugPort,
    driverTerminationTimeoutMilliseconds,
    driverLogPath: path.join(scratch, `webdriver-${label.replace(/[^a-z0-9]+/gi, "-")}.log`),
    label,
    nativeDriverPath,
    projectDialogDebugPort,
    sessionTimeoutMilliseconds: webDriverSessionTimeoutMilliseconds,
    workingDirectory: workspace,
  });
}

async function findElement(driver, using, value, label, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const element = await driver.request(
        "POST",
        `/session/${driver.sessionId}/element`,
        { using, value },
      );
      const elementId = element["element-6066-11e4-a52e-4f735466cecf"];
      if (elementId) return elementId;
    } catch (error) {
      lastError = error;
    }
    await delay(50);
  }
  let source = "unavailable";
  try {
    source = await driver.request("GET", `/session/${driver.sessionId}/source`);
    writeFileSync(path.join(scratch, "last-webview-source.html"), source);
  } catch {
    // The WebView may disappear while the failure is being reported.
  }
  throw new Error(
    `${label} was not found: ${lastError ?? "no element"}; source=${String(source).slice(0, 2_000)}`,
  );
}

async function click(driver, using, value, label) {
  const elementId = await findElement(driver, using, value, label);
  await driver.request(
    "POST",
    `/session/${driver.sessionId}/element/${encodeURIComponent(elementId)}/click`,
    {},
  );
  return elementId;
}

async function clickWhenEnabled(driver, using, value, label) {
  const elementId = await findElement(driver, using, value, label);
  const endpoint = `/session/${driver.sessionId}/element/${encodeURIComponent(elementId)}`;
  await waitFor(
    `${label} enabled`,
    () => driver.request("GET", `${endpoint}/enabled`),
    timeoutMilliseconds,
  );
  await driver.request("POST", `${endpoint}/click`, {});
  return elementId;
}

async function waitForHttpUnavailable(url, label, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  let consecutiveUnavailable = 0;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(500) });
      await response.arrayBuffer();
      consecutiveUnavailable = 0;
    } catch {
      consecutiveUnavailable += 1;
      if (consecutiveUnavailable >= 3) return;
    }
    await delay(50);
  }
  throw new Error(`${label} did not release its DevTools endpoint`);
}

function webViewProcessesForDataDirectory(dataDirectory) {
  return (
    powershellJson(
      String.raw`
$directory = [IO.Path]::GetFullPath($env:MYALBUNS_GATE_WEBVIEW_DATA_DIRECTORY)
$instances = @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" -ErrorAction Stop | Where-Object {
    -not [string]::IsNullOrWhiteSpace($_.CommandLine) -and
    $_.CommandLine.IndexOf($directory, [StringComparison]::OrdinalIgnoreCase) -ge 0
} | ForEach-Object {
    [ordered]@{
        processId = [int]$_.ProcessId
        parentProcessId = [int]$_.ParentProcessId
        creationTimeUtc = $_.CreationDate.ToUniversalTime().ToString('O')
        name = [string]$_.Name
        commandLine = [string]$_.CommandLine
    }
})
[Console]::Out.Write((ConvertTo-Json -InputObject $instances -Compress))
`,
      { MYALBUNS_GATE_WEBVIEW_DATA_DIRECTORY: dataDirectory },
    ) ?? []
  );
}

async function waitForWebViewDataDirectoryRelease(
  dataDirectory,
  label,
  timeout = 30_000,
) {
  const deadline = Date.now() + timeout;
  let consecutiveEmpty = 0;
  while (Date.now() < deadline) {
    if (webViewProcessesForDataDirectory(dataDirectory).length === 0) {
      consecutiveEmpty += 1;
      if (consecutiveEmpty >= 3) return;
    } else {
      consecutiveEmpty = 0;
    }
    await delay(100);
  }
  throw new Error(`${label} did not release its WebView2 data directory`);
}

async function clickElementWhenInteractable(driver, elementId, label) {
  const endpoint = `/session/${driver.sessionId}/element/${encodeURIComponent(elementId)}`;
  const deadline = Date.now() + 10_000;
  let lastError;
  while (Date.now() < deadline) {
    try {
      if (await driver.request("GET", `${endpoint}/enabled`)) {
        await driver.request("POST", `${endpoint}/click`, {});
        return;
      }
    } catch (error) {
      lastError = error;
    }
    await delay(50);
  }
  throw new Error(`${label} did not become interactable`, { cause: lastError });
}

async function findApplicationMenuCommand(
  driver,
  menuLabel,
  commandLabel,
  label,
) {
  const menuTrigger = await findElement(
    driver,
    "xpath",
    `//nav[@aria-label='Menu principal']//button[normalize-space()='${menuLabel}']`,
    `${label} menu`,
  );
  if (
    (await elementAttribute(driver, menuTrigger, "aria-expanded")) !== "true"
  ) {
    await clickElementWhenInteractable(driver, menuTrigger, `${label} menu`);
  }
  const command = await findElement(
    driver,
    "xpath",
    `//*[@role='menu' and @aria-label='${menuLabel}']//button[@aria-label='${commandLabel}']`,
    label,
  );
  return { command, menuTrigger };
}

async function selectApplicationMenuCommand(
  driver,
  menuLabel,
  commandLabel,
  label,
) {
  const { command } = await findApplicationMenuCommand(
    driver,
    menuLabel,
    commandLabel,
    label,
  );
  await clickElementWhenInteractable(driver, command, label);
}

async function selectApplicationMenuCommandUntilLogEvent(
  driver,
  menuLabel,
  commandLabel,
  event,
  label,
) {
  const expectedCount = recordsFor(event).length + 1;
  const deadline = Date.now() + timeoutMilliseconds;
  let lastError;
  while (Date.now() < deadline) {
    if (recordsFor(event).length >= expectedCount) return;
    try {
      await selectApplicationMenuCommand(
        driver,
        menuLabel,
        commandLabel,
        label,
      );
    } catch (error) {
      lastError = error;
    }
    const observationDeadline = Math.min(deadline, Date.now() + 500);
    while (Date.now() < observationDeadline) {
      if (recordsFor(event).length >= expectedCount) return;
      await delay(50);
    }
  }
  let interactionState;
  try {
    interactionState = await readProjectInteractionState(driver);
  } catch (error) {
    interactionState = { diagnosticError: String(error) };
  }
  throw new Error(
    `${label} produced no ${event} observation; interactionState=${JSON.stringify(interactionState)}`,
    {
      cause: lastError,
    },
  );
}

async function applicationMenuCommandEnabled(
  driver,
  menuLabel,
  commandLabel,
  label,
) {
  const { command, menuTrigger } = await findApplicationMenuCommand(
    driver,
    menuLabel,
    commandLabel,
    label,
  );
  const enabled = await driver.request(
    "GET",
    `/session/${driver.sessionId}/element/${encodeURIComponent(command)}/enabled`,
  );
  await driver.request(
    "POST",
    `/session/${driver.sessionId}/element/${encodeURIComponent(menuTrigger)}/click`,
    {},
  );
  return enabled;
}

async function openPhotoImportDialog(driver, label) {
  await click(
    driver,
    "xpath",
    "//div[contains(@class, 'media-import-menu')]/button[normalize-space()='Importar']",
    `${label} menu`,
  );
  await clickWhenEnabled(
    driver,
    "xpath",
    "//*[@role='menu' and @aria-label='Importar']//button[normalize-space()='Arquivo JPEG…']",
    label,
  );
}

async function withProjectDialog(driver, label, operation) {
  if (!driver.projectDialogDebugPort) {
    throw new Error(`${label} has no Project dialog debugging authority`);
  }
  const dialogDriver = await startAttachedWebDriver(
    driver.projectDialogDebugPort,
    `${label} dialog`,
  );
  try {
    return await operation(dialogDriver);
  } finally {
    await disposeConfirmedWebDriver(dialogDriver);
  }
}

function xpathLiteral(value) {
  if (!value.includes("'")) return `'${value}'`;
  if (!value.includes('"')) return `"${value}"`;
  return `concat(${value
    .split("'")
    .map((part) => `'${part}'`)
    .join(`, "'", `)})`;
}

function accessibleProjectDialogXpath(dialogLabel) {
  const title = xpathLiteral(dialogLabel);
  return `//*[@role='dialog' and @aria-modal='true' and @aria-labelledby = //*[normalize-space()=${title}]/@id]`;
}

async function clickProjectDialogAction(
  driver,
  dialogLabel,
  actionLabel,
  label,
) {
  return withProjectDialog(driver, label, (dialogDriver) =>
    clickWhenEnabled(
      dialogDriver,
      "xpath",
      `${accessibleProjectDialogXpath(dialogLabel)}//button[normalize-space()=${xpathLiteral(actionLabel)}]`,
      label,
    ),
  );
}

async function ensureInspectorSectionExpanded(driver, title, label) {
  const section = await findElement(
    driver,
    "css selector",
    `button[aria-label='${title}']`,
    label,
  );
  if ((await elementAttribute(driver, section, "aria-expanded")) !== "true") {
    await driver.request(
      "POST",
      `/session/${driver.sessionId}/element/${encodeURIComponent(section)}/click`,
      {},
    );
  }
}

async function findAlbumInformationDpi(driver, label) {
  await ensureInspectorSectionExpanded(
    driver,
    "Informações do Álbum",
    `${label} section`,
  );
  return findElement(driver, "css selector", "input[aria-label='DPI']", label);
}

async function replaceAlbumInformationDpi(driver, value, label) {
  await ensureInspectorSectionExpanded(
    driver,
    "Informações do Álbum",
    `${label} section`,
  );
  return replaceInput(
    driver,
    "css selector",
    "input[aria-label='DPI']",
    value,
    label,
  );
}

async function applyAlbumInformation(driver, label) {
  await clickWhenEnabled(
    driver,
    "css selector",
    "button[form='album-information-settings']",
    label,
  );

  await clickProjectDialogAction(
    driver,
    "Aplicar alterações no Álbum?",
    "Aplicar",
    `${label} confirmation`,
  );
  await waitForHttpUnavailable(
    `http://127.0.0.1:${driver.projectDialogDebugPort}/json/version`,
    `${label} confirmation dialog`,
    timeoutMilliseconds,
  );
}

async function doubleClick(driver, using, value, label) {
  const elementId = await findElement(driver, using, value, label);
  const endpoint = `/session/${driver.sessionId}`;
  const element = {
    "element-6066-11e4-a52e-4f735466cecf": elementId,
  };
  try {
    await driver.request("POST", `${endpoint}/actions`, {
      actions: [
        {
          type: "pointer",
          id: "productive-mouse",
          parameters: { pointerType: "mouse" },
          actions: [
            { type: "pointerMove", duration: 0, origin: element, x: 0, y: 0 },
            { type: "pointerDown", button: 0 },
            { type: "pointerUp", button: 0 },
            { type: "pause", duration: 60 },
            { type: "pointerDown", button: 0 },
            { type: "pointerUp", button: 0 },
          ],
        },
      ],
    });
  } finally {
    await driver
      .request("DELETE", `${endpoint}/actions`)
      .catch(() => undefined);
  }
  return elementId;
}

async function beginUncommittedPointerGesture(driver, using, value, label) {
  const elementId = await findElement(driver, using, value, label);
  const element = {
    "element-6066-11e4-a52e-4f735466cecf": elementId,
  };
  await driver.request("POST", `/session/${driver.sessionId}/actions`, {
    actions: [
      {
        type: "pointer",
        id: "interrupted-recovery-mouse",
        parameters: { pointerType: "mouse" },
        actions: [
          { type: "pointerMove", duration: 0, origin: element, x: 0, y: 0 },
          { type: "pointerDown", button: 0 },
          { type: "pointerMove", duration: 120, origin: element, x: 24, y: 12 },
        ],
      },
    ],
  });
}

async function sendEscape(driver) {
  const body = await findElement(
    driver,
    "css selector",
    "body",
    "Project body",
  );
  await driver.request(
    "POST",
    `/session/${driver.sessionId}/element/${encodeURIComponent(body)}/value`,
    { text: "\uE00C", value: ["\uE00C"] },
  );
}

async function clickUntilLogEvent(driver, using, value, event, label) {
  const expectedCount = recordsFor(event).length + 1;
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    await clickWhenEnabled(driver, using, value, label);
    const observationDeadline = Math.min(deadline, Date.now() + 1_000);
    while (Date.now() < observationDeadline) {
      if (recordsFor(event).length >= expectedCount) {
        return;
      }
      await delay(50);
    }
  }
  throw new Error(`${label} produced no ${event} observation`);
}

async function replaceInput(driver, using, value, text, label) {
  const elementId = await findElement(driver, using, value, label);
  const endpoint = `/session/${driver.sessionId}/element/${encodeURIComponent(elementId)}`;
  await driver.request("POST", `${endpoint}/clear`, {});
  await driver.request("POST", `${endpoint}/value`, {
    text,
    value: [...text],
  });
  return elementId;
}

async function changeFormControl(driver, using, value, nextValue, label) {
  const elementId = await findElement(driver, using, value, label);
  const observedValue = await driver.request(
    "POST",
    `/session/${driver.sessionId}/execute/sync`,
    {
      script: `
        const element = arguments[0];
        const nextValue = arguments[1];
        const descriptor = Object.getOwnPropertyDescriptor(
          Object.getPrototypeOf(element),
          "value",
        );
        if (!descriptor || typeof descriptor.set !== "function") {
          throw new Error("The public form control has no writable value");
        }
        descriptor.set.call(element, nextValue);
        element.dispatchEvent(new Event("input", { bubbles: true }));
        element.dispatchEvent(new Event("change", { bubbles: true }));
        return element.value;
      `,
      args: [{ "element-6066-11e4-a52e-4f735466cecf": elementId }, nextValue],
    },
  );
  if (observedValue !== nextValue) {
    throw new Error(`${label} did not retain its public value`);
  }
  return elementId;
}

async function elementText(driver, elementId) {
  return driver.request(
    "GET",
    `/session/${driver.sessionId}/element/${encodeURIComponent(elementId)}/text`,
  );
}

async function elementAttribute(driver, elementId, attribute) {
  return driver.request(
    "GET",
    `/session/${driver.sessionId}/element/${encodeURIComponent(elementId)}/attribute/${attribute}`,
  );
}

async function observeSheetGrid(driver, label) {
  const grid = await findElement(driver, "css selector", ".sheet-grid", label);
  return driver.request("POST", `/session/${driver.sessionId}/execute/sync`, {
    script: `
        const grid = arguments[0];
        const slots = Array.from(
          grid.querySelectorAll(":scope > .sheet-grid-slot"),
        );
        const focused = slots.find((slot) =>
          slot.querySelector("button.active"),
        );
        return {
          count: slots.length,
          focusedSheetId: focused?.dataset.sheetId ?? null,
          order: slots.map((slot) => slot.dataset.sheetId ?? ""),
        };
      `,
    args: [{ "element-6066-11e4-a52e-4f735466cecf": grid }],
  });
}

async function waitForSheetGrid(driver, label, predicate) {
  return waitFor(
    label,
    async () => {
      const observation = await observeSheetGrid(driver, label);
      return predicate(observation) ? observation : false;
    },
    timeoutMilliseconds,
  );
}

async function dragSheetInGrid(driver, sourceSheetId, targetSheetId, label) {
  const source = await findElement(
    driver,
    "css selector",
    `.sheet-grid-slot[data-sheet-id='${sourceSheetId}']`,
    `${label} source`,
  );
  const target = await findElement(
    driver,
    "css selector",
    `.sheet-grid-slot[data-sheet-id='${targetSheetId}']`,
    `${label} target`,
  );
  if ((await elementAttribute(driver, source, "draggable")) === "true") {
    throw new Error(
      `${label} unexpectedly fell back to native HTML drag-and-drop`,
    );
  }
  const endpoint = `/session/${driver.sessionId}`;
  const sourceElement = {
    "element-6066-11e4-a52e-4f735466cecf": source,
  };
  const targetElement = {
    "element-6066-11e4-a52e-4f735466cecf": target,
  };
  await driver.request("POST", `${endpoint}/execute/sync`, {
    script: scrollIntoPointerViewportScript,
    args: [sourceElement],
  });
  await driver.request("POST", `${endpoint}/execute/sync`, {
    script: scrollIntoPointerViewportScript,
    args: [targetElement],
  });
  const geometry = await driver.request("POST", `${endpoint}/execute/sync`, {
    script: measureVisiblePointerGeometryScript,
    args: [sourceElement, targetElement],
  });
  if (
    (await elementAttribute(driver, source, "data-reorder-enabled")) !== "true"
  ) {
    throw new Error(`${label} source was not ready for pointer reordering`);
  }
  try {
    await driver.request("POST", `${endpoint}/actions`, {
      actions: [
        {
          type: "pointer",
          id: "physical-album-structure-mouse",
          parameters: { pointerType: "mouse" },
          actions: buildCapturedPointerGestureActions({
            ...geometry,
            phase: "drop",
          }),
        },
      ],
    });
  } finally {
    await driver
      .request("DELETE", `${endpoint}/actions`)
      .catch(() => undefined);
  }
}

function isHost(instance) {
  return instance.commandLine.includes("--myalbuns-project-host");
}
function projectDataNamespace(projectId) {
  return `project-${createHash("sha256").update(projectId).digest("hex")}`;
}

function projectIntentRecords(processId, intent) {
  return recordsFor("project_intent_applied").filter(
    (record) =>
      Number(record.process_id) === processId && record.intent === intent,
  );
}

async function waitForProjectIntent(processId, intent, expectedCount, label) {
  return waitFor(
    label,
    () => {
      const records = projectIntentRecords(processId, intent);
      return records.length >= expectedCount
        ? records[expectedCount - 1]
        : false;
    },
    timeoutMilliseconds,
  );
}

function exportProcessorAttempts() {
  return recordsFor("imaging_process_spawned").filter(
    (record) => record.operation === "export",
  );
}

function directoryContainsJpeg(directory) {
  if (!existsSync(directory)) return false;
  return readdirSync(directory, { withFileTypes: true }).some((entry) => {
    const entryPath = path.join(directory, entry.name);
    return entry.isDirectory()
      ? directoryContainsJpeg(entryPath)
      : entry.isFile() && /\.jpe?g$/i.test(entry.name);
  });
}

function jpegDimensions(bytes) {
  if (bytes[0] !== 0xff || bytes[1] !== 0xd8) {
    throw new Error("The exported file is not a JPEG stream");
  }
  let offset = 2;
  while (offset + 9 < bytes.length) {
    if (bytes[offset] !== 0xff) {
      offset += 1;
      continue;
    }
    const marker = bytes[offset + 1];
    if ([0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7].includes(marker)) {
      return {
        height: bytes.readUInt16BE(offset + 5),
        width: bytes.readUInt16BE(offset + 7),
      };
    }
    if (marker === 0xd9 || marker === 0xda) break;
    const length = bytes.readUInt16BE(offset + 2);
    if (length < 2) break;
    offset += 2 + length;
  }
  throw new Error("The exported JPEG has no supported SOF marker");
}

function sourceContainsNativePath(source, candidate) {
  const normalizedSource = source.toLowerCase();
  const normalizedPath = candidate.toLowerCase();
  return (
    normalizedSource.includes(normalizedPath) ||
    normalizedSource.includes(normalizedPath.replaceAll("\\", "/"))
  );
}

async function waitForHostUiReady(instance, label) {
  return waitFor(
    label,
    () =>
      recordsFor("project_ui_ready").some(
        (record) => Number(record.process_id) === instance.processId,
      ),
    timeoutMilliseconds,
  );
}

const globalDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
const hostDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
const reopenedHostDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
const projectDialogDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
const saveAsHostDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
const applicationEnvironment = {
  ...process.env,
  MYALBUNS_PROCESS_GATE_DATA_ROOT: processDataRoot,
  MYALBUNS_DEV_GLOBAL_WEBVIEW_DEBUG_PORT: String(globalDebugPort),
  MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT: String(hostDebugPort),
  MYALBUNS_DEV_ALTERNATE_HOST_WEBVIEW_DEBUG_PORT: String(reopenedHostDebugPort),
  MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DEBUG_PORT: String(
    projectDialogDebugPort,
  ),
  MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DATA_DIRECTORY: path.join(
    scratch,
    "first-project-dialog-webview",
  ),
  MYALBUNS_DEV_SAVE_AS_WEBVIEW_DEBUG_PORT: String(saveAsHostDebugPort),
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${globalDebugPort}`,
};

assertNoPreexistingProcessInstances(applicationPath, "myalbuns-desktop.exe");
const firstGlobalChild = spawn(applicationPath, [], {
  cwd: workspace,
  windowsHide: true,
  stdio: ["ignore", "pipe", "pipe"],
  env: applicationEnvironment,
});
const firstGlobal = await waitForProcessInstance(
  firstGlobalChild.pid,
  "first Global",
);
let globalDriver;
let hostDriver;
let originalDriver;
let secondGlobalDriver;
let recoveryDialogDriver;
let firstHost;
let secondGlobal;
let secondHost;
let recoveryGlobal;
let crashedHost;
let originalGlobal;
let originalHost;
let originalReplacementGlobal;
let finalGlobal;
let externalCopyDialogDriver;
let externalCopyHostDriver;
let externalCopyGlobal;
let cancelledExternalCopyHost;
let queuedExternalCopyHost;
let externalCopyHost;
let externalCopyReplacementGlobal;

try {
  globalDriver = await startAttachedWebDriver(globalDebugPort, "first Global");
  await findElement(
    globalDriver,
    "css selector",
    ".global-shell",
    "Global welcome surface",
  );
  await click(
    globalDriver,
    "css selector",
    "button[aria-label='Novo Projeto']",
    "New Project action",
  );
  for (const [label, text] of [
    ["Largura da Lâmina fechada", "50.8"],
    ["Altura da Lâmina fechada", "25.4"],
    ["DPI", "240"],
    ["Quantidade de Lâminas", "3"],
  ]) {
    await replaceInput(
      globalDriver,
      "xpath",
      `//label[normalize-space()='${label}']/following::input[1]`,
      text,
      label,
    );
  }
  for (const label of ["Primeira Lâmina", "Última Lâmina"]) {
    await changeFormControl(
      globalDriver,
      "xpath",
      `//label[.//span[normalize-space()='${label}']]//select`,
      "singlePage",
      label,
    );
  }
  await click(
    globalDriver,
    "css selector",
    "button[aria-label='Continuar']",
    "creation next action",
  );
  await changeFormControl(
    globalDriver,
    "xpath",
    "//label[.//span[normalize-space()='Cor do Background']]//input[@type='color']",
    personalizedBackgroundRgb.toLowerCase(),
    "Background color",
  );
  await clickWhenEnabled(
    globalDriver,
    "css selector",
    "button[aria-label='Criar Projeto']",
    "creation action",
  );
  await delay(250);
  const cancelledCreation = driveNativeDialog(
    firstGlobal,
    "cancel",
    "Criar Projeto MyAlbuns",
  );
  if (
    cancelledCreation.action !== "cancel" ||
    existsSync(projectPath) ||
    applicationProcesses().some(isHost)
  ) {
    throw new Error("Cancelled creation crossed the ProjectCore boundary");
  }

  await clickUntilLogEvent(
    globalDriver,
    "css selector",
    "button[aria-label='Criar Projeto']",
    "native_save_dialog_opening",
    "creation retry action",
  );
  const selectedCreation = driveNativeDialog(
    firstGlobal,
    "select",
    "Criar Projeto MyAlbuns",
    projectPath,
  );
  if (selectedCreation.action !== "select") {
    throw new Error("The CreateOnly destination was not confirmed");
  }
  firstHost = await waitForNewApplication(isHost, [], "first Project Host");
  globalDriver = await disposeConfirmedWebDriver(globalDriver);
  await waitForHostUiReady(firstHost, "first Project UI ready");
  hostDriver = await startAttachedWebDriver(
    hostDebugPort,
    "first Project Host",
    projectDialogDebugPort,
  );
  await waitForExit(firstGlobal, "first Global exit after handoff");
  await waitForHttpUnavailable(
    `http://127.0.0.1:${globalDebugPort}/json/version`,
    "first Global",
    timeoutMilliseconds,
  );
  await waitFor("created Project file", () => existsSync(projectPath));
  await waitForLogEvent("project_ui_ready", 1, "first Project UI ready");
  assertCausalProjectHandoff(logText());
  await findElement(hostDriver, "css selector", ".app-shell", "Project UI");
  const projectPageSource = await hostDriver.request(
    "GET",
    `/session/${hostDriver.sessionId}/source`,
  );
  let sourcePathExposedToWebView = [
    projectPath,
    exportPath,
    photoPath,
    processDataRoot,
  ].some((candidate) => sourceContainsNativePath(projectPageSource, candidate));
  if (sourcePathExposedToWebView) {
    throw new Error("The Project WebView exposed a native operation path");
  }
  await click(
    hostDriver,
    "css selector",
    ".sheet-grid > .sheet-grid-slot:nth-child(2) > button",
    "second sheet",
  );
  const activeSheetNumber = Number(
    await elementText(
      hostDriver,
      await findElement(
        hostDriver,
        "css selector",
        ".sheet-grid > .sheet-grid-slot > button.active .sheet-tile__number",
        "active sheet number",
      ),
    ),
  );
  if (activeSheetNumber !== 2) {
    throw new Error(
      `The productive Grade click did not activate Sheet 2 (observed ${activeSheetNumber})`,
    );
  }
  await replaceAlbumInformationDpi(hostDriver, "300", "DPI input");
  await applyAlbumInformation(hostDriver, "Apply Album information action");
  await waitForLogEvent("project_intent_applied", 1, "DPI application");
  await selectApplicationMenuCommand(
    hostDriver,
    "Editar",
    "Desfazer",
    "Undo action",
  );
  await waitForLogEvent("project_undo_completed", 1, "Undo terminal");
  await selectApplicationMenuCommand(
    hostDriver,
    "Editar",
    "Refazer",
    "Redo action",
  );
  await waitForLogEvent("project_redo_completed", 1, "Redo terminal");

  await openPhotoImportDialog(hostDriver, "Import Photo action");
  const selectedPhoto = driveNativeDialog(
    firstHost,
    "select",
    "Importar Foto JPEG",
    photoPath,
  );
  if (selectedPhoto.action !== "select") {
    throw new Error("The native JPEG Photo selection was not confirmed");
  }
  await waitForLogEvent("photo_imported", 1, "Photo import terminal");
  const firstImport = recordsFor("photo_imported").at(-1);
  await openPhotoImportDialog(hostDriver, "Reimport existing Photo action");
  const reselectedPhoto = driveNativeDialog(
    firstHost,
    "select",
    "Importar Foto JPEG",
    photoPath,
  );
  if (reselectedPhoto.action !== "select") {
    throw new Error(
      "The native existing JPEG Photo selection was not confirmed",
    );
  }
  await waitForLogEvent(
    "photo_import_existing_selected",
    1,
    "existing Photo selection terminal",
  );
  const existingSelection = recordsFor("photo_import_existing_selected").at(-1);
  const selectedMediaCard = await findElement(
    hostDriver,
    "css selector",
    "button[data-media-id][aria-pressed='true']",
    "reselected existing Photo card",
  );
  const selectedMediaId = await elementAttribute(
    hostDriver,
    selectedMediaCard,
    "data-media-id",
  );
  const reimportedExistingPhotoWithoutRevision =
    firstImport &&
    existingSelection &&
    Number(existingSelection.revision) === Number(firstImport.revision) &&
    existingSelection.media_id === firstImport.media_id &&
    selectedMediaId === firstImport.media_id;
  if (!reimportedExistingPhotoWithoutRevision) {
    throw new Error(
      "Reimporting the same JPEG did not select its existing card without a revision",
    );
  }
  await doubleClick(
    hostDriver,
    "css selector",
    "button[data-media-id]",
    "imported Photo card",
  );
  await waitForLogEvent(
    "project_intent_applied",
    2,
    "double-click Photo placement",
  );
  await findElement(
    hostDriver,
    "xpath",
    "//*[normalize-space()='Frame selecionado']",
    "affected Frame contextual selection",
  );
  await waitForLogEvent(
    "canvas_opaque_preview_texture_loaded",
    1,
    "Canvas Photo preview",
  );
  const cacheRoot = path.join(processDataRoot, "Local", "MyAlbuns2", "Cache");
  await waitFor(
    "real cached Photo preview",
    () => directoryContainsJpeg(cacheRoot),
    timeoutMilliseconds,
  );
  const previewCacheBeforePurge = summarizeOwnedCache(cacheRoot);
  const importedPageSource = await hostDriver.request(
    "GET",
    `/session/${hostDriver.sessionId}/source`,
  );
  sourcePathExposedToWebView ||= [
    projectPath,
    exportPath,
    photoPath,
    processDataRoot,
  ].some((candidate) =>
    sourceContainsNativePath(importedPageSource, candidate),
  );

  await selectApplicationMenuCommand(
    hostDriver,
    "Arquivo",
    "Salvar",
    "Save action",
  );
  await waitForLogEvent("project_save_completed", 1, "Save terminal");
  const savedProject = readFileSync(projectPath);
  const savedDocument = JSON.parse(savedProject.toString("utf8"));
  const savedPhoto = savedDocument.project.media.find(
    (media) => media.kind === "photo",
  );
  const savedFrames = savedDocument.project.sheets[1].frames;
  const persistedPhotoLinkOnly =
    savedPhoto &&
    savedDocument.project.media.length === 1 &&
    Object.keys(savedPhoto).sort().join(",") === "id,kind,path" &&
    savedFrames.length === 1 &&
    savedFrames[0].photo?.mediaId === savedPhoto.id &&
    Object.keys(savedFrames[0].photo.transform).sort().join(",") ===
      "panX,panY,userZoom";
  if (
    savedDocument.schemaVersion !== 3 ||
    savedDocument.revision !== 3 ||
    savedDocument.project.document.dpi !== 300 ||
    !persistedPhotoLinkOnly
  ) {
    throw new Error(
      "The productive save did not persist the revision-3 external Photo composition",
    );
  }
  if (!readFileSync(photoPath).equals(originalPhoto)) {
    throw new Error("Import and Save modified the linked Photo Original");
  }

  const projectWebViewDataDirectory = path.join(
    processDataRoot,
    "Local",
    "MyAlbuns2",
    "State",
    "WebView2",
    projectDataNamespace(savedDocument.projectId),
  );
  if (
    webViewProcessesForDataDirectory(projectWebViewDataDirectory).length === 0
  ) {
    throw new Error("The productive Host WebView2 process was not observable");
  }

  await selectApplicationMenuCommandUntilLogEvent(
    hostDriver,
    "Arquivo",
    "Fechar Projeto",
    "clean_project_close_requested",
    "saved Project close",
  );
  hostDriver = await disposeConfirmedWebDriver(hostDriver);
  await waitForExit(firstHost, "first Project Host close after Save");
  await waitForHttpUnavailable(
    `http://127.0.0.1:${hostDebugPort}/json/version`,
    "first Project Host",
    timeoutMilliseconds,
  );
  await waitForWebViewDataDirectoryRelease(
    projectWebViewDataDirectory,
    "first Project Host",
    timeoutMilliseconds,
  );
  const canvasPreviewCountBeforeReopen = recordsFor(
    "canvas_opaque_preview_texture_loaded",
  ).length;

  secondGlobal = await waitForNewApplication(
    (instance) => !isHost(instance),
    [firstGlobal],
    "replacement Global",
  );
  secondGlobalDriver = await startAttachedWebDriver(
    globalDebugPort,
    "replacement Global",
  );
  await click(
    secondGlobalDriver,
    "css selector",
    ".global-recent-list button",
    "recent Project",
  );
  secondHost = await waitForNewApplication(
    isHost,
    [firstHost],
    "reopened Project Host",
  );
  await waitForExit(secondGlobal, "replacement Global handoff");
  await waitForHostUiReady(secondHost, "reopened Project UI ready");
  secondGlobalDriver = await disposeConfirmedWebDriver(secondGlobalDriver);

  hostDriver = await startAttachedWebDriver(
    reopenedHostDebugPort,
    "reopened Project Host",
    projectDialogDebugPort,
  );
  const reopenedDpi = await findAlbumInformationDpi(hostDriver, "reopened DPI");
  if ((await elementAttribute(hostDriver, reopenedDpi, "value")) !== "300") {
    throw new Error("The reopened Project did not restore the saved DPI");
  }
  await click(
    hostDriver,
    "css selector",
    ".sheet-grid > .sheet-grid-slot:nth-child(2) > button",
    "reopened second sheet",
  );
  const reopenedActiveSheetNumber = Number(
    await elementText(
      hostDriver,
      await findElement(
        hostDriver,
        "css selector",
        ".sheet-grid > .sheet-grid-slot > button.active .sheet-tile__number",
        "reopened active sheet number",
      ),
    ),
  );
  if (reopenedActiveSheetNumber !== activeSheetNumber) {
    throw new Error("The reopened Host did not select the saved Photo sheet");
  }
  await findElement(
    hostDriver,
    "css selector",
    "button[data-media-id]",
    "reopened linked Photo",
  );
  await findElement(
    hostDriver,
    "css selector",
    "[data-preview-photo-id]",
    "reopened persisted Frame Photo",
  );
  await waitForLogEvent(
    "canvas_opaque_preview_texture_loaded",
    canvasPreviewCountBeforeReopen + 1,
    "reopened Canvas Photo preview",
  );
  const reopenedPageSource = await hostDriver.request(
    "GET",
    `/session/${hostDriver.sessionId}/source`,
  );
  sourcePathExposedToWebView ||= [
    projectPath,
    exportPath,
    photoPath,
    processDataRoot,
  ].some((candidate) =>
    sourceContainsNativePath(reopenedPageSource, candidate),
  );
  for (const label of ["Desfazer", "Refazer"]) {
    const enabled = await applicationMenuCommandEnabled(
      hostDriver,
      "Editar",
      label,
      `${label} after reopen`,
    );
    if (enabled) {
      throw new Error(`The reopened Project retained ${label} history`);
    }
  }

  await doubleClick(
    hostDriver,
    "css selector",
    "canvas.pixi-canvas",
    "selected Photo Frame",
  );
  await sendEscape(hostDriver);
  await findElement(
    hostDriver,
    "xpath",
    "//button[.//span[normalize-space()='Design do Álbum']]",
    "Album inspector after leaving edit mode",
  );

  await replaceAlbumInformationDpi(hostDriver, "360", "unsaved DPI input");
  await applyAlbumInformation(
    hostDriver,
    "Apply unsaved Album information action",
  );
  await waitForLogEvent("project_intent_applied", 3, "unsaved DPI application");

  const originalProjectId = savedDocument.projectId;
  const originalNamespace = projectDataNamespace(originalProjectId);
  const recoveryCheckpointPath = path.join(
    processDataRoot,
    "Local",
    "MyAlbuns2",
    "Recovery",
    "Projects",
    `${originalNamespace}.json`,
  );
  const recoveryCheckpoint = await waitFor(
    "debounced recovery checkpoint after completed action",
    () => {
      if (!existsSync(recoveryCheckpointPath)) return undefined;
      try {
        const checkpoint = JSON.parse(
          readFileSync(recoveryCheckpointPath, "utf8"),
        );
        const envelopeKeys = Object.keys(checkpoint).sort().join(",");
        const baseKeys = Object.keys(checkpoint.baseRevision ?? {})
          .sort()
          .join(",");
        const creativeKeys = Object.keys(checkpoint.creativeState ?? {})
          .sort()
          .join(",");
        return checkpoint.schemaVersion === 1 &&
          checkpoint.projectId === originalProjectId &&
          checkpoint.baseRevision?.projectId === originalProjectId &&
          checkpoint.baseRevision?.revision === 3 &&
          checkpoint.creativeState?.projectId === originalProjectId &&
          checkpoint.creativeState?.documentType === "myalbuns.project" &&
          checkpoint.creativeState?.revision === 4 &&
          checkpoint.creativeState?.project?.document?.dpi === 360 &&
          envelopeKeys ===
            "baseRevision,creativeState,projectId,schemaVersion" &&
          baseKeys === "projectId,revision" &&
          creativeKeys ===
            "documentType,project,projectId,revision,schemaVersion"
          ? checkpoint
          : undefined;
      } catch {
        return undefined;
      }
    },
    timeoutMilliseconds,
  );
  const recoveryCheckpointBytes = readFileSync(recoveryCheckpointPath);
  const projectBytesBeforeCrash = readFileSync(projectPath);
  if (!projectBytesBeforeCrash.equals(savedProject)) {
    throw new Error("The recovery checkpoint behaved as an autosave");
  }

  await doubleClick(
    hostDriver,
    "css selector",
    "canvas.pixi-canvas",
    "Photo Frame before interrupted gesture",
  );
  await beginUncommittedPointerGesture(
    hostDriver,
    "css selector",
    "canvas.pixi-canvas",
    "continuous Photo gesture",
  );
  await delay(500);
  const midGesturePreservedPreviousCheckpoint =
    readFileSync(recoveryCheckpointPath).equals(recoveryCheckpointBytes) &&
    readFileSync(projectPath).equals(projectBytesBeforeCrash);
  if (!midGesturePreservedPreviousCheckpoint) {
    throw new Error(
      "An unfinished continuous gesture replaced the previous checkpoint or saved Project",
    );
  }

  crashedHost = secondHost;
  terminateProcessInstance(crashedHost);
  await waitForExit(crashedHost, "crashed Project Host");
  hostDriver = await disposeConfirmedWebDriver(hostDriver);
  if (
    !existsSync(recoveryCheckpointPath) ||
    !readFileSync(recoveryCheckpointPath).equals(recoveryCheckpointBytes) ||
    !readFileSync(projectPath).equals(projectBytesBeforeCrash)
  ) {
    throw new Error("The abrupt Host exit changed durable Project state");
  }

  const recoveryGlobalDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
  const recoveryHostDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
  const recoveryProjectDialogDebugPort = await findFreeTcpPortInRange(
    40_000,
    44_999,
  );
  const recoveryApplicationEnvironment = {
    ...process.env,
    MYALBUNS_PROCESS_GATE_DATA_ROOT: processDataRoot,
    MYALBUNS_DEV_GLOBAL_WEBVIEW_DEBUG_PORT: String(recoveryGlobalDebugPort),
    MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT: String(recoveryHostDebugPort),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DEBUG_PORT: String(
      recoveryProjectDialogDebugPort,
    ),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DATA_DIRECTORY: path.join(
      scratch,
      "recovery-project-dialog-webview",
    ),
    MYALBUNS_DEV_SAVE_AS_WEBVIEW_DEBUG_PORT: String(saveAsHostDebugPort),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${recoveryGlobalDebugPort}`,
  };
  const recoveryGlobalChild = spawn(applicationPath, [projectPath], {
    cwd: workspace,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
    env: recoveryApplicationEnvironment,
  });
  recoveryGlobal = await waitForProcessInstance(
    recoveryGlobalChild.pid,
    "recovery Global",
  );
  secondHost = await waitForNewApplication(
    isHost,
    [firstHost, crashedHost],
    "recovery Project Host",
  );
  const projectWindowTitleBeforeDecision = nativeWindowTitle(secondHost);
  const hostWebViewBeforeDecision = await httpAvailable(
    `http://127.0.0.1:${recoveryHostDebugPort}/json/version`,
  );
  const recoveryTargetSnapshot = await waitFor(
    "external Recovery dialog target",
    async () => {
      const ownerTargets = await devToolsTargets(
        recoveryGlobalDebugPort,
        "recovery Global opening owner",
      );
      const globalTargets = ownerTargets
        .filter((target) => target.type === "page")
        .filter((target) => target.url.includes("global.html"));
      const recoveryTargets = ownerTargets
        .filter((target) => target.type === "page")
        .filter((target) => {
          try {
            const url = new URL(target.url);
            return (
              url.pathname.endsWith("/dialog.html") &&
              url.searchParams.get("kind") === "project-recovery"
            );
          } catch {
            return false;
          }
        });
      return globalTargets.length >= 1 && recoveryTargets.length === 1
        ? { globalTargets, recoveryTargets }
        : undefined;
    },
    timeoutMilliseconds,
  );
  recoveryDialogDriver = await startAttachedWebDriver(
    recoveryGlobalDebugPort,
    "external recovery opening dialog",
  );
  await switchToWebDriverWindow(
    recoveryDialogDriver,
    (url) => {
      const parsed = new URL(url);
      return (
        parsed.pathname.endsWith("/dialog.html") &&
        parsed.searchParams.get("kind") === "project-recovery"
      );
    },
    "external Recovery dialog WebView",
  );
  await findElement(
    recoveryDialogDriver,
    "css selector",
    ".ui-owned-window-shell [role='dialog']",
    "external Recovery decision",
  );
  const recoveryPresentation = await recoveryDialogDriver.request(
    "POST",
    `/session/${recoveryDialogDriver.sessionId}/execute/sync`,
    {
      script: `
        const dialog = document.querySelector('[role="dialog"]');
        const title = dialog?.getAttribute('aria-labelledby');
        const transition = document.querySelector('[data-opening-owner-transition]');
        const shell = document.querySelector('.ui-owned-window-shell');
        const actionGeometry = Array.from(dialog?.querySelectorAll('button') ?? [])
          .map((button) => {
            const range = document.createRange();
            range.selectNodeContents(button);
            const lineTops = [];
            for (const rect of range.getClientRects()) {
              if (rect.width <= 0 || rect.height <= 0) continue;
              if (!lineTops.some((top) => Math.abs(top - rect.top) <= 1)) {
                lineTops.push(rect.top);
              }
            }
            const bounds = button.getBoundingClientRect();
            return {
              clientWidth: button.clientWidth,
              height: bounds.height,
              label: button.textContent.trim(),
              lineCount: lineTops.length,
              scrollWidth: button.scrollWidth,
              width: bounds.width,
            };
          });
        return {
          actionGeometry,
          ariaModal: dialog?.getAttribute('aria-modal') ?? null,
          choices: Array.from(dialog?.querySelectorAll('button') ?? [])
            .map((button) => button.textContent.trim()),
          contentFitted:
            shell !== null &&
            Math.abs(document.documentElement.clientHeight - shell.scrollHeight) <= 2,
          dialogCount: document.querySelectorAll('[role="dialog"]').length,
          externalDialog:
            window.location.pathname.endsWith('/dialog.html') &&
            new URLSearchParams(window.location.search).get('kind') === 'project-recovery',
          fullPageRecoveryCount: document.querySelectorAll('.startup-surface .recovery-actions').length,
          initialFocus: document.activeElement?.textContent?.trim() ?? null,
          modalLayerCount: document.querySelectorAll('.ui-modal-dialog-layer').length,
          openedFromLoadingOwner:
            transition?.getAttribute('data-opening-owner-transition') === 'true',
          ownerSurfaceCount: document.querySelectorAll('[data-project-owner-surface]').length,
          ownedShellCount: document.querySelectorAll('.ui-owned-window-shell').length,
          title: title ? document.getElementById(title)?.textContent?.trim() ?? null : null,
          url: window.location.href,
          viewportHeight: document.documentElement.clientHeight,
          viewportWidth: document.documentElement.clientWidth,
        };
      `,
      args: [],
    },
  );
  const recoveryChoices = recoveryPresentation.choices;
  if (
    JSON.stringify(recoveryChoices) !==
    JSON.stringify([
      "Agora não",
      "Abrir última versão salva",
      "Reabrir e recuperar",
    ])
  ) {
    throw new Error(
      `The recovery prompt exposed unexpected choices: ${JSON.stringify(recoveryChoices)}`,
    );
  }
  const recoveryActionsAreSingleLine =
    recoveryPresentation.actionGeometry.every(
      (action) =>
        action.lineCount === 1 && action.scrollWidth <= action.clientWidth + 1,
    );
  if (
    recoveryPresentation.dialogCount !== 1 ||
    recoveryPresentation.modalLayerCount !== 0 ||
    recoveryPresentation.ownerSurfaceCount !== 0 ||
    recoveryPresentation.ownedShellCount !== 1 ||
    recoveryPresentation.fullPageRecoveryCount !== 0 ||
    recoveryPresentation.ariaModal !== "true" ||
    recoveryPresentation.title !== "Recuperar trabalho não salvo?" ||
    recoveryPresentation.initialFocus !== "Reabrir e recuperar" ||
    !recoveryActionsAreSingleLine ||
    !recoveryPresentation.contentFitted ||
    recoveryPresentation.viewportWidth !== 492 ||
    !recoveryPresentation.externalDialog ||
    !recoveryPresentation.openedFromLoadingOwner ||
    projectWindowTitleBeforeDecision !== "" ||
    hostWebViewBeforeDecision
  ) {
    throw new Error(
      `Recovery was not one accessible external dialog in the stable opening owner: ${JSON.stringify(
        {
          ...recoveryPresentation,
          hostWebViewBeforeDecision,
          projectWindowTitleBeforeDecision,
        },
      )}`,
    );
  }

  const forwardedRecoveryActivationCount = recordsFor(
    "global_activation_forwarded",
  ).length;
  const queuedRecoveryActivationChild = spawn(applicationPath, [projectPath], {
    cwd: workspace,
    windowsHide: true,
    stdio: "ignore",
    env: recoveryApplicationEnvironment,
  });
  await waitForLogEvent(
    "global_activation_forwarded",
    forwardedRecoveryActivationCount + 1,
    "forwarded activation queued behind Recovery",
  );
  await waitFor(
    "forwarded activation process terminal",
    () =>
      queuedRecoveryActivationChild.exitCode !== null ||
      queuedRecoveryActivationChild.signalCode !== null,
    timeoutMilliseconds,
  );
  await delay(300);
  const targetsWithQueuedActivation = await devToolsTargets(
    recoveryGlobalDebugPort,
    "Recovery owner with queued activation",
  );
  const recoveryTargetWithQueuedActivation = targetsWithQueuedActivation.find(
    (target) =>
      target.id === recoveryTargetSnapshot.recoveryTargets[0].id &&
      target.url === recoveryTargetSnapshot.recoveryTargets[0].url,
  );
  const queuedActivationDialogCount = await recoveryDialogDriver.request(
    "POST",
    `/session/${recoveryDialogDriver.sessionId}/execute/sync`,
    {
      script: "return document.querySelectorAll('[role=\"dialog\"]').length;",
      args: [],
    },
  );
  const activeHostsWithQueuedActivation = applicationProcesses().filter(isHost);
  const singleHostDuringQueuedActivation =
    activeHostsWithQueuedActivation.length === 1 &&
    sameProcessInstance(activeHostsWithQueuedActivation[0], secondHost);
  const queuedActivationPreservedOwner =
    recoveryTargetWithQueuedActivation !== undefined &&
    queuedActivationDialogCount === 1 &&
    aliveProcessInstances([recoveryGlobal, secondHost]).length === 2;
  if (!queuedActivationPreservedOwner || !singleHostDuringQueuedActivation) {
    throw new Error(
      "A forwarded activation replaced the correlated Recovery owner or duplicated its Host",
    );
  }
  await click(
    recoveryDialogDriver,
    "xpath",
    "//button[normalize-space()='Reabrir e recuperar']",
    "Reopen and recover choice",
  );
  await waitForExit(recoveryGlobal, "recovery Global handoff");
  recoveryDialogDriver = await disposeConfirmedWebDriver(recoveryDialogDriver);
  await waitForHostUiReady(secondHost, "recovered Project UI ready");
  hostDriver = await startAttachedWebDriver(
    recoveryHostDebugPort,
    "recovery Project Host",
    recoveryProjectDialogDebugPort,
  );
  await findElement(
    hostDriver,
    "css selector",
    ".app-shell",
    "recovered Project UI",
  );
  const recoveredOwnerUrl = await hostDriver.request(
    "POST",
    `/session/${hostDriver.sessionId}/execute/sync`,
    { script: "return window.location.href;", args: [] },
  );
  const projectRouteNormal = !recoveredOwnerUrl.includes("project-recovery");

  const recoveredDpi = await findAlbumInformationDpi(
    hostDriver,
    "recovered DPI",
  );
  if ((await elementAttribute(hostDriver, recoveredDpi, "value")) !== "360") {
    throw new Error(
      "The recovered Project did not restore the checkpoint state",
    );
  }
  let recoveredHistoryEmpty = true;
  for (const label of ["Desfazer", "Refazer"]) {
    if (
      await applicationMenuCommandEnabled(
        hostDriver,
        "Editar",
        label,
        `${label} after recovery`,
      )
    ) {
      recoveredHistoryEmpty = false;
    }
  }
  const recoveredUnsaved = await applicationMenuCommandEnabled(
    hostDriver,
    "Arquivo",
    "Salvar",
    "Save after recovery",
  );
  const checkpointPreservedAfterRecovery =
    existsSync(recoveryCheckpointPath) &&
    readFileSync(recoveryCheckpointPath).equals(recoveryCheckpointBytes);
  const projectFileUnchangedThroughRecovery = readFileSync(projectPath).equals(
    projectBytesBeforeCrash,
  );
  if (
    !recoveredHistoryEmpty ||
    !recoveredUnsaved ||
    !checkpointPreservedAfterRecovery ||
    !projectFileUnchangedThroughRecovery
  ) {
    throw new Error(
      "Recovered state was not unsaved with empty History and an intact checkpoint",
    );
  }

  for (const [dpi, revision] of [
    [300, 5],
    [360, 6],
  ]) {
    await replaceAlbumInformationDpi(
      hostDriver,
      String(dpi),
      `post-recovery DPI ${dpi}`,
    );
    await applyAlbumInformation(
      hostDriver,
      `Apply post-recovery Album information ${dpi}`,
    );
    await waitFor(
      `post-recovery revision ${revision}`,
      () =>
        recordsFor("project_intent_applied").some(
          (record) =>
            record.project_id === originalProjectId &&
            Number(record.revision) === revision,
        ),
      timeoutMilliseconds,
    );
  }
  const postRecoveryCheckpoint = await waitFor(
    "checkpoint after post-recovery actions",
    () => {
      if (!existsSync(recoveryCheckpointPath)) return undefined;
      try {
        const checkpoint = JSON.parse(
          readFileSync(recoveryCheckpointPath, "utf8"),
        );
        return checkpoint.projectId === originalProjectId &&
          checkpoint.baseRevision?.revision === 3 &&
          checkpoint.creativeState?.revision === 6 &&
          checkpoint.creativeState?.project?.document?.dpi === 360
          ? checkpoint
          : undefined;
      } catch {
        return undefined;
      }
    },
    timeoutMilliseconds,
  );
  const preSaveAsRecoveryCheckpointBytes = readFileSync(recoveryCheckpointPath);
  const webviewStateRoot = path.join(
    processDataRoot,
    "Local",
    "MyAlbuns2",
    "State",
    "WebView2",
  );
  const originalWebviewDataDirectory = path.join(
    webviewStateRoot,
    originalNamespace,
  );
  await waitFor(
    "original identity WebView2 data directory",
    () => existsSync(originalWebviewDataDirectory),
    timeoutMilliseconds,
  );
  const saveAsCompletedBeforeCancel = recordsFor(
    "project_save_as_completed",
  ).length;
  const cacheStageBeforeCancel = recordsFor(
    "project_save_as_cache_staged_empty",
  ).length;
  const localTransitionBeforeCancel = recordsFor(
    "project_save_as_local_authority_transitioned",
  ).length;
  const recoveryFinishBeforeCancel = recordsFor(
    "project_save_as_previous_recovery_finished",
  ).length;

  await selectApplicationMenuCommand(
    hostDriver,
    "Arquivo",
    "Salvar como…",
    "Save As cancellation action",
  );
  const cancelledSaveAs = driveNativeDialog(
    secondHost,
    "cancel",
    "Salvar Projeto como",
  );
  await waitFor(
    "cancelled Save As frontend settlement",
    async () => {
      const dpi = await findAlbumInformationDpi(
        hostDriver,
        "DPI after cancelled Save As",
      );
      return (await elementAttribute(hostDriver, dpi, "value")) === "360";
    },
    timeoutMilliseconds,
  );
  const cancelledSaveAsBeforeCore =
    cancelledSaveAs.action === "cancel" &&
    !existsSync(saveAsPath) &&
    readFileSync(projectPath).equals(savedProject) &&
    existsSync(recoveryCheckpointPath) &&
    readFileSync(recoveryCheckpointPath).equals(
      preSaveAsRecoveryCheckpointBytes,
    ) &&
    recordsFor("project_save_as_completed").length ===
      saveAsCompletedBeforeCancel &&
    recordsFor("project_save_as_cache_staged_empty").length ===
      cacheStageBeforeCancel &&
    recordsFor("project_save_as_local_authority_transitioned").length ===
      localTransitionBeforeCancel &&
    recordsFor("project_save_as_previous_recovery_finished").length ===
      recoveryFinishBeforeCancel;
  if (!cancelledSaveAsBeforeCore) {
    throw new Error(
      "Cancelled Save As crossed the ProjectCore or identity-scoped local transition boundary",
    );
  }

  await selectApplicationMenuCommand(
    hostDriver,
    "Arquivo",
    "Salvar como…",
    "Save As action",
  );
  const selectedSaveAs = driveNativeDialog(
    secondHost,
    "select",
    "Salvar Projeto como",
    saveAsPath,
  );
  if (selectedSaveAs.action !== "select") {
    throw new Error("The Save As CreateOnly destination was not confirmed");
  }
  await waitForLogEvent(
    "project_save_as_completed",
    saveAsCompletedBeforeCancel + 1,
    "Save As completion",
  );
  hostDriver = await disposeConfirmedWebDriver(hostDriver);
  await waitFor(
    "Save As destination",
    () => existsSync(saveAsPath),
    timeoutMilliseconds,
  );
  const savedAsProject = readFileSync(saveAsPath);
  const savedAsDocument = JSON.parse(savedAsProject.toString("utf8"));
  const expectedSavedAsProject = JSON.parse(
    JSON.stringify(savedDocument.project),
  );
  expectedSavedAsProject.document.dpi = 360;
  const savedAsContentPreserved =
    savedAsDocument.schemaVersion === 3 &&
    savedAsDocument.projectId !== originalProjectId &&
    savedAsDocument.revision === 6 &&
    JSON.stringify(savedAsDocument.project) ===
      JSON.stringify(expectedSavedAsProject);
  const originalByteIdenticalAfterSaveAs =
    readFileSync(projectPath).equals(savedProject);
  if (!savedAsContentPreserved || !originalByteIdenticalAfterSaveAs) {
    throw new Error(
      "Save As did not preserve the complete visible Project while leaving the original byte-identical",
    );
  }
  const copiedProjectId = savedAsDocument.projectId;
  const copiedNamespace = projectDataNamespace(copiedProjectId);
  const copiedWebviewDataDirectory = path.join(
    webviewStateRoot,
    copiedNamespace,
  );
  const originalCacheDirectory = path.join(cacheRoot, originalNamespace);
  const copiedCacheDirectory = path.join(cacheRoot, copiedNamespace);
  await waitFor(
    "copied identity WebView2 data directory",
    () => existsSync(copiedWebviewDataDirectory),
    timeoutMilliseconds,
  );
  await waitFor(
    "copied identity Cache namespace",
    () => existsSync(copiedCacheDirectory),
    timeoutMilliseconds,
  );
  const emptyCacheStage = recordsFor("project_save_as_cache_staged_empty").find(
    (record) =>
      record.project_id === copiedProjectId &&
      Number(record.cache_entry_count) === 0 &&
      Number(record.cache_byte_count) === 0,
  );
  const localAuthorityTransitioned = recordsFor(
    "project_save_as_local_authority_transitioned",
  ).some((record) => record.project_id === copiedProjectId);
  const recoveryFinished =
    !existsSync(recoveryCheckpointPath) &&
    recordsFor("project_save_as_previous_recovery_finished").some(
      (record) => record.project_id === copiedProjectId,
    );
  const namespaceTransitioned =
    originalNamespace !== copiedNamespace &&
    existsSync(originalWebviewDataDirectory) &&
    existsSync(copiedWebviewDataDirectory) &&
    existsSync(originalCacheDirectory) &&
    existsSync(copiedCacheDirectory) &&
    Boolean(emptyCacheStage) &&
    localAuthorityTransitioned;
  if (!namespaceTransitioned || !recoveryFinished) {
    throw new Error(
      "Save As did not transition WebView2/Cache authority or finish the previous Recovery checkpoint",
    );
  }

  const expectedSavedAsTitle = `${path.basename(
    saveAsPath,
    ".myalbuns",
  )} — ${saveAsPath}`;
  const observedSavedAsTitle = nativeWindowTitle(secondHost);
  if (
    !observedSavedAsTitle.includes(path.basename(saveAsPath, ".myalbuns")) ||
    !observedSavedAsTitle.includes(saveAsPath)
  ) {
    throw new Error(
      `Save As native title omitted its current Name or Location: observed=${JSON.stringify(observedSavedAsTitle)}, expected=${JSON.stringify(expectedSavedAsTitle)}`,
    );
  }
  const nativeTitleUpdated = true;
  await waitForLogEvent(
    "project_webview_authority_ready",
    1,
    "Save As replacement WebView readiness",
  );
  const rebuiltWebviewReady = recordsFor(
    "project_webview_authority_ready",
  ).some((record) => Number(record.process_id) === secondHost.processId);
  if (!rebuiltWebviewReady) {
    throw new Error(
      "The Save As Host did not make its replacement WebView ready",
    );
  }

  hostDriver = await startAttachedWebDriver(
    saveAsHostDebugPort,
    "Save As Project Host",
    recoveryProjectDialogDebugPort,
  );
  await findElement(
    hostDriver,
    "css selector",
    ".app-shell",
    "Save As Project UI",
  );
  const copiedAlbumDesign = await findElement(
    hostDriver,
    "xpath",
    "//button[.//span[normalize-space()='Design do Álbum']]",
    "fresh Save As Album Design section",
  );
  const copiedAlbumDesignExpanded = await elementAttribute(
    hostDriver,
    copiedAlbumDesign,
    "aria-expanded",
  );
  const globalInspectorPreferencePreserved =
    copiedAlbumDesignExpanded === "true";
  if (!globalInspectorPreferencePreserved) {
    throw new Error(
      "The Save As WebView did not preserve the user's global inspector preference",
    );
  }
  const freshActiveSheetNumber = Number(
    await elementText(
      hostDriver,
      await findElement(
        hostDriver,
        "css selector",
        ".sheet-grid > .sheet-grid-slot > button.active .sheet-tile__number",
        "fresh Save As active sheet number",
      ),
    ),
  );
  const projectLocalSelectionReset = freshActiveSheetNumber === 1;
  if (!projectLocalSelectionReset) {
    throw new Error(
      "The Save As WebView inherited the previous local sheet selection",
    );
  }
  await click(
    hostDriver,
    "css selector",
    ".sheet-grid > .sheet-grid-slot:nth-child(2) > button",
    "Save As second sheet",
  );
  await waitFor(
    "Save As second sheet selection",
    async () =>
      Number(
        await elementText(
          hostDriver,
          await findElement(
            hostDriver,
            "css selector",
            ".sheet-grid > .sheet-grid-slot > button.active .sheet-tile__number",
            "Save As selected sheet number",
          ),
        ),
      ) === activeSheetNumber,
    timeoutMilliseconds,
  );
  const copiedDpi = await findAlbumInformationDpi(hostDriver, "Save As DPI");
  if ((await elementAttribute(hostDriver, copiedDpi, "value")) !== "360") {
    throw new Error(
      "The rebuilt Save As WebView did not adopt the copied projection",
    );
  }
  const copiedPageSource = await hostDriver.request(
    "GET",
    `/session/${hostDriver.sessionId}/source`,
  );
  sourcePathExposedToWebView ||= [
    projectPath,
    saveAsPath,
    exportPath,
    photoPath,
    processDataRoot,
  ].some((candidate) => sourceContainsNativePath(copiedPageSource, candidate));

  const undoAfterSaveAsCount = recordsFor("project_undo_completed").length;
  await selectApplicationMenuCommand(
    hostDriver,
    "Editar",
    "Desfazer",
    "Undo after Save As",
  );
  await waitForLogEvent(
    "project_undo_completed",
    undoAfterSaveAsCount + 1,
    "Undo after Save As terminal",
  );
  await waitFor(
    "Save As Undo projection",
    async () => {
      const dpi = await findAlbumInformationDpi(
        hostDriver,
        "DPI after Save As Undo",
      );
      return (await elementAttribute(hostDriver, dpi, "value")) === "300";
    },
    timeoutMilliseconds,
  );
  const redoAfterSaveAsCount = recordsFor("project_redo_completed").length;
  await selectApplicationMenuCommand(
    hostDriver,
    "Editar",
    "Refazer",
    "Redo after Save As",
  );
  await waitForLogEvent(
    "project_redo_completed",
    redoAfterSaveAsCount + 1,
    "Redo after Save As terminal",
  );
  await waitFor(
    "Save As Redo projection",
    async () => {
      const dpi = await findAlbumInformationDpi(
        hostDriver,
        "DPI after Save As Redo",
      );
      return (await elementAttribute(hostDriver, dpi, "value")) === "360";
    },
    timeoutMilliseconds,
  );
  const historyPreservedAfterSaveAs = true;

  const originalGlobalDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
  const originalHostDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
  const originalProjectDialogDebugPort = await findFreeTcpPortInRange(
    40_000,
    44_999,
  );
  const originalApplicationEnvironment = {
    ...process.env,
    MYALBUNS_PROCESS_GATE_DATA_ROOT: processDataRoot,
    MYALBUNS_DEV_GLOBAL_WEBVIEW_DEBUG_PORT: String(originalGlobalDebugPort),
    MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT: String(originalHostDebugPort),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DEBUG_PORT: String(
      originalProjectDialogDebugPort,
    ),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DATA_DIRECTORY: path.join(
      scratch,
      "original-project-dialog-webview",
    ),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${originalGlobalDebugPort}`,
  };
  const originalGlobalChild = spawn(applicationPath, [projectPath], {
    cwd: workspace,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
    env: originalApplicationEnvironment,
  });
  originalGlobal = await waitForProcessInstance(
    originalGlobalChild.pid,
    "simultaneous original Global",
  );
  originalHost = await waitForNewApplication(
    isHost,
    [firstHost, secondHost],
    "simultaneous original Project Host",
  );
  await waitForExit(originalGlobal, "simultaneous original Global handoff");
  await waitForHostUiReady(
    originalHost,
    "simultaneous original Project UI ready",
  );
  originalDriver = await startAttachedWebDriver(
    originalHostDebugPort,
    "simultaneous original Project Host",
    originalProjectDialogDebugPort,
  );
  await findElement(
    originalDriver,
    "css selector",
    ".app-shell",
    "simultaneous original Project UI",
  );
  const simultaneousOriginalDpi = await findAlbumInformationDpi(
    originalDriver,
    "simultaneous original DPI",
  );
  if (
    (await elementAttribute(
      originalDriver,
      simultaneousOriginalDpi,
      "value",
    )) !== "300"
  ) {
    throw new Error(
      "The simultaneous original did not retain its saved content",
    );
  }
  for (const label of ["Desfazer", "Refazer"]) {
    if (
      await applicationMenuCommandEnabled(
        originalDriver,
        "Editar",
        label,
        `${label} in simultaneous original`,
      )
    ) {
      throw new Error(`The simultaneous original retained ${label} history`);
    }
  }
  const simultaneousOriginalHistoryEmpty = true;
  const simultaneousHostsOpen =
    applicationProcesses().filter(isHost).length === 2 &&
    aliveProcessInstances([secondHost, originalHost]).length === 2;
  if (!simultaneousHostsOpen) {
    throw new Error(
      "The original and Save As copy were not open simultaneously",
    );
  }

  const copiedBytesBeforeOriginalSave = readFileSync(saveAsPath);
  await replaceAlbumInformationDpi(
    originalDriver,
    "320",
    "independent original DPI input",
  );
  await applyAlbumInformation(
    originalDriver,
    "independent original Album information action",
  );
  await waitFor(
    "independent original intent terminal",
    () =>
      recordsFor("project_intent_applied").some(
        (record) =>
          record.project_id === originalProjectId &&
          Number(record.revision) === 4,
      ),
    timeoutMilliseconds,
  );
  await selectApplicationMenuCommand(
    originalDriver,
    "Arquivo",
    "Salvar",
    "independent original Save",
  );
  await waitFor(
    "independent original Save terminal",
    () =>
      recordsFor("project_save_completed").some(
        (record) =>
          record.project_id === originalProjectId &&
          Number(record.revision) === 4,
      ),
    timeoutMilliseconds,
  );
  const independentlySavedOriginal = readFileSync(projectPath);
  const independentlySavedOriginalDocument = JSON.parse(
    independentlySavedOriginal.toString("utf8"),
  );
  if (
    independentlySavedOriginalDocument.projectId !== originalProjectId ||
    independentlySavedOriginalDocument.revision !== 4 ||
    independentlySavedOriginalDocument.project.document.dpi !== 320 ||
    !readFileSync(saveAsPath).equals(copiedBytesBeforeOriginalSave)
  ) {
    throw new Error(
      "Saving the simultaneous original crossed into the Save As copy",
    );
  }

  await replaceAlbumInformationDpi(
    hostDriver,
    "420",
    "independent copy DPI input",
  );
  await applyAlbumInformation(
    hostDriver,
    "independent copy Album information action",
  );
  await waitFor(
    "independent copy intent terminal",
    () =>
      recordsFor("project_intent_applied").some(
        (record) =>
          record.project_id === copiedProjectId &&
          Number(record.revision) === 7,
      ),
    timeoutMilliseconds,
  );
  await selectApplicationMenuCommand(
    hostDriver,
    "Arquivo",
    "Salvar",
    "independent copy Save",
  );
  await waitFor(
    "independent copy Save terminal",
    () =>
      recordsFor("project_save_completed").some(
        (record) =>
          record.project_id === copiedProjectId &&
          Number(record.revision) === 7,
      ),
    timeoutMilliseconds,
  );
  const independentlySavedCopy = readFileSync(saveAsPath);
  const independentlySavedCopyDocument = JSON.parse(
    independentlySavedCopy.toString("utf8"),
  );
  const isolatedIndependentSaves =
    independentlySavedCopyDocument.projectId === copiedProjectId &&
    independentlySavedCopyDocument.revision === 7 &&
    independentlySavedCopyDocument.project.document.dpi === 420 &&
    readFileSync(projectPath).equals(independentlySavedOriginal);
  if (!isolatedIndependentSaves) {
    throw new Error(
      "Saving the Save As copy crossed into the original Project",
    );
  }

  await selectApplicationMenuCommandUntilLogEvent(
    originalDriver,
    "Arquivo",
    "Fechar Projeto",
    "clean_project_close_requested",
    "simultaneous original close",
  );
  originalDriver = await disposeConfirmedWebDriver(originalDriver);
  await waitForExit(originalHost, "simultaneous original Project Host close");
  originalReplacementGlobal = await waitForNewApplication(
    (instance) => !isHost(instance),
    [firstGlobal, secondGlobal, originalGlobal],
    "simultaneous original replacement Global",
  );
  terminateProcessInstance(originalReplacementGlobal);
  await waitForExit(
    originalReplacementGlobal,
    "simultaneous original replacement Global cleanup",
  );

  await replaceAlbumInformationDpi(
    hostDriver,
    "360",
    "pending export DPI input after Save As",
  );
  await applyAlbumInformation(
    hostDriver,
    "pending export Album information action after Save As",
  );
  await waitFor(
    "pending export DPI terminal after Save As",
    () =>
      recordsFor("project_intent_applied").some(
        (record) =>
          record.project_id === copiedProjectId &&
          Number(record.revision) === 8,
      ),
    timeoutMilliseconds,
  );

  const exportStartedBeforeCancel = recordsFor("export_started").length;
  const processorBeforeCancel = exportProcessorAttempts().length;
  await selectApplicationMenuCommand(
    hostDriver,
    "Arquivo",
    "Exportar Lâmina…",
    "Export action",
  );
  const cancelledExport = driveNativeDialog(
    secondHost,
    "cancel",
    "Exportar Lâmina como JPEG",
  );
  if (
    cancelledExport.action !== "cancel" ||
    existsSync(exportPath) ||
    recordsFor("export_started").length !== exportStartedBeforeCancel ||
    exportProcessorAttempts().length !== processorBeforeCancel
  ) {
    throw new Error(
      `Cancelled Export crossed the ExportPipeline boundary: action=${cancelledExport.action}, ` +
        `targetExists=${existsSync(exportPath)}, ` +
        `exportStarted=${exportStartedBeforeCancel}->${recordsFor("export_started").length}, ` +
        `exportProcessorSpawned=${processorBeforeCancel}->${exportProcessorAttempts().length}`,
    );
  }

  await selectApplicationMenuCommandUntilLogEvent(
    hostDriver,
    "Arquivo",
    "Exportar Lâmina…",
    "native_save_dialog_opening",
    "Export retry action",
  );
  const emptyCacheBeforeExport = purgeOwnedCache(cacheRoot);
  const selectedExport = driveNativeDialog(
    secondHost,
    "select",
    "Exportar Lâmina como JPEG",
    exportPath,
  );
  if (selectedExport.action !== "select") {
    throw new Error("The JPEG destination was not confirmed");
  }
  await waitForLogEvent("export_completed", 1, "Export completion");
  await clickProjectDialogAction(
    hostDriver,
    "Exportação concluída",
    "Fechar",
    "close Export success feedback",
  );
  await waitFor(
    "exported JPEG",
    () => existsSync(exportPath),
    timeoutMilliseconds,
  );
  await waitForLogEvent("imaging_process_stopped", 1, "Processador terminal");
  const exported = readFileSync(exportPath);
  const emptyCacheAfterExport = summarizeOwnedCache(cacheRoot);
  const emptyCacheEvidence = assertEmptyCacheExport({
    previewArtifactCountBeforePurge: previewCacheBeforePurge.jpegCount,
    cacheEntryCountBeforeExport: emptyCacheBeforeExport.entryCount,
    cacheByteCountBeforeExport: emptyCacheBeforeExport.byteCount,
    cacheEntryCountAfterExport: emptyCacheAfterExport.entryCount,
    cacheByteCountAfterExport: emptyCacheAfterExport.byteCount,
  });
  const dimensions = jpegDimensions(exported);
  const sheetEvidence = assertDistinguishableSheetExport({
    document: savedDocument.project.document,
    sheets: savedDocument.project.sheets,
    visualDefaults: savedDocument.project.visualDefaults,
    expectedBackgroundRgb: personalizedBackgroundRgb,
    selectedSheetNumber: activeSheetNumber,
    exportedDpi: 360,
    jpegDimensions: dimensions,
  });
  if (
    !readFileSync(saveAsPath).equals(independentlySavedCopy) ||
    !readFileSync(projectPath).equals(independentlySavedOriginal)
  ) {
    throw new Error(
      "Export mutated either independently saved Project document",
    );
  }
  const liveDpiInput = await findAlbumInformationDpi(
    hostDriver,
    "live DPI input after Export",
  );
  if ((await elementAttribute(hostDriver, liveDpiInput, "value")) !== "360") {
    throw new Error("Export changed the pending DPI in the live Project");
  }
  const undoEnabledAfterExport = await applicationMenuCommandEnabled(
    hostDriver,
    "Editar",
    "Desfazer",
    "Undo action after Export",
  );
  if (!undoEnabledAfterExport) {
    throw new Error("Export changed the pending Project history");
  }
  const canvas = await findElement(
    hostDriver,
    "css selector",
    "canvas.pixi-canvas",
    "productive Canvas",
  );
  const screenshot = await hostDriver.request(
    "GET",
    `/session/${hostDriver.sessionId}/element/${encodeURIComponent(canvas)}/screenshot`,
  );
  writeFileSync(screenshotPath, Buffer.from(screenshot, "base64"));
  const canvasPhotoSample = await hostDriver.request(
    "POST",
    `/session/${hostDriver.sessionId}/execute/sync`,
    {
      script: `
        const canvas = arguments[0];
        const sheetHeightPx = arguments[1] / 1000;
        const bounds = canvas.getBoundingClientRect();
        const scale = (bounds.height - 48) / (sheetHeightPx + 24);
        return {
          cssWidth: bounds.width,
          cssHeight: bounds.height,
          // Keep the fidelity sample inside the Photo while avoiding the
          // editor-only spine rendered at the exact center of a double sheet.
          x: bounds.width * 0.45,
          y: 24 + (24 + sheetHeightPx / 2) * scale,
        };
      `,
      args: [
        { "element-6066-11e4-a52e-4f735466cecf": canvas },
        savedDocument.project.document.sheetHeightUm,
      ],
    },
  );
  if (!readFileSync(photoPath).equals(originalPhoto)) {
    throw new Error("Export modified the linked Photo Original");
  }

  const residentCanvasPreviewBeforeMissingOriginal =
    recordsFor("canvas_opaque_preview_texture_loaded").length >
    canvasPreviewCountBeforeReopen;
  let missingOriginalBlocked = false;
  let missingOriginalActionable = false;
  let cacheCouldNotProduceFalseSuccess = false;
  const missingOriginalProcessorCount = exportProcessorAttempts().length;
  unlinkSync(photoPath);
  try {
    await selectApplicationMenuCommandUntilLogEvent(
      hostDriver,
      "Arquivo",
      "Exportar Lâmina…",
      "native_save_dialog_opening",
      "missing-Original Export action",
    );
    const selectedMissingExport = driveNativeDialog(
      secondHost,
      "select",
      "Exportar Lâmina como JPEG",
      missingOriginalExportPath,
    );
    if (selectedMissingExport.action !== "select") {
      throw new Error(
        "The missing-Original Export destination was not confirmed",
      );
    }
    await waitForLogEvent("export_failed", 1, "missing-Original failure");
    await waitFor(
      "missing-Original Processador spawn",
      () =>
        exportProcessorAttempts().length === missingOriginalProcessorCount + 1,
      timeoutMilliseconds,
    );
    const missingOriginalAttempt = exportProcessorAttempts().at(-1);
    await waitFor(
      "missing-Original Processador terminal",
      () =>
        recordsFor("imaging_process_stopped").some(
          (record) =>
            Number(record.process_id) ===
            Number(missingOriginalAttempt.imaging_process_id),
        ),
      timeoutMilliseconds,
    );
    const failureText = await withProjectDialog(
      hostDriver,
      "actionable missing-Original message",
      async (dialogDriver) => {
        const failureDialog = await findElement(
          dialogDriver,
          "xpath",
          accessibleProjectDialogXpath("Exportação não concluída"),
          "actionable missing-Original message",
        );
        const text = await elementText(dialogDriver, failureDialog);
        await clickWhenEnabled(
          dialogDriver,
          "xpath",
          `${accessibleProjectDialogXpath("Exportação não concluída")}//button[normalize-space()=${xpathLiteral("Fechar")}]`,
          "close missing-Original feedback",
        );
        return text;
      },
    );
    missingOriginalBlocked = !existsSync(missingOriginalExportPath);
    missingOriginalActionable =
      failureText.includes("Religar") || failureText.includes("Religue");
    const missingOriginalFailure = recordsFor("export_failed").at(-1);
    cacheCouldNotProduceFalseSuccess =
      residentCanvasPreviewBeforeMissingOriginal &&
      missingOriginalFailure?.stage === "source_verification" &&
      missingOriginalBlocked;
    if (
      !missingOriginalBlocked ||
      !missingOriginalActionable ||
      !cacheCouldNotProduceFalseSuccess
    ) {
      throw new Error(
        `The missing Original was not blocked cleanly: ${failureText}`,
      );
    }
  } finally {
    copyFileSync(photoFixturePath, photoPath);
  }
  if (!readFileSync(photoPath).equals(originalPhoto)) {
    throw new Error(
      "The restored proof Original differs from the imported bytes",
    );
  }

  await ensureInspectorSectionExpanded(
    hostDriver,
    "Grade de Lâminas",
    "physical Album structure Grade",
  );
  const physicalAlbumBefore = await waitForSheetGrid(
    hostDriver,
    "physical Album structure baseline",
    (observation) =>
      observation.count === 3 &&
      observation.order.length === 3 &&
      observation.order[1] === observation.focusedSheetId,
  );
  const structuralIntentKinds = ["add_sheet", "reorder_sheet", "delete_sheet"];
  const structuralIntentCountsBefore = Object.fromEntries(
    structuralIntentKinds.map((intent) => [
      intent,
      projectIntentRecords(secondHost.processId, intent).length,
    ]),
  );

  await selectApplicationMenuCommand(
    hostDriver,
    "Lâmina",
    "Adicionar depois",
    "Add Sheet after the focused Sheet",
  );
  const addSheetEvent = await waitForProjectIntent(
    secondHost.processId,
    "add_sheet",
    structuralIntentCountsBefore.add_sheet + 1,
    "ProjectCore Add Sheet event",
  );
  const physicalAlbumAfterAdd = await waitForSheetGrid(
    hostDriver,
    "four-Sheet Album after Add",
    (observation) =>
      observation.count === 4 &&
      observation.order.length === 4 &&
      observation.focusedSheetId !== null &&
      !physicalAlbumBefore.order.includes(observation.focusedSheetId) &&
      observation.order[0] === physicalAlbumBefore.order[0] &&
      observation.order[1] === physicalAlbumBefore.order[1] &&
      observation.order[2] === observation.focusedSheetId &&
      observation.order[3] === physicalAlbumBefore.order[2],
  );
  const addedSheetId = physicalAlbumAfterAdd.focusedSheetId;

  await dragSheetInGrid(
    hostDriver,
    addedSheetId,
    physicalAlbumBefore.focusedSheetId,
    "Reorder added Sheet through the Grade",
  );
  const reorderSheetEvent = await waitForProjectIntent(
    secondHost.processId,
    "reorder_sheet",
    structuralIntentCountsBefore.reorder_sheet + 1,
    "ProjectCore Reorder Sheet event",
  );
  const expectedReorderedSheetIds = [
    physicalAlbumBefore.order[0],
    addedSheetId,
    physicalAlbumBefore.order[1],
    physicalAlbumBefore.order[2],
  ];
  const physicalAlbumAfterReorder = await waitForSheetGrid(
    hostDriver,
    "four-Sheet Album after Grade reorder",
    (observation) =>
      observation.count === 4 &&
      observation.focusedSheetId === addedSheetId &&
      observation.order.join(",") === expectedReorderedSheetIds.join(","),
  );

  await selectApplicationMenuCommand(
    hostDriver,
    "Lâmina",
    "Excluir",
    "Delete reordered Sheet",
  );
  const deleteSheetEvent = await waitForProjectIntent(
    secondHost.processId,
    "delete_sheet",
    structuralIntentCountsBefore.delete_sheet + 1,
    "ProjectCore Delete Sheet event",
  );
  const physicalAlbumAfterDelete = await waitForSheetGrid(
    hostDriver,
    "original three-Sheet Album after Delete",
    (observation) =>
      observation.count === 3 &&
      observation.focusedSheetId === physicalAlbumBefore.focusedSheetId &&
      observation.order.join(",") === physicalAlbumBefore.order.join(","),
  );

  for (const intent of structuralIntentKinds) {
    const expectedCount = structuralIntentCountsBefore[intent] + 1;
    const observedCount = projectIntentRecords(
      secondHost.processId,
      intent,
    ).length;
    if (observedCount !== expectedCount) {
      throw new Error(
        `The physical Album structure journey expected one ${intent} event and observed ${observedCount - structuralIntentCountsBefore[intent]}`,
      );
    }
  }
  const projectCoreEvents = assertPhysicalAlbumProjectCoreEvents(
    [addSheetEvent, reorderSheetEvent, deleteSheetEvent],
    {
      hostProcessId: secondHost.processId,
      intents: structuralIntentKinds,
    },
  );
  const physicalAlbumStructure = {
    reorderSurface: "grid",
    dragTransport: "w3c-pointer-actions",
    addedSheetId,
    before: physicalAlbumBefore,
    afterAdd: physicalAlbumAfterAdd,
    afterReorder: physicalAlbumAfterReorder,
    afterDelete: physicalAlbumAfterDelete,
    restoredOriginalOrder: true,
    projectCoreEvents,
  };

  await selectApplicationMenuCommandUntilLogEvent(
    hostDriver,
    "Arquivo",
    "Fechar Projeto",
    "dirty_project_close_confirmation_required",
    "Close Project action",
  );
  await clickProjectDialogAction(
    hostDriver,
    "Salvar alterações antes de fechar?",
    "Descartar e fechar",
    "Discard pending DPI action",
  ).catch(async (error) => {
    if (recordsFor("project_close_discarded").length === 0) throw error;
  });
  hostDriver = await disposeConfirmedWebDriver(hostDriver);
  await waitForExit(secondHost, "reopened Project Host close");

  finalGlobal = await waitForNewApplication(
    (instance) => !isHost(instance),
    [
      firstGlobal,
      secondGlobal,
      recoveryGlobal,
      originalGlobal,
      originalReplacementGlobal,
    ],
    "final Global",
  );
  terminateProcessInstance(finalGlobal);
  await waitForExit(finalGlobal, "final Global cleanup");

  copyFileSync(projectPath, externalCopyPath);
  const readonlyResult = spawnSync("attrib.exe", ["+R", externalCopyPath], {
    cwd: workspace,
    windowsHide: true,
    encoding: "utf8",
  });
  if (readonlyResult.status !== 0) {
    throw new Error(
      `The external-copy fixture could not become read-only: ${readonlyResult.stderr || readonlyResult.stdout}`,
    );
  }
  const externalSourceBytes = readFileSync(externalCopyPath);
  const externalSourceDocument = JSON.parse(
    externalSourceBytes.toString("utf8"),
  );
  const externalGlobalDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
  const externalHostDebugPort = await findFreeTcpPortInRange(40_000, 44_999);
  const externalProjectDialogDebugPort = await findFreeTcpPortInRange(
    40_000,
    44_999,
  );
  const externalApplicationEnvironment = {
    ...process.env,
    MYALBUNS_PROCESS_GATE_DATA_ROOT: processDataRoot,
    MYALBUNS_DEV_GLOBAL_WEBVIEW_DEBUG_PORT: String(externalGlobalDebugPort),
    MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT: String(externalHostDebugPort),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DEBUG_PORT: String(
      externalProjectDialogDebugPort,
    ),
    MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DATA_DIRECTORY: path.join(
      scratch,
      "external-copy-project-dialog-webview",
    ),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${externalGlobalDebugPort}`,
  };
  const externalGlobalChild = spawn(applicationPath, [externalCopyPath], {
    cwd: workspace,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
    env: externalApplicationEnvironment,
  });
  externalCopyGlobal = await waitForProcessInstance(
    externalGlobalChild.pid,
    "external-copy Global",
  );
  cancelledExternalCopyHost = await waitForNewApplication(
    isHost,
    [firstHost, crashedHost, secondHost, originalHost],
    "first cancellable external-copy Project Host",
  );
  const externalProjectWindowTitleBeforeDecision = nativeWindowTitle(
    cancelledExternalCopyHost,
  );
  const externalHostWebViewBeforeDecision = await httpAvailable(
    `http://127.0.0.1:${externalHostDebugPort}/json/version`,
  );
  const externalTargetSnapshot = await waitFor(
    "external-copy opening dialog target",
    async () => {
      const ownerTargets = await devToolsTargets(
        externalGlobalDebugPort,
        "external-copy Global opening owner",
      );
      const globalTargets = ownerTargets
        .filter((target) => target.type === "page")
        .filter((target) => target.url.includes("global.html"));
      const decisionTargets = ownerTargets
        .filter((target) => target.type === "page")
        .filter((target) => {
          try {
            const url = new URL(target.url);
            return (
              url.pathname.endsWith("/dialog.html") &&
              url.searchParams.get("kind") === "external-copy"
            );
          } catch {
            return false;
          }
        });
      return globalTargets.length >= 1 && decisionTargets.length === 1
        ? { globalTargets, decisionTargets }
        : undefined;
    },
    timeoutMilliseconds,
  );
  externalCopyDialogDriver = await startAttachedWebDriver(
    externalGlobalDebugPort,
    "external-copy opening dialog",
  );
  await switchToWebDriverWindow(
    externalCopyDialogDriver,
    (url) => {
      const parsed = new URL(url);
      return (
        parsed.pathname.endsWith("/dialog.html") &&
        parsed.searchParams.get("kind") === "external-copy"
      );
    },
    "external-copy decision WebView",
  );
  await findElement(
    externalCopyDialogDriver,
    "css selector",
    ".ui-owned-window-shell [role='dialog']",
    "external-copy decision",
  );
  const externalCopyPresentation = await externalCopyDialogDriver.request(
    "POST",
    `/session/${externalCopyDialogDriver.sessionId}/execute/sync`,
    {
      script: `
        const dialog = document.querySelector('[role="dialog"]');
        const title = dialog?.getAttribute('aria-labelledby');
        const transition = document.querySelector('[data-opening-owner-transition]');
        const shell = document.querySelector('.ui-owned-window-shell');
        return {
          ariaModal: dialog?.getAttribute('aria-modal') ?? null,
          choices: Array.from(dialog?.querySelectorAll('button') ?? [])
            .map((button) => button.textContent.trim()),
          contentFitted:
            shell !== null &&
            Math.abs(document.documentElement.clientHeight - shell.scrollHeight) <= 2,
          dialogCount: document.querySelectorAll('[role="dialog"]').length,
          externalDialog:
            window.location.pathname.endsWith('/dialog.html') &&
            new URLSearchParams(window.location.search).get('kind') === 'external-copy',
          initialFocus: document.activeElement?.textContent?.trim() ?? null,
          modalLayerCount: document.querySelectorAll('.ui-modal-dialog-layer').length,
          openedFromLoadingOwner:
            transition?.getAttribute('data-opening-owner-transition') === 'true',
          ownerSurfaceCount: document.querySelectorAll('[data-project-owner-surface]').length,
          ownedShellCount: document.querySelectorAll('.ui-owned-window-shell').length,
          title: title ? document.getElementById(title)?.textContent?.trim() ?? null : null,
          viewportWidth: document.documentElement.clientWidth,
        };
      `,
      args: [],
    },
  );
  const externalChoices = ["Cancelar", "Salvar cópia como…"];
  if (
    JSON.stringify(externalCopyPresentation.choices) !==
      JSON.stringify(externalChoices) ||
    externalCopyPresentation.dialogCount !== 1 ||
    externalCopyPresentation.modalLayerCount !== 0 ||
    externalCopyPresentation.ownerSurfaceCount !== 0 ||
    externalCopyPresentation.ownedShellCount !== 1 ||
    externalCopyPresentation.ariaModal !== "true" ||
    externalCopyPresentation.title !== "Cópia externa somente leitura" ||
    externalCopyPresentation.initialFocus !== "Salvar cópia como…" ||
    !externalCopyPresentation.contentFitted ||
    externalCopyPresentation.viewportWidth !== 440 ||
    !externalCopyPresentation.externalDialog ||
    !externalCopyPresentation.openedFromLoadingOwner ||
    externalProjectWindowTitleBeforeDecision !== "" ||
    externalHostWebViewBeforeDecision
  ) {
    throw new Error(
      `The external-copy decision did not remain in the stable opening owner: ${JSON.stringify(
        {
          ...externalCopyPresentation,
          externalHostWebViewBeforeDecision,
          externalProjectWindowTitleBeforeDecision,
        },
      )}`,
    );
  }

  const firstExternalNativeOwner = nativeOwnedWindowState(externalCopyGlobal);
  const firstExternalOwnerWasReplaced =
    firstExternalNativeOwner.dialogCount === 1 &&
    firstExternalNativeOwner.dialog?.visible === true &&
    firstExternalNativeOwner.dialog?.enabled === true &&
    firstExternalNativeOwner.owner?.visible === false &&
    firstExternalNativeOwner.owner?.enabled === false;
  if (!firstExternalOwnerWasReplaced) {
    const applicationWindowStates = applicationProcesses().map((instance) => ({
      instance,
      nativeState: nativeOwnedWindowState(instance),
    }));
    throw new Error(
      `The external-copy decision did not replace and block its native Global owner: ${JSON.stringify(
        {
          expectedGlobal: externalCopyGlobal,
          expectedState: firstExternalNativeOwner,
          applicationWindowStates,
        },
      )}`,
    );
  }

  await click(
    externalCopyDialogDriver,
    "xpath",
    "//button[normalize-space()='Cancelar']",
    "first external-copy cancellation",
  );
  externalCopyDialogDriver = await disposeConfirmedWebDriver(
    externalCopyDialogDriver,
  );
  await waitForExit(
    cancelledExternalCopyHost,
    "cancelled external-copy pending Host cleanup",
  );
  const nativeOwnerAfterExternalCancel = await waitFor(
    "Global owner restoration after external-copy cancellation",
    () => {
      const state = nativeOwnedWindowState(externalCopyGlobal);
      return state.dialogCount === 0 &&
        state.windows.some(
          (window) =>
            window.ownerHwnd === 0 && window.visible && window.enabled,
        )
        ? state
        : undefined;
    },
    timeoutMilliseconds,
  );
  const cancelRestoredGlobalAndCleanedHost =
    aliveProcessInstances([externalCopyGlobal]).length === 1 &&
    aliveProcessInstances([cancelledExternalCopyHost]).length === 0 &&
    nativeOwnerAfterExternalCancel.dialogCount === 0;

  const externalActivationBatchCount = recordsFor(
    "global_activation_batch_completed",
  ).length;
  const firstForwardedExternalActivationCount = recordsFor(
    "global_activation_forwarded",
  ).length;
  const firstForwardedExternalActivation = spawn(
    applicationPath,
    [externalCopyPath],
    {
      cwd: workspace,
      windowsHide: true,
      stdio: "ignore",
      env: externalApplicationEnvironment,
    },
  );
  await waitForLogEvent(
    "global_activation_forwarded",
    firstForwardedExternalActivationCount + 1,
    "first real-path external-copy activation",
  );
  await waitFor(
    "first forwarded external-copy client terminal",
    () =>
      firstForwardedExternalActivation.exitCode !== null ||
      firstForwardedExternalActivation.signalCode !== null,
    timeoutMilliseconds,
  );
  queuedExternalCopyHost = await waitForNewApplication(
    isHost,
    [
      firstHost,
      crashedHost,
      secondHost,
      originalHost,
      cancelledExternalCopyHost,
    ],
    "queued external-copy Project Host",
  );
  const queuedExternalTargetSnapshot = await waitFor(
    "queued external-copy opening dialog target",
    async () => {
      const targets = await devToolsTargets(
        externalGlobalDebugPort,
        "queued external-copy opening owner",
      );
      const decisionTargets = targets.filter((target) => {
        try {
          const url = new URL(target.url);
          return (
            url.pathname.endsWith("/dialog.html") &&
            url.searchParams.get("kind") === "external-copy" &&
            target.id !== externalTargetSnapshot.decisionTargets[0].id
          );
        } catch {
          return false;
        }
      });
      return decisionTargets.length === 1 ? decisionTargets[0] : undefined;
    },
    timeoutMilliseconds,
  );
  externalCopyDialogDriver = await startAttachedWebDriver(
    externalGlobalDebugPort,
    "queued external-copy opening dialog",
  );
  await switchToWebDriverWindow(
    externalCopyDialogDriver,
    (url) => {
      const parsed = new URL(url);
      return (
        parsed.pathname.endsWith("/dialog.html") &&
        parsed.searchParams.get("kind") === "external-copy"
      );
    },
    "queued external-copy decision WebView",
  );
  await findElement(
    externalCopyDialogDriver,
    "css selector",
    ".ui-owned-window-shell [role='dialog']",
    "queued external-copy decision",
  );

  const secondForwardedExternalActivationCount = recordsFor(
    "global_activation_forwarded",
  ).length;
  const queuedExternalActivationChild = spawn(
    applicationPath,
    [externalCopyPath],
    {
      cwd: workspace,
      windowsHide: true,
      stdio: "ignore",
      env: externalApplicationEnvironment,
    },
  );
  await waitFor(
    "queued external-copy activation client terminal",
    () =>
      queuedExternalActivationChild.exitCode !== null ||
      queuedExternalActivationChild.signalCode !== null,
    timeoutMilliseconds,
  );
  await delay(300);
  const targetsWithQueuedExternalActivation = await devToolsTargets(
    externalGlobalDebugPort,
    "external-copy owner with queued path activation",
  );
  const queuedExternalTarget = targetsWithQueuedExternalActivation.find(
    (target) =>
      target.id === queuedExternalTargetSnapshot.id &&
      target.url === queuedExternalTargetSnapshot.url,
  );
  const queuedExternalDialogCount = await externalCopyDialogDriver.request(
    "POST",
    `/session/${externalCopyDialogDriver.sessionId}/execute/sync`,
    {
      script: "return document.querySelectorAll('[role=\"dialog\"]').length;",
      args: [],
    },
  );
  const queuedExternalNativeOwner = nativeOwnedWindowState(externalCopyGlobal);
  const activeExternalHosts = applicationProcesses().filter(isHost);
  const queuedActivationPreservedExternalOwner =
    queuedExternalTarget !== undefined &&
    queuedExternalDialogCount === 1 &&
    queuedExternalNativeOwner.dialogCount === 1 &&
    queuedExternalNativeOwner.dialog?.enabled === true &&
    queuedExternalNativeOwner.owner?.visible === false &&
    queuedExternalNativeOwner.owner?.enabled === false &&
    activeExternalHosts.length === 1 &&
    sameProcessInstance(activeExternalHosts[0], queuedExternalCopyHost);
  if (!queuedActivationPreservedExternalOwner) {
    throw new Error(
      `A real-path activation replaced the external-copy owner or duplicated its pending Host: ${JSON.stringify({
        expectedTarget: queuedExternalTargetSnapshot,
        observedTarget: queuedExternalTarget,
        queuedExternalDialogCount,
        nativeOwner: queuedExternalNativeOwner,
        expectedHost: queuedExternalCopyHost,
        activeHosts: activeExternalHosts,
      })}`,
    );
  }

  await click(
    externalCopyDialogDriver,
    "xpath",
    "//button[normalize-space()='Cancelar']",
    "queued external-copy cancellation",
  );
  externalCopyDialogDriver = await disposeConfirmedWebDriver(
    externalCopyDialogDriver,
  );
  await waitForExit(
    queuedExternalCopyHost,
    "queued external-copy pending Host cleanup",
  );
  await waitForLogEvent(
    "global_activation_forwarded",
    secondForwardedExternalActivationCount + 1,
    "real-path activation consumed after the preceding decision terminal",
  );
  await waitForLogEvent(
    "global_activation_batch_completed",
    externalActivationBatchCount + 1,
    "cancelled queued external-copy activation terminal",
  );

  externalCopyHost = await waitForNewApplication(
    isHost,
    [
      firstHost,
      crashedHost,
      secondHost,
      originalHost,
      cancelledExternalCopyHost,
      queuedExternalCopyHost,
    ],
    "serial external-copy retry Host",
  );
  const serialExternalTargetSnapshot = await waitFor(
    "serial external-copy retry dialog target",
    async () => {
      const targets = await devToolsTargets(
        externalGlobalDebugPort,
        "serial external-copy retry owner",
      );
      const decisionTargets = targets.filter((target) => {
        try {
          const url = new URL(target.url);
          return (
            url.pathname.endsWith("/dialog.html") &&
            url.searchParams.get("kind") === "external-copy" &&
            target.id !== queuedExternalTargetSnapshot.id
          );
        } catch {
          return false;
        }
      });
      return decisionTargets.length === 1 ? decisionTargets[0] : undefined;
    },
    timeoutMilliseconds,
  );
  externalCopyDialogDriver = await startAttachedWebDriver(
    externalGlobalDebugPort,
    "serial external-copy retry dialog",
  );
  await switchToWebDriverWindow(
    externalCopyDialogDriver,
    (url) => {
      const parsed = new URL(url);
      return (
        parsed.pathname.endsWith("/dialog.html") &&
        parsed.searchParams.get("kind") === "external-copy"
      );
    },
    "serial external-copy retry WebView",
  );
  await findElement(
    externalCopyDialogDriver,
    "css selector",
    ".ui-owned-window-shell [role='dialog']",
    "serial external-copy retry decision",
  );
  const externalAttemptBeforePickerCancel = new URL(
    serialExternalTargetSnapshot.url,
  ).searchParams.get("attemptId");
  const retryNativeOwnerBeforePicker =
    nativeOwnedWindowState(externalCopyGlobal);

  await clickUntilLogEvent(
    externalCopyDialogDriver,
    "xpath",
    "//button[normalize-space()='Salvar cópia como…']",
    "native_save_dialog_opening",
    "external-copy first Save Copy As action",
  );
  const cancelledExternalCopyPicker = driveNativeDialog(
    externalCopyGlobal,
    "cancel",
    "Criar Projeto MyAlbuns",
  );
  await findElement(
    externalCopyDialogDriver,
    "css selector",
    ".ui-owned-window-shell [role='dialog']",
    "external-copy decision after native picker cancellation",
  );
  const externalAttemptAfterPickerCancel =
    await externalCopyDialogDriver.request(
      "POST",
      `/session/${externalCopyDialogDriver.sessionId}/execute/sync`,
      {
        script:
          "return new URLSearchParams(window.location.search).get('attemptId');",
        args: [],
      },
    );
  const retryNativeOwnerAfterPicker =
    nativeOwnedWindowState(externalCopyGlobal);
  const pickerCancellationPreservedAttempt =
    cancelledExternalCopyPicker.action === "cancel" &&
    externalAttemptBeforePickerCancel !== null &&
    externalAttemptAfterPickerCancel === externalAttemptBeforePickerCancel &&
    aliveProcessInstances([externalCopyHost]).length === 1 &&
    applicationProcesses().filter(isHost).length === 1 &&
    retryNativeOwnerBeforePicker.dialog?.ownerHwnd ===
      retryNativeOwnerAfterPicker.dialog?.ownerHwnd &&
    retryNativeOwnerAfterPicker.dialogCount === 1 &&
    retryNativeOwnerAfterPicker.owner?.visible === false &&
    retryNativeOwnerAfterPicker.owner?.enabled === false;
  if (!pickerCancellationPreservedAttempt) {
    throw new Error(
      "Cancelling the external-copy picker did not return to the same opening owner, attempt, and Host",
    );
  }

  const emptyActivationForwardedCount = recordsFor(
    "global_activation_forwarded",
  ).length;
  const queuedEmptyActivationChild = spawn(applicationPath, [], {
    cwd: workspace,
    windowsHide: true,
    stdio: "ignore",
    env: externalApplicationEnvironment,
  });
  await waitFor(
    "queued pathless activation client terminal",
    () =>
      queuedEmptyActivationChild.exitCode !== null ||
      queuedEmptyActivationChild.signalCode !== null,
    timeoutMilliseconds,
  );
  await delay(300);
  const nativeOwnerWithQueuedEmptyActivation =
    nativeOwnedWindowState(externalCopyGlobal);
  const emptyActivationPreservedPendingOwner =
    nativeOwnerWithQueuedEmptyActivation.dialogCount === 1 &&
    nativeOwnerWithQueuedEmptyActivation.dialog?.ownerHwnd ===
      retryNativeOwnerAfterPicker.dialog?.ownerHwnd &&
    nativeOwnerWithQueuedEmptyActivation.owner?.visible === false &&
    nativeOwnerWithQueuedEmptyActivation.owner?.enabled === false &&
    aliveProcessInstances([externalCopyHost]).length === 1;
  if (!emptyActivationPreservedPendingOwner) {
    throw new Error(
      "A queued pathless activation altered the pending external-copy owner",
    );
  }

  await clickUntilLogEvent(
    externalCopyDialogDriver,
    "xpath",
    "//button[normalize-space()='Salvar cópia como…']",
    "native_save_dialog_opening",
    "external-copy retried Save Copy As action",
  );
  const selectedExternalCopy = driveNativeDialog(
    externalCopyGlobal,
    "select",
    "Criar Projeto MyAlbuns",
    externalSavedCopyPath,
  );
  if (selectedExternalCopy.action !== "select") {
    throw new Error("The external-copy destination was not confirmed");
  }
  await waitForLogEvent(
    "global_activation_forwarded",
    emptyActivationForwardedCount + 1,
    "pathless activation consumed after successful external-copy handoff",
  );
  await waitForExit(externalCopyGlobal, "external-copy Global handoff");
  await waitForLogEvent(
    "global_activation_batch_completed",
    externalActivationBatchCount + 2,
    "serial external-copy activation handoff terminal",
  );
  const emptyActivationDidNotResurrectGlobal =
    aliveProcessInstances([externalCopyGlobal]).length === 0;
  externalCopyDialogDriver = await disposeConfirmedWebDriver(
    externalCopyDialogDriver,
  );
  await waitForHostUiReady(
    externalCopyHost,
    "external-copy Project UI ready in the pending Host",
  );
  externalCopyHostDriver = await startAttachedWebDriver(
    externalHostDebugPort,
    "external-copy Project Host",
    externalProjectDialogDebugPort,
  );
  await findElement(
    externalCopyHostDriver,
    "css selector",
    ".app-shell",
    "external-copy Project UI",
  );
  const externalSavedDocument = JSON.parse(
    readFileSync(externalSavedCopyPath).toString("utf8"),
  );
  const externalSourcePreserved =
    readFileSync(externalCopyPath).equals(externalSourceBytes);
  const samePendingHostCompletedHandoff =
    applicationProcesses().filter(isHost).length === 1 &&
    aliveProcessInstances([externalCopyHost]).length === 1;
  const externalActivationBatches = recordsFor(
    "global_activation_batch_completed",
  ).slice(externalActivationBatchCount);
  const realPathActivationsCompletedSerially =
    externalActivationBatches.length === 2 &&
    Number(externalActivationBatches[0]?.failed_count) === 1 &&
    Number(externalActivationBatches[1]?.opened_count) === 1 &&
    aliveProcessInstances([cancelledExternalCopyHost, queuedExternalCopyHost])
      .length === 0 &&
    new Set([
      cancelledExternalCopyHost.processId,
      queuedExternalCopyHost.processId,
      externalCopyHost.processId,
    ]).size === 3;
  if (
    !externalSourcePreserved ||
    !samePendingHostCompletedHandoff ||
    !realPathActivationsCompletedSerially ||
    externalSavedDocument.projectId === externalSourceDocument.projectId ||
    externalSavedDocument.revision !== externalSourceDocument.revision
  ) {
    throw new Error(
      "Saving the external copy changed its source, revision, identity isolation, or pending Host",
    );
  }
  const graphicsMutationCount = recordsFor("project_intent_applied").length;
  await replaceAlbumInformationDpi(
    externalCopyHostDriver,
    "420",
    "graphics-failure dirty DPI input",
  );
  await applyAlbumInformation(
    externalCopyHostDriver,
    "graphics-failure dirty Album information action",
  );
  await waitForLogEvent(
    "project_intent_applied",
    graphicsMutationCount + 1,
    "graphics-failure dirty Project mutation",
  );

  const contextLostCount = recordsFor("canvas_context_lost").length;
  const contextRestoreFailedCount = recordsFor(
    "canvas_context_restore_failed",
  ).length;
  const contextLossDispatched = await externalCopyHostDriver.request(
    "POST",
    `/session/${externalCopyHostDriver.sessionId}/execute/sync`,
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
    throw new Error(
      "The productive Canvas did not accept the context-loss event",
    );
  }
  await waitForLogEvent(
    "canvas_context_lost",
    contextLostCount + 1,
    "productive Canvas context loss",
  );
  await waitForLogEvent(
    "canvas_context_restore_failed",
    contextRestoreFailedCount + 1,
    "productive Canvas restore timeout",
  );
  const blockedWorkspaceAfterGraphicsFailure = await waitFor(
    "immediate Project interaction block after graphics failure",
    async () => {
      const state = await externalCopyHostDriver.request(
        "POST",
        `/session/${externalCopyHostDriver.sessionId}/execute/sync`,
        {
          script: `
            const grid = document.querySelector('.workspace-grid');
            const exportButton = document.querySelector("button[aria-label='Exportar Lâmina']");
            return {
              canvasStillMounted: document.querySelector('canvas.pixi-canvas') !== null,
              exportDisabled: exportButton?.disabled === true,
              inlineFailureCount: document.querySelectorAll('.startup-surface [role="alert"]').length,
              workspaceBusy: grid?.getAttribute('aria-busy') === 'true',
              workspaceInert: grid?.hasAttribute('inert') === true,
            };
          `,
          args: [],
        },
      );
      return state.workspaceInert && state.workspaceBusy && state.exportDisabled
        ? state
        : undefined;
    },
    timeoutMilliseconds,
  );
  const graphicsDialogTarget = await waitForProjectDialogStateTarget(
    externalProjectDialogDebugPort,
    "graphicsFailure",
    "graphics-failure Project dialog target",
  );
  const graphicsPresentation = await withProjectDialog(
    externalCopyHostDriver,
    "graphics-failure presentation",
    (dialogDriver) =>
      dialogDriver.request(
        "POST",
        `/session/${dialogDriver.sessionId}/execute/sync`,
        {
          script: `
            const dialog = document.querySelector('[role="dialog"]');
            const titleId = dialog?.getAttribute('aria-labelledby');
            return {
              ariaModal: dialog?.getAttribute('aria-modal') ?? null,
              dialogCount: document.querySelectorAll('[role="dialog"]').length,
              externalProjectDialog: window.location.pathname.endsWith('/project-dialog.html'),
              title: titleId ? document.getElementById(titleId)?.textContent?.trim() ?? null : null,
            };
          `,
          args: [],
        },
      ),
  );
  const graphicsNativeOwner = nativeOwnedWindowState(externalCopyHost);
  const graphicsDialogOwnedAndProjectBlocked =
    graphicsDialogTarget !== undefined &&
    graphicsPresentation.dialogCount === 1 &&
    graphicsPresentation.ariaModal === "true" &&
    graphicsPresentation.externalProjectDialog &&
    graphicsPresentation.title === "O Canvas não pôde ser iniciado" &&
    graphicsNativeOwner.dialogCount === 1 &&
    graphicsNativeOwner.dialog?.visible === true &&
    graphicsNativeOwner.dialog?.enabled === true &&
    graphicsNativeOwner.owner?.visible === true &&
    graphicsNativeOwner.owner?.enabled === false &&
    blockedWorkspaceAfterGraphicsFailure.canvasStillMounted &&
    blockedWorkspaceAfterGraphicsFailure.inlineFailureCount === 0;
  if (!graphicsDialogOwnedAndProjectBlocked) {
    throw new Error(
      `The late graphics failure did not use one external Project-owned dialog: ${JSON.stringify(
        {
          blockedWorkspaceAfterGraphicsFailure,
          graphicsNativeOwner,
          graphicsPresentation,
          targetCount: graphicsDialogTarget ? 1 : 0,
        },
      )}`,
    );
  }

  const dirtyGraphicsCloseCount = recordsFor(
    "dirty_project_close_confirmation_required",
  ).length;
  await clickProjectDialogAction(
    externalCopyHostDriver,
    "O Canvas não pôde ser iniciado",
    "Fechar Projeto",
    "graphics-failure close request",
  );
  await waitForLogEvent(
    "dirty_project_close_confirmation_required",
    dirtyGraphicsCloseCount + 1,
    "dirty close confirmation after graphics failure",
  );
  const firstGraphicsCloseTarget = await waitForProjectDialogStateTarget(
    externalProjectDialogDebugPort,
    "projectCloseConfirmation",
    "dirty close confirmation Project dialog target",
    [graphicsDialogTarget.id],
  );
  const cancelledGraphicsCloseCount = recordsFor(
    "project_close_cancelled",
  ).length;
  await clickProjectDialogAction(
    externalCopyHostDriver,
    "Salvar alterações antes de fechar?",
    "Cancelar",
    "cancel dirty close after graphics failure",
  );
  await waitForLogEvent(
    "project_close_cancelled",
    cancelledGraphicsCloseCount + 1,
    "cancelled dirty close after graphics failure",
  );
  const rearmedGraphicsTarget = await waitForProjectDialogStateTarget(
    externalProjectDialogDebugPort,
    "graphicsFailure",
    "rearmed graphics-failure Project dialog target",
    [graphicsDialogTarget.id, firstGraphicsCloseTarget.id],
  );
  const rearmedGraphicsPresentation = await withProjectDialog(
    externalCopyHostDriver,
    "rearmed graphics-failure presentation",
    async (dialogDriver) => {
      await findElement(
        dialogDriver,
        "xpath",
        accessibleProjectDialogXpath("O Canvas não pôde ser iniciado"),
        "rearmed graphics-failure dialog",
      );
      return dialogDriver.request(
        "POST",
        `/session/${dialogDriver.sessionId}/execute/sync`,
        {
          script:
            "return document.querySelectorAll('[role=\"dialog\"]').length;",
          args: [],
        },
      );
    },
  );
  const rearmedGraphicsNativeOwner = nativeOwnedWindowState(externalCopyHost);
  const cancelledCloseRearmedSingleGraphicsDialog =
    rearmedGraphicsTarget !== undefined &&
    rearmedGraphicsPresentation === 1 &&
    rearmedGraphicsNativeOwner.dialogCount === 1 &&
    rearmedGraphicsNativeOwner.owner?.visible === true &&
    rearmedGraphicsNativeOwner.owner?.enabled === false &&
    aliveProcessInstances([externalCopyHost]).length === 1;
  if (!cancelledCloseRearmedSingleGraphicsDialog) {
    throw new Error(
      "Cancelling dirty Project close did not rearm exactly one graphics-failure dialog",
    );
  }

  const secondDirtyGraphicsCloseCount = recordsFor(
    "dirty_project_close_confirmation_required",
  ).length;
  await clickProjectDialogAction(
    externalCopyHostDriver,
    "O Canvas não pôde ser iniciado",
    "Fechar Projeto",
    "second graphics-failure close request",
  );
  await waitForLogEvent(
    "dirty_project_close_confirmation_required",
    secondDirtyGraphicsCloseCount + 1,
    "second dirty close confirmation after graphics failure",
  );
  await waitForProjectDialogStateTarget(
    externalProjectDialogDebugPort,
    "projectCloseConfirmation",
    "second dirty close confirmation Project dialog target",
    [
      graphicsDialogTarget.id,
      firstGraphicsCloseTarget.id,
      rearmedGraphicsTarget.id,
    ],
  );
  await clickProjectDialogAction(
    externalCopyHostDriver,
    "Salvar alterações antes de fechar?",
    "Descartar e fechar",
    "discard dirty Project after graphics failure",
  ).catch(async (error) => {
    if (recordsFor("project_close_discarded").length === 0) throw error;
  });
  externalCopyHostDriver = await disposeConfirmedWebDriver(
    externalCopyHostDriver,
  );
  await waitForExit(
    externalCopyHost,
    "external-copy Project Host close after graphics failure",
  );
  const graphicsFailureTerminalCleanedHost =
    aliveProcessInstances([externalCopyHost]).length === 0;
  externalCopyReplacementGlobal = await waitForNewApplication(
    (instance) => !isHost(instance),
    [
      firstGlobal,
      secondGlobal,
      recoveryGlobal,
      originalGlobal,
      originalReplacementGlobal,
      finalGlobal,
      externalCopyGlobal,
    ],
    "external-copy replacement Global",
  );
  terminateProcessInstance(externalCopyReplacementGlobal);
  await waitForExit(
    externalCopyReplacementGlobal,
    "external-copy replacement Global cleanup",
  );

  const externalCopyOpening = {
    choices: externalCopyPresentation.choices,
    dialogCount: externalCopyPresentation.dialogCount,
    modalLayerCount: externalCopyPresentation.modalLayerCount,
    ownedShellCount: externalCopyPresentation.ownedShellCount,
    ariaModal: externalCopyPresentation.ariaModal,
    title: externalCopyPresentation.title,
    initialFocus: externalCopyPresentation.initialFocus,
    viewportWidth: externalCopyPresentation.viewportWidth,
    externalDialog: externalCopyPresentation.externalDialog,
    openedFromLoadingOwner: externalCopyPresentation.openedFromLoadingOwner,
    globalRoutePreserved: externalTargetSnapshot.globalTargets.length >= 1,
    decisionDialogTargetCount: externalTargetSnapshot.decisionTargets.length,
    hostWebViewBeforeDecision: externalHostWebViewBeforeDecision,
    projectVisibleBeforeDecision:
      externalProjectWindowTitleBeforeDecision !== "",
    cancelRestoredGlobalAndCleanedHost,
    emptyActivationDidNotResurrectGlobal,
    emptyActivationPreservedPendingOwner,
    nativeOwnerReplaced: firstExternalOwnerWasReplaced,
    pickerCancellationPreservedAttempt,
    realPathActivationsCompletedSerially,
    queuedActivationPreservedOwner: queuedActivationPreservedExternalOwner,
    samePendingHostCompletedHandoff,
    sourcePreserved: externalSourcePreserved,
    revisionPreserved:
      externalSavedDocument.revision === externalSourceDocument.revision,
    identityIsolated:
      externalSavedDocument.projectId !== externalSourceDocument.projectId,
    pickerOwnedByOpeningProcess: selectedExternalCopy.exactProcess === true,
  };

  const graphicsFailure = {
    cancelledCloseRearmedSingleDialog:
      cancelledCloseRearmedSingleGraphicsDialog,
    dialogOwnedByProject: graphicsDialogOwnedAndProjectBlocked,
    hostCleanedAfterTerminal: graphicsFailureTerminalCleanedHost,
    inlineFailureCount: blockedWorkspaceAfterGraphicsFailure.inlineFailureCount,
    ownerEnabledWhileOpen: graphicsNativeOwner.owner?.enabled ?? null,
    ownerVisibleWhileOpen: graphicsNativeOwner.owner?.visible ?? null,
    projectCanvasRemainedMounted:
      blockedWorkspaceAfterGraphicsFailure.canvasStillMounted,
    projectDialogTargetCount: graphicsDialogTarget ? 1 : 0,
    workspaceBusyBeforeDialogTerminal:
      blockedWorkspaceAfterGraphicsFailure.workspaceBusy,
    workspaceInertBeforeDialogTerminal:
      blockedWorkspaceAfterGraphicsFailure.workspaceInert,
    exportDisabledBeforeDialogTerminal:
      blockedWorkspaceAfterGraphicsFailure.exportDisabled,
  };

  const bootstrapCorrelations = [
    {
      globalProcessId: firstGlobal.processId,
      hostProcessId: firstHost.processId,
    },
    {
      globalProcessId: secondGlobal.processId,
      hostProcessId: crashedHost.processId,
    },
    {
      globalProcessId: recoveryGlobal.processId,
      hostProcessId: secondHost.processId,
    },
    {
      globalProcessId: originalGlobal.processId,
      hostProcessId: originalHost.processId,
    },
    {
      globalProcessId: externalCopyGlobal.processId,
      hostProcessId: externalCopyHost.processId,
    },
  ];
  await waitFor(
    "correlated journey terminals flushed",
    () => {
      const observedRecords = logRecords();
      const observedExportSpawns = observedRecords.filter(
        (record) =>
          record.event === "imaging_process_spawned" &&
          record.operation === "export",
      );
      if (observedExportSpawns.length !== 2) return false;
      try {
        assertCorrelatedJourneyTerminals(observedRecords, {
          bootstraps: bootstrapCorrelations,
          imagingAttempts: observedExportSpawns.map((record) => ({
            hostProcessId: secondHost.processId,
            imagingProcessId: Number(record.imaging_process_id),
          })),
        });
        return true;
      } catch {
        return false;
      }
    },
    timeoutMilliseconds,
  );

  const records = logRecords();
  const exportSpawns = records.filter(
    (record) =>
      record.event === "imaging_process_spawned" &&
      record.operation === "export",
  );
  if (exportSpawns.length !== 2) {
    throw new Error(
      `The productive journey expected exactly two Processador Export attempts and observed ${exportSpawns.length}`,
    );
  }
  const [successfulSpawn, missingOriginalSpawn] = exportSpawns;
  const correlations = assertCorrelatedJourneyTerminals(records, {
    bootstraps: bootstrapCorrelations,
    imagingAttempts: exportSpawns.map((record) => ({
      hostProcessId: secondHost.processId,
      imagingProcessId: Number(record.imaging_process_id),
    })),
  });
  const exportedAfterReopen = exportSpawns.every((record) =>
    assertReopenedHostExport({
      savedHostProcessId: firstHost.processId,
      reopenedHostProcessId: secondHost.processId,
      exportHostProcessId: Number(record.process_id),
    }),
  );
  if (
    new Set([
      secondGlobal.processId,
      crashedHost.processId,
      recoveryGlobal.processId,
      secondHost.processId,
      originalGlobal.processId,
      originalHost.processId,
      externalCopyGlobal.processId,
      cancelledExternalCopyHost.processId,
      queuedExternalCopyHost.processId,
      externalCopyHost.processId,
      ...exportSpawns.map((record) => Number(record.imaging_process_id)),
    ]).size !==
    10 + exportSpawns.length
  ) {
    throw new Error(
      "Global, Host, serialized external-copy attempts and both Processadores did not use distinct PIDs",
    );
  }
  if (applicationProcesses().length !== 0) {
    throw new Error("The productive journey left an application process alive");
  }

  console.log(
    JSON.stringify({
      cancelledCreationBeforeCore: true,
      cancelledExportBeforePipeline: true,
      createAuthorization: "createOnly",
      ...sheetEvidence,
      exportedDpi: 360,
      savedRevision: savedDocument.revision,
      savedDpi: savedDocument.project.document.dpi,
      schemaVersion: savedDocument.schemaVersion,
      photoFrameCount: savedFrames.length,
      persistedPhotoLinkOnly,
      reimportedExistingPhotoWithoutRevision,
      physicalAlbumStructure: {
        ...physicalAlbumStructure,
      },
      sessionRecovery: {
        schemaVersion: recoveryCheckpoint.schemaVersion,
        baseSavedRevision: recoveryCheckpoint.baseRevision.revision,
        creativeRevision: recoveryCheckpoint.creativeState.revision,
        recoveredDpi: recoveryCheckpoint.creativeState.project.document.dpi,
        promptChoices: recoveryChoices,
        presentation: {
          ...recoveryPresentation,
          recoveredOwnerUrl,
          owner: "global-opening",
          globalRoutePreserved:
            recoveryTargetSnapshot.globalTargets.length >= 1,
          recoveryDialogTargetCount:
            recoveryTargetSnapshot.recoveryTargets.length,
          hostWebViewBeforeDecision,
          projectRouteNormal,
          projectVisibleBeforeDecision: projectWindowTitleBeforeDecision !== "",
          sameOpeningWindow: recoveryPresentation.openedFromLoadingOwner,
          stableOpeningOwner:
            recoveryGlobal.processId !== secondHost.processId &&
            recoveryPresentation.externalDialog,
          queuedActivationPreservedOwner,
          singleHostDuringQueuedActivation,
        },
        opaqueProjectKey:
          path.basename(recoveryCheckpointPath) ===
            `${originalNamespace}.json` &&
          !path.basename(recoveryCheckpointPath).includes(originalProjectId),
        completedActionCheckpointed: true,
        midGesturePreservedPreviousCheckpoint,
        projectFileUnchangedThroughRecovery,
        checkpointPreservedAfterRecovery,
        recoveredUnsaved,
        recoveredHistoryEmpty,
        postRecoveryActionsCheckpointed:
          postRecoveryCheckpoint.creativeState.revision === 6,
        checkpointPreservedByCancelledSaveAs: cancelledSaveAsBeforeCore,
        checkpointFinishedBySuccessfulSaveAs: recoveryFinished,
        crashedHostProcessId: crashedHost.processId,
        recoveredHostProcessId: secondHost.processId,
        lockReleasedToDistinctHost:
          crashedHost.processId !== secondHost.processId,
      },
      externalCopyOpening,
      graphicsFailure,
      saveAs: {
        cancelledBeforeCore: cancelledSaveAsBeforeCore,
        createAuthorization: "createOnly",
        originalProjectId,
        copiedProjectId,
        savedAsRevision: savedAsDocument.revision,
        contentPreserved: savedAsContentPreserved,
        originalByteIdentical: originalByteIdenticalAfterSaveAs,
        historyPreserved: historyPreservedAfterSaveAs,
        originalHistoryEmpty: simultaneousOriginalHistoryEmpty,
        simultaneouslyOpen: simultaneousHostsOpen,
        isolatedIndependentSaves,
        originalSavedRevision: independentlySavedOriginalDocument.revision,
        originalSavedDpi:
          independentlySavedOriginalDocument.project.document.dpi,
        copySavedRevision: independentlySavedCopyDocument.revision,
        copySavedDpi: independentlySavedCopyDocument.project.document.dpi,
        previousRecoveryFinished: recoveryFinished,
        cacheStagedEmpty: Boolean(emptyCacheStage),
        localAuthorityTransitioned,
        webviewNamespaceTransitioned: namespaceTransitioned,
        nativeTitleUpdated,
        observedNativeTitle: observedSavedAsTitle,
        replacementWebviewReady: rebuiltWebviewReady,
        globalInspectorPreferencePreserved,
        projectLocalSelectionReset,
      },
      originalUnchanged: readFileSync(photoPath).equals(originalPhoto),
      missingOriginalBlocked,
      missingOriginalActionable,
      residentCanvasPreviewBeforeMissingOriginal,
      ...emptyCacheEvidence,
      cacheCouldNotProduceFalseSuccess,
      jpeg: {
        ...dimensions,
        byteCount: exported.length,
        sha256: createHash("sha256").update(exported).digest("hex"),
      },
      processIds: {
        firstGlobal: firstGlobal.processId,
        firstHost: firstHost.processId,
        global: secondGlobal.processId,
        crashedHost: crashedHost.processId,
        recoveryGlobal: recoveryGlobal.processId,
        host: secondHost.processId,
        simultaneousOriginalGlobal: originalGlobal.processId,
        simultaneousOriginalHost: originalHost.processId,
        externalCopyGlobal: externalCopyGlobal.processId,
        cancelledExternalCopyHost: cancelledExternalCopyHost.processId,
        queuedExternalCopyHost: queuedExternalCopyHost.processId,
        externalCopyHost: externalCopyHost.processId,
        imaging: Number(successfulSpawn.imaging_process_id),
        missingOriginalImaging: Number(missingOriginalSpawn.imaging_process_id),
      },
      correlations,
      exportedAfterReopen,
      reopenedInIndependentHost: secondHost.processId !== firstHost.processId,
      reopenedHistoryEmpty: true,
      screenshotPath,
      canvasPhotoSample,
      sourcePathExposedToWebView,
      terminalCounts: {
        globalHandoffs: eventCount(
          logText(),
          "global_exited_after_project_handoff",
        ),
        hostReady: eventCount(logText(), "host_ready"),
        imagingStopped: records.filter(
          (record) =>
            record.event === "imaging_process_stopped" &&
            exportSpawns.some(
              (spawned) =>
                Number(record.process_id) ===
                Number(spawned.imaging_process_id),
            ),
        ).length,
      },
    }),
  );
} catch (error) {
  try {
    const observations = {};
    for (const [label, driver] of Object.entries({
      globalDriver,
      hostDriver,
      originalDriver,
      recoveryDialogDriver,
      secondGlobalDriver,
      externalCopyDialogDriver,
      externalCopyHostDriver,
    })) {
      if (!driver) continue;
      try {
        observations[label] = await readProjectInteractionState(driver);
        const screenshot = await driver.request(
          "GET",
          `/session/${driver.sessionId}/screenshot`,
        );
        writeFileSync(
          path.join(scratch, `failure-${label}.png`),
          Buffer.from(screenshot, "base64"),
        );
      } catch (observationError) {
        observations[label] = {
          ...observations[label],
          observationError: String(observationError),
        };
      }
    }
    writeFileSync(
      path.join(scratch, "failure.json"),
      JSON.stringify({
        error: String(error),
        stack: error.stack,
        observations,
        processes: applicationProcesses(),
        browsers: webViewProcessesForDataDirectory(scratch),
        recentEvents: logRecords().slice(-20),
      }, null, 2),
    );
  } catch (diagnosticError) {
    console.error(`Productive journey diagnostics failed: ${diagnosticError}`);
  }
  throw error;
} finally {
  let driverCleanupFailure;
  for (const driver of [
    globalDriver,
    hostDriver,
    originalDriver,
    recoveryDialogDriver,
    secondGlobalDriver,
    externalCopyDialogDriver,
    externalCopyHostDriver,
  ]) {
    if (driver) {
      try {
        await disposeConfirmedWebDriver(driver);
      } catch (error) {
        driverCleanupFailure ??= error;
      }
    }
  }
  for (const instance of applicationProcesses()) {
    terminateProcessInstance(instance);
  }
  if (existsSync(externalCopyPath)) {
    spawnSync("attrib.exe", ["-R", externalCopyPath], {
      cwd: workspace,
      windowsHide: true,
      encoding: "utf8",
    });
  }
  if (driverCleanupFailure) {
    throw driverCleanupFailure;
  }
}
