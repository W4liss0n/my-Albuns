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
  processInstancesByExecutable,
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
  assertReopenedHostExport,
  eventCount,
} from "./ProductiveJourneyObservations.mjs";
import {
  createWebDriverClient,
  findFreeTcpPort,
} from "./GateWebDriver.mjs";

const [workspaceArgument, scratchArgument, applicationArgument, driverArgument] =
  process.argv.slice(2);
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
const driverTerminationTimeoutMilliseconds = 30_000;

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
  existsSync(exportPath) ||
  existsSync(missingOriginalExportPath) ||
  existsSync(photoPath)
) {
  throw new Error("The productive journey requires absent CreateOnly targets");
}
copyFileSync(photoFixturePath, photoPath);
const originalPhoto = readFileSync(photoPath);

const delay = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

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
  throw lastError ?? new Error(`${label} did not become ready`);
}

async function startAttachedWebDriver(debugPort, label) {
  await waitForHttp(
    `http://127.0.0.1:${debugPort}/json/version`,
    `${label} DevTools endpoint`,
  );
  const driverPort = await findFreeTcpPort();
  const child = spawn(
    nativeDriverPath,
    [`--port=${driverPort}`, "--host=127.0.0.1"],
    {
      cwd: workspace,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  let output = "";
  child.stdout.on("data", (chunk) => {
    output += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    output += chunk.toString();
  });
  const instance = await waitForProcessInstance(child.pid, `${label} WebDriver`);
  const baseUrl = `http://127.0.0.1:${driverPort}`;
  await waitForHttp(`${baseUrl}/status`, `${label} WebDriver`);
  const request = createWebDriverClient(baseUrl);
  const session = await request("POST", "/session", {
    capabilities: {
      alwaysMatch: {
        browserName: "webview2",
        pageLoadStrategy: "none",
        "ms:edgeChromium": true,
        "ms:edgeOptions": {
          debuggerAddress: `127.0.0.1:${debugPort}`,
        },
      },
    },
  });
  if (!session.sessionId) {
    throw new Error(`${label} WebDriver returned no session id`);
  }
  const sessionId = session.sessionId;
  await request("POST", `/session/${sessionId}/timeouts`, {
    implicit: 250,
    pageLoad: 5_000,
    script: 5_000,
  });
  return {
    request,
    sessionId,
    async dispose() {
      try {
        await request("DELETE", `/session/${sessionId}`);
      } catch {
        // The WebView can close before the attach-only session is deleted.
      }
      terminateProcessInstance(instance);
      const deadline = Date.now() + driverTerminationTimeoutMilliseconds;
      while (
        aliveProcessInstances([instance]).length !== 0 &&
        Date.now() < deadline
      ) {
        await delay(25);
      }
      if (aliveProcessInstances([instance]).length !== 0) {
        throw new Error(`${label} WebDriver did not terminate`);
      }
      await waitFor(
        `${label} WebDriver process handle terminal`,
        () => child.exitCode !== null || child.signalCode !== null,
        driverTerminationTimeoutMilliseconds,
      );
      return output;
    },
  };
}

async function findElement(
  driver,
  using,
  value,
  label,
  timeout = 30_000,
) {
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
    await driver.request("DELETE", `${endpoint}/actions`).catch(() => undefined);
  }
  return elementId;
}

async function sendEscape(driver) {
  const body = await findElement(driver, "css selector", "body", "Project body");
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
      args: [
        { "element-6066-11e4-a52e-4f735466cecf": elementId },
        nextValue,
      ],
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

async function waitFor(label, predicate, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  let observation;
  while (Date.now() < deadline) {
    observation = await predicate();
    if (observation) return observation;
    await delay(50);
  }
  throw new Error(`${label} was not observed: ${JSON.stringify(observation)}`);
}

function applicationProcesses() {
  return processInstancesByExecutable(
    applicationPath,
    "myalbuns-desktop.exe",
  );
}

function isHost(instance) {
  return instance.commandLine.includes("--myalbuns-project-host");
}

async function waitForNewApplication(predicate, known, label) {
  return waitFor(
    label,
    () =>
      applicationProcesses().find(
        (instance) =>
          predicate(instance) &&
          !known.some((candidate) => sameProcessInstance(instance, candidate)),
      ),
    timeoutMilliseconds,
  );
}

async function waitForExit(instance, label) {
  await waitFor(
    label,
    () => aliveProcessInstances([instance]).length === 0,
    timeoutMilliseconds,
  );
}

function driveNativeDialog(instance, action, title, destination) {
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
  if (destination) {
    arguments_.push("-DestinationPath", destination);
  }
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

function logRecords() {
  const directory = path.join(processDataRoot, "Local", "MyAlbuns2", "Logs");
  if (!existsSync(directory)) return [];
  return readdirSync(directory)
    .filter((name) => name.endsWith(".jsonl"))
    .flatMap((name) =>
      readFileSync(path.join(directory, name), "utf8")
        .split(/\r?\n/)
        .filter(Boolean)
        .flatMap((line) => {
          try {
            return [JSON.parse(line)];
          } catch {
            return [];
          }
        }),
    )
    .sort((left, right) => String(left.timestamp).localeCompare(String(right.timestamp)));
}

function logText() {
  return logRecords().map((record) => JSON.stringify(record)).join("\n");
}

function recordsFor(event) {
  return logRecords().filter((record) => record.event === event);
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

async function waitForLogEvent(event, count, label) {
  return waitFor(
    label,
    () => recordsFor(event).length >= count,
    timeoutMilliseconds,
  );
}

const globalDebugPort = await findFreeTcpPort();
const hostDebugPort = await findFreeTcpPort();
const applicationEnvironment = {
  ...process.env,
  MYALBUNS_PROCESS_GATE_DATA_ROOT: processDataRoot,
  MYALBUNS_DEV_GLOBAL_WEBVIEW_DEBUG_PORT: String(globalDebugPort),
  MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT: String(hostDebugPort),
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
let secondGlobalDriver;
let firstHost;
let secondGlobal;
let secondHost;
let finalGlobal;

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
    "xpath",
    "//button[normalize-space()='Novo Projeto']",
    "New Project action",
  );
  for (const [label, text] of [
    ["Largura da Lâmina", "50.8"],
    ["Altura da Lâmina", "25.4"],
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
    "xpath",
    "//button[normalize-space()='Próximo']",
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
    "xpath",
    "//button[normalize-space()='Criar']",
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
    "xpath",
    "//button[normalize-space()='Criar']",
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
  const globalDriverDisposal = globalDriver.dispose();
  globalDriver = undefined;
  hostDriver = await startAttachedWebDriver(hostDebugPort, "first Project Host");
  await globalDriverDisposal;
  await waitForExit(firstGlobal, "first Global exit after handoff");
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
    ".sheet-grid > button:nth-child(2)",
    "second sheet",
  );
  const activeSheetNumber = Number(
    await elementText(
      hostDriver,
      await findElement(
        hostDriver,
        "css selector",
        ".sheet-grid > button.active > span",
        "active sheet number",
      ),
    ),
  );
  if (!Number.isInteger(activeSheetNumber)) {
    throw new Error("The productive UI exposed no active sheet number");
  }
  await click(
    hostDriver,
    "xpath",
    "//button[.//span[normalize-space()='Design do Álbum']]",
    "Album design inspector",
  );

  await replaceInput(
    hostDriver,
    "css selector",
    ".document-dpi-control input",
    "300",
    "DPI input",
  );
  await click(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Aplicar DPI']",
    "Apply DPI action",
  );
  await waitForLogEvent("project_intent_applied", 1, "DPI application");
  await click(
    hostDriver,
    "css selector",
    "button[aria-label='Desfazer']",
    "Undo action",
  );
  await waitForLogEvent("project_undo_completed", 1, "Undo terminal");
  await click(
    hostDriver,
    "css selector",
    "button[aria-label='Refazer']",
    "Redo action",
  );
  await waitForLogEvent("project_redo_completed", 1, "Redo terminal");

  await clickWhenEnabled(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Importar JPEG…']",
    "Import Photo action",
  );
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
  await doubleClick(
    hostDriver,
    "css selector",
    ".media-card[data-media-id]",
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
  ].some((candidate) => sourceContainsNativePath(importedPageSource, candidate));

  await click(
    hostDriver,
    "css selector",
    "button[aria-label='Salvar']",
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

  await click(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Arquivo']",
    "saved File menu",
  );
  await click(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Fechar Projeto']",
    "saved Project close",
  ).catch(() => undefined);
  await hostDriver.dispose().catch(() => undefined);
  hostDriver = undefined;
  await waitForExit(firstHost, "first Project Host close after Save");
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
  await waitForLogEvent("project_ui_ready", 2, "reopened Project UI ready");
  await secondGlobalDriver.dispose();
  secondGlobalDriver = undefined;

  hostDriver = await startAttachedWebDriver(
    hostDebugPort,
    "reopened Project Host",
  );
  const reopenedDpi = await findElement(
    hostDriver,
    "css selector",
    ".document-dpi-control input",
    "reopened DPI",
  );
  if ((await elementAttribute(hostDriver, reopenedDpi, "value")) !== "300") {
    throw new Error("The reopened Project did not restore the saved DPI");
  }
  await click(
    hostDriver,
    "css selector",
    ".sheet-grid > button:nth-child(2)",
    "reopened second sheet",
  );
  const reopenedActiveSheetNumber = Number(
    await elementText(
      hostDriver,
      await findElement(
        hostDriver,
        "css selector",
        ".sheet-grid > button.active > span",
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
    ".media-card[data-media-id]",
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
  ].some((candidate) => sourceContainsNativePath(reopenedPageSource, candidate));
  for (const label of ["Desfazer", "Refazer"]) {
    const button = await findElement(
      hostDriver,
      "css selector",
      `button[aria-label='${label}']`,
      `${label} after reopen`,
    );
    const enabled = await hostDriver.request(
      "GET",
      `/session/${hostDriver.sessionId}/element/${encodeURIComponent(button)}/enabled`,
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

  await replaceInput(
    hostDriver,
    "css selector",
    ".document-dpi-control input",
    "360",
    "unsaved DPI input",
  );
  await click(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Aplicar DPI']",
    "Apply unsaved DPI action",
  );
  await waitForLogEvent("project_intent_applied", 3, "unsaved DPI application");

  const exportStartedBeforeCancel = recordsFor("export_started").length;
  const processorBeforeCancel = exportProcessorAttempts().length;
  await clickWhenEnabled(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Exportar Lâmina']",
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

  await clickUntilLogEvent(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Exportar Lâmina']",
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
  await waitFor("exported JPEG", () => existsSync(exportPath), timeoutMilliseconds);
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
  if (!readFileSync(projectPath).equals(savedProject)) {
    throw new Error("Export mutated the saved Project document");
  }
  const liveDpiInput = await findElement(
    hostDriver,
    "css selector",
    ".document-dpi-control input",
    "live DPI input after Export",
  );
  if ((await elementAttribute(hostDriver, liveDpiInput, "value")) !== "360") {
    throw new Error("Export changed the pending DPI in the live Project");
  }
  const undoButton = await findElement(
    hostDriver,
    "css selector",
    "button[aria-label='Desfazer']",
    "Undo action after Export",
  );
  if (!(await hostDriver.request(
    "GET",
    `/session/${hostDriver.sessionId}/element/${encodeURIComponent(undoButton)}/enabled`,
  ))) {
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
          x: bounds.width / 2,
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
  unlinkSync(photoPath);
  try {
    await clickUntilLogEvent(
      hostDriver,
      "xpath",
      "//button[normalize-space()='Exportar Lâmina']",
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
      throw new Error("The missing-Original Export destination was not confirmed");
    }
    await waitForLogEvent("export_failed", 1, "missing-Original failure");
    const failureDialog = await findElement(
      hostDriver,
      "xpath",
      "//*[@role='dialog' and @aria-label='Exportação não concluída']",
      "actionable missing-Original message",
    );
    const failureText = await elementText(hostDriver, failureDialog);
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
    await click(
      hostDriver,
      "xpath",
      "//*[@role='dialog' and @aria-label='Exportação não concluída']//button[normalize-space()='Fechar']",
      "close missing-Original feedback",
    );
  } finally {
    copyFileSync(photoFixturePath, photoPath);
  }
  if (!readFileSync(photoPath).equals(originalPhoto)) {
    throw new Error("The restored proof Original differs from the imported bytes");
  }

  await click(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Arquivo']",
    "File menu",
  );
  await click(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Fechar Projeto']",
    "Close Project action",
  ).catch(() => undefined);
  await click(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Descartar e fechar']",
    "Discard pending DPI action",
  ).catch(() => undefined);
  await hostDriver.dispose();
  hostDriver = undefined;
  await waitForExit(secondHost, "reopened Project Host close");

  finalGlobal = await waitForNewApplication(
    (instance) => !isHost(instance),
    [firstGlobal, secondGlobal],
    "final Global",
  );
  terminateProcessInstance(finalGlobal);
  await waitForExit(finalGlobal, "final Global cleanup");

  const records = logRecords();
  const spawn = records.find(
    (record) =>
      record.event === "imaging_process_spawned" &&
      Number(record.process_id) === secondHost.processId &&
      record.operation === "export",
  );
  if (!spawn) {
    throw new Error("The productive Export exposed no Processador correlation");
  }
  const correlations = assertCorrelatedJourneyTerminals(records, {
    bootstraps: [
      {
        globalProcessId: firstGlobal.processId,
        hostProcessId: firstHost.processId,
      },
      {
        globalProcessId: secondGlobal.processId,
        hostProcessId: secondHost.processId,
      },
    ],
    imagingAttempts: [
      {
        hostProcessId: secondHost.processId,
        imagingProcessId: Number(spawn.imaging_process_id),
      },
    ],
  });
  const exportedAfterReopen = assertReopenedHostExport({
    savedHostProcessId: firstHost.processId,
    reopenedHostProcessId: secondHost.processId,
    exportHostProcessId: Number(spawn.process_id),
  });
  if (
    new Set([
      secondGlobal.processId,
      secondHost.processId,
      Number(spawn.imaging_process_id),
    ]).size !== 3
  ) {
    throw new Error("Global, Host and Processador did not use distinct PIDs");
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
        host: secondHost.processId,
        imaging: Number(spawn.imaging_process_id),
      },
      correlations,
      exportedAfterReopen,
      reopenedInIndependentHost: secondHost.processId !== firstHost.processId,
      reopenedHistoryEmpty: true,
      screenshotPath,
      canvasPhotoSample,
      sourcePathExposedToWebView,
      terminalCounts: {
        globalHandoffs: eventCount(logText(), "global_exited_after_project_handoff"),
        hostReady: eventCount(logText(), "host_ready"),
        imagingStopped: records.filter(
          (record) =>
            record.event === "imaging_process_stopped" &&
            Number(record.process_id) === Number(spawn.imaging_process_id),
        ).length,
      },
    }),
  );
} finally {
  for (const driver of [
    globalDriver,
    hostDriver,
    secondGlobalDriver,
  ]) {
    if (driver) {
      try {
        await driver.dispose();
      } catch {
        // Exact application cleanup below remains fail-closed in the wrapper.
      }
    }
  }
  for (const instance of applicationProcesses()) {
    terminateProcessInstance(instance);
  }
}
