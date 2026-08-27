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
  disposeConfirmedWebDriver,
  findFreeTcpPort,
  findFreeTcpPortInRange,
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
const saveAsPath = path.join(scratch, "Jornada produtiva - Cópia.myalbuns");
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
  throw new Error(`${label} did not become ready: ${lastError ?? "unknown error"}`, {
    cause: lastError,
  });
}

async function startAttachedWebDriver(
  debugPort,
  label,
  projectDialogDebugPort,
) {
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
  const rawRequest = createWebDriverClient(baseUrl);
  const request = async (method, endpoint, body, timeout) => {
    try {
      return await rawRequest(method, endpoint, body, timeout);
    } catch (error) {
      throw new Error(
        `${label} WebDriver ${method} ${endpoint} failed; driverExitCode=${child.exitCode}; output=${output.slice(-1_000)}`,
        { cause: error },
      );
    }
  };
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
  }, webDriverSessionTimeoutMilliseconds);
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
    projectDialogDebugPort,
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
  if ((await elementAttribute(driver, menuTrigger, "aria-expanded")) !== "true") {
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
  throw new Error(`${label} produced no ${event} observation`, {
    cause: lastError,
  });
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
      `//*[@role='dialog' and @aria-label='${dialogLabel}']//button[normalize-space()='${actionLabel}']`,
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
  return findElement(
    driver,
    "css selector",
    "input[aria-label='DPI']",
    label,
  );
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
    await driver.request("DELETE", `${endpoint}/actions`).catch(() => undefined);
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

function nativeWindowTitle(instance) {
  const encoded = powershellJson(
    String.raw`
$observed = Get-CimInstance Win32_Process -Filter "ProcessId = $env:MYALBUNS_GATE_WINDOW_PID" -ErrorAction Stop
if ($null -eq $observed -or $observed.CreationDate.ToUniversalTime().ToString('O') -cne $env:MYALBUNS_GATE_WINDOW_CREATED) {
    throw 'The native window no longer belongs to the expected process instance.'
}
$process = Get-Process -Id ([int]$env:MYALBUNS_GATE_WINDOW_PID) -ErrorAction Stop
[void]$process.Handle
if ($process.HasExited) {
    throw 'The native window process exited before its title was observed.'
}
$titleBytes = [System.Text.Encoding]::UTF8.GetBytes([string]$process.MainWindowTitle)
$titleBase64 = [System.Convert]::ToBase64String($titleBytes)
[Console]::Out.Write((ConvertTo-Json -InputObject $titleBase64 -Compress))
`,
    {
      MYALBUNS_GATE_WINDOW_PID: String(instance.processId),
      MYALBUNS_GATE_WINDOW_CREATED: instance.creationTimeUtc,
    },
  );
  return Buffer.from(encoded, "base64").toString("utf8");
}

function projectDataNamespace(projectId) {
  return `project-${createHash("sha256").update(projectId).digest("hex")}`;
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
  MYALBUNS_DEV_ALTERNATE_HOST_WEBVIEW_DEBUG_PORT: String(
    reopenedHostDebugPort,
  ),
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
let firstHost;
let secondGlobal;
let secondHost;
let recoveryGlobal;
let crashedHost;
let originalGlobal;
let originalHost;
let originalReplacementGlobal;
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
    throw new Error("The native existing JPEG Photo selection was not confirmed");
  }
  await waitForLogEvent(
    "photo_import_existing_selected",
    1,
    "existing Photo selection terminal",
  );
  const existingSelection = recordsFor(
    "photo_import_existing_selected",
  ).at(-1);
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
  ].some((candidate) => sourceContainsNativePath(importedPageSource, candidate));

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
  if (webViewProcessesForDataDirectory(projectWebViewDataDirectory).length === 0) {
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
  ].some((candidate) => sourceContainsNativePath(reopenedPageSource, candidate));
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
  await applyAlbumInformation(hostDriver, "Apply unsaved Album information action");
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
  await waitForExit(recoveryGlobal, "recovery Global handoff");
  await waitForHostUiReady(secondHost, "recovery choice UI ready");
  hostDriver = await startAttachedWebDriver(
    recoveryHostDebugPort,
    "recovery Project Host",
    recoveryProjectDialogDebugPort,
  );
  const recoveryChoices = await hostDriver.request(
    "POST",
    `/session/${hostDriver.sessionId}/execute/sync`,
    {
      script: `return Array.from(document.querySelectorAll('.recovery-actions button')).map((button) => button.textContent.trim());`,
      args: [],
    },
  );
  if (
    JSON.stringify(recoveryChoices) !==
    JSON.stringify([
      "Reabrir e recuperar",
      "Abrir última versão salva",
      "Agora não",
    ])
  ) {
    throw new Error(
      `The recovery prompt exposed unexpected choices: ${JSON.stringify(recoveryChoices)}`,
    );
  }
  await click(
    hostDriver,
    "xpath",
    "//button[normalize-space()='Reabrir e recuperar']",
    "Reopen and recover choice",
  );
  await findElement(
    hostDriver,
    "css selector",
    ".app-shell",
    "recovered Project UI",
  );
  const recoveredDpi = await findAlbumInformationDpi(hostDriver, "recovered DPI");
  if ((await elementAttribute(hostDriver, recoveredDpi, "value")) !== "360") {
    throw new Error("The recovered Project did not restore the checkpoint state");
  }
  let recoveredHistoryEmpty = true;
  for (const label of ["Desfazer", "Refazer"]) {
    if (await applicationMenuCommandEnabled(
      hostDriver,
      "Editar",
      label,
      `${label} after recovery`,
    )) {
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
  const projectFileUnchangedThroughRecovery =
    readFileSync(projectPath).equals(projectBytesBeforeCrash);
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
  const preSaveAsRecoveryCheckpointBytes = readFileSync(
    recoveryCheckpointPath,
  );
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
  const emptyCacheStage = recordsFor(
    "project_save_as_cache_staged_empty",
  ).find(
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
    throw new Error("The Save As Host did not make its replacement WebView ready");
  }

  hostDriver = await startAttachedWebDriver(
    saveAsHostDebugPort,
    "Save As Project Host",
    recoveryProjectDialogDebugPort,
  );
  await findElement(hostDriver, "css selector", ".app-shell", "Save As Project UI");
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
        ".sheet-grid > button.active > span",
        "fresh Save As active sheet number",
      ),
    ),
  );
  const projectLocalSelectionReset = freshActiveSheetNumber === 1;
  if (!projectLocalSelectionReset) {
    throw new Error("The Save As WebView inherited the previous local sheet selection");
  }
  await click(
    hostDriver,
    "css selector",
    ".sheet-grid > button:nth-child(2)",
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
            ".sheet-grid > button.active > span",
            "Save As selected sheet number",
          ),
        ),
      ) === activeSheetNumber,
    timeoutMilliseconds,
  );
  const copiedDpi = await findAlbumInformationDpi(hostDriver, "Save As DPI");
  if ((await elementAttribute(hostDriver, copiedDpi, "value")) !== "360") {
    throw new Error("The rebuilt Save As WebView did not adopt the copied projection");
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
  await waitForExit(
    originalGlobal,
    "simultaneous original Global handoff",
  );
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
    (await elementAttribute(originalDriver, simultaneousOriginalDpi, "value")) !==
    "300"
  ) {
    throw new Error("The simultaneous original did not retain its saved content");
  }
  for (const label of ["Desfazer", "Refazer"]) {
    if (await applicationMenuCommandEnabled(
      originalDriver,
      "Editar",
      label,
      `${label} in simultaneous original`,
    )) {
      throw new Error(`The simultaneous original retained ${label} history`);
    }
  }
  const simultaneousOriginalHistoryEmpty = true;
  const simultaneousHostsOpen =
    applicationProcesses().filter(isHost).length === 2 &&
    aliveProcessInstances([secondHost, originalHost]).length === 2;
  if (!simultaneousHostsOpen) {
    throw new Error("The original and Save As copy were not open simultaneously");
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
    throw new Error("Saving the simultaneous original crossed into the Save As copy");
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
    throw new Error("Saving the Save As copy crossed into the original Project");
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
  if (
    !readFileSync(saveAsPath).equals(independentlySavedCopy) ||
    !readFileSync(projectPath).equals(independentlySavedOriginal)
  ) {
    throw new Error("Export mutated either independently saved Project document");
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
      throw new Error("The missing-Original Export destination was not confirmed");
    }
    await waitForLogEvent("export_failed", 1, "missing-Original failure");
    await waitFor(
      "missing-Original Processador spawn",
      () =>
        exportProcessorAttempts().length ===
        missingOriginalProcessorCount + 1,
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
          "//*[@role='dialog' and @aria-label='Exportação não concluída']",
          "actionable missing-Original message",
        );
        const text = await elementText(dialogDriver, failureDialog);
        await clickWhenEnabled(
          dialogDriver,
          "xpath",
          "//*[@role='dialog' and @aria-label='Exportação não concluída']//button[normalize-space()='Fechar']",
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
    throw new Error("The restored proof Original differs from the imported bytes");
  }

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
      ...exportSpawns.map((record) => Number(record.imaging_process_id)),
    ]).size !== 6 + exportSpawns.length
  ) {
    throw new Error(
      "Global, Host and both Processadores did not use distinct PIDs",
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
      sessionRecovery: {
        schemaVersion: recoveryCheckpoint.schemaVersion,
        baseSavedRevision: recoveryCheckpoint.baseRevision.revision,
        creativeRevision: recoveryCheckpoint.creativeState.revision,
        recoveredDpi: recoveryCheckpoint.creativeState.project.document.dpi,
        promptChoices: recoveryChoices,
        opaqueProjectKey:
          path.basename(recoveryCheckpointPath) === `${originalNamespace}.json` &&
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
        globalHandoffs: eventCount(logText(), "global_exited_after_project_handoff"),
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
} finally {
  let driverCleanupFailure;
  for (const driver of [
    globalDriver,
    hostDriver,
    originalDriver,
    secondGlobalDriver,
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
  if (driverCleanupFailure) {
    throw driverCleanupFailure;
  }
}
