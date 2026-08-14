import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import net from "node:net";
import path from "node:path";

import {
  aliveProcessInstances,
  assertNoPreexistingProcessInstances,
  processInstancesByExecutable,
  sameProcessInstance,
  terminateProcessInstance,
  waitForProcessInstance,
} from "./DevLifecycleProcessInstances.mjs";
import {
  assertCausalProjectHandoff,
  assertCorrelatedJourneyTerminals,
  eventCount,
} from "./ProductiveJourneyObservations.mjs";

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
const projectPath = path.join(scratch, "Jornada produtiva.myalbuns");
const exportPath = path.join(scratch, "Jornada produtiva_002.jpg");
const screenshotPath = path.join(scratch, "project-canvas.png");
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
]) {
  if (!existsSync(candidate)) {
    throw new Error(`${label} was not found`);
  }
}
mkdirSync(scratch, { recursive: true });
for (const knownFolder of ["Roaming", "Local", "Temporary"]) {
  mkdirSync(path.join(processDataRoot, knownFolder), { recursive: true });
}
if (existsSync(projectPath) || existsSync(exportPath)) {
  throw new Error("The productive journey requires absent CreateOnly targets");
}

const delay = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

async function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close((error) => {
        if (error) reject(error);
        else resolve(address.port);
      });
    });
  });
}

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

function webdriverClient(baseUrl) {
  return async (method, endpoint, body, timeout = 10_000) => {
    const response = await fetch(`${baseUrl}${endpoint}`, {
      method,
      headers:
        body === undefined ? undefined : { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: AbortSignal.timeout(timeout),
    });
    const text = await response.text();
    const payload = text ? JSON.parse(text) : { value: null };
    if (!response.ok || payload.value?.error) {
      throw new Error(
        `${method} ${endpoint} failed (${response.status}): ${JSON.stringify(payload)}`,
      );
    }
    return payload.value;
  };
}

async function startAttachedWebDriver(debugPort, label) {
  await waitForHttp(
    `http://127.0.0.1:${debugPort}/json/version`,
    `${label} DevTools endpoint`,
  );
  const driverPort = await freePort();
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
  const request = webdriverClient(baseUrl);
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

const globalDebugPort = await freePort();
const hostDebugPort = await freePort();
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
let secondHostDriver;
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
  await click(
    globalDriver,
    "xpath",
    "//button[normalize-space()='Próximo']",
    "creation next action",
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
  const sourcePathExposedToWebView = [
    projectPath,
    exportPath,
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
  await click(
    hostDriver,
    "xpath",
    "//button[.//span[normalize-space()='Design do Álbum']]",
    "Album design inspector",
  );

  const dpiInput = await replaceInput(
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
  await click(
    hostDriver,
    "css selector",
    "button[aria-label='Salvar']",
    "Save action",
  );
  await waitForLogEvent("project_save_completed", 1, "Save terminal");
  const savedProject = readFileSync(projectPath);
  const savedDocument = JSON.parse(savedProject.toString("utf8"));
  if (savedDocument.revision !== 1 || savedDocument.project.document.dpi !== 300) {
    throw new Error("The productive save did not persist revision 1 at 300 DPI");
  }

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
  await waitForLogEvent("project_intent_applied", 2, "unsaved DPI application");

  const exportStartedBeforeCancel = recordsFor("export_started").length;
  const processorBeforeCancel = recordsFor("imaging_process_spawned").length;
  await clickWhenEnabled(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Exportar Lâmina']",
    "Export action",
  );
  const cancelledExport = driveNativeDialog(
    firstHost,
    "cancel",
    "Exportar Lâmina como JPEG",
  );
  if (
    cancelledExport.action !== "cancel" ||
    existsSync(exportPath) ||
    recordsFor("export_started").length !== exportStartedBeforeCancel ||
    recordsFor("imaging_process_spawned").length !== processorBeforeCancel
  ) {
    throw new Error("Cancelled Export crossed the ExportPipeline boundary");
  }

  await clickUntilLogEvent(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Exportar Lâmina']",
    "native_save_dialog_opening",
    "Export retry action",
  );
  const selectedExport = driveNativeDialog(
    firstHost,
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
  const dimensions = jpegDimensions(exported);
  if (dimensions.width !== 720 || dimensions.height !== 360) {
    throw new Error(
      `The non-initial sheet was not exported at unsaved 360 DPI: ${JSON.stringify(dimensions)}`,
    );
  }
  if (!readFileSync(projectPath).equals(savedProject)) {
    throw new Error("Export mutated the saved Project document");
  }
  if ((await elementAttribute(hostDriver, dpiInput, "value")) !== "360") {
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
  );
  await click(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Descartar e fechar']",
    "Discard pending DPI action",
  ).catch(() => undefined);
  await hostDriver.dispose();
  hostDriver = undefined;
  await waitForExit(firstHost, "first Project Host close");

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

  secondHostDriver = await startAttachedWebDriver(
    hostDebugPort,
    "reopened Project Host",
  );
  const reopenedDpi = await findElement(
    secondHostDriver,
    "css selector",
    ".document-dpi-control input",
    "reopened DPI",
  );
  if ((await elementAttribute(secondHostDriver, reopenedDpi, "value")) !== "300") {
    throw new Error("The reopened Project did not restore the saved DPI");
  }
  for (const label of ["Desfazer", "Refazer"]) {
    const button = await findElement(
      secondHostDriver,
      "css selector",
      `button[aria-label='${label}']`,
      `${label} after reopen`,
    );
    const enabled = await secondHostDriver.request(
      "GET",
      `/session/${secondHostDriver.sessionId}/element/${encodeURIComponent(button)}/enabled`,
    );
    if (enabled) {
      throw new Error(`The reopened Project retained ${label} history`);
    }
  }
  await click(
    secondHostDriver,
    "xpath",
    "//button[normalize-space()='Arquivo']",
    "reopened File menu",
  );
  await click(
    secondHostDriver,
    "xpath",
    "//button[normalize-space()='Fechar Projeto']",
    "reopened Project close",
  ).catch(() => undefined);
  await secondHostDriver.dispose();
  secondHostDriver = undefined;
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
      Number(record.process_id) === firstHost.processId &&
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
        hostProcessId: firstHost.processId,
        imagingProcessId: Number(spawn.imaging_process_id),
      },
    ],
  });
  if (
    new Set([
      firstGlobal.processId,
      firstHost.processId,
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
      exportedSheetNumber: 2,
      exportedDpi: 360,
      savedRevision: savedDocument.revision,
      savedDpi: savedDocument.project.document.dpi,
      jpeg: {
        ...dimensions,
        byteCount: exported.length,
        sha256: createHash("sha256").update(exported).digest("hex"),
      },
      processIds: {
        global: firstGlobal.processId,
        host: firstHost.processId,
        imaging: Number(spawn.imaging_process_id),
      },
      correlations,
      reopenedInIndependentHost: secondHost.processId !== firstHost.processId,
      reopenedHistoryEmpty: true,
      screenshotPath,
      sourcePathExposedToWebView,
      terminalCounts: {
        globalHandoffs: eventCount(logText(), "global_exited_after_project_handoff"),
        hostReady: eventCount(logText(), "host_ready"),
        imagingStopped: eventCount(logText(), "imaging_process_stopped"),
      },
    }),
  );
} finally {
  for (const driver of [
    globalDriver,
    hostDriver,
    secondGlobalDriver,
    secondHostDriver,
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
