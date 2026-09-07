import { spawn, execFileSync } from "node:child_process";
import {
  closeSync,
  existsSync,
  mkdirSync,
  openSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import net from "node:net";
import path from "node:path";

import {
  aliveProcessInstances,
  captureListeningProcessInstance,
  mergeProcessInstances,
  processForestInstances,
  terminateProcessInstance,
  waitForProcessInstance,
} from "./DevLifecycleProcessInstances.mjs";
import {
  finalizeUiAcceptanceSourceEvidence,
  renderUiAcceptanceReport,
  validateUiAcceptanceManifest,
  webdriverElementId,
} from "./UiAcceptance.mjs";
import {
  assertBrowserZoomUnchanged,
  captureBrowserZoomState,
  captureUiAcceptanceScreenshot,
  neutralizeUiAcceptancePointer,
  performUiAcceptanceAction,
  requiresBrowserZoomInvariant,
} from "./UiAcceptanceRunner.mjs";

const [workspaceArgument, outputArgument, edgeArgument, driverArgument] =
  process.argv.slice(2);
if (!workspaceArgument || !outputArgument || !edgeArgument || !driverArgument) {
  throw new Error(
    "Usage: Run-UiAcceptance.mjs <workspace> <output-directory> <edge> <edge-driver>",
  );
}

const workspace = path.resolve(workspaceArgument);
const outputDirectory = path.resolve(outputArgument);
const edgeExecutable = path.resolve(edgeArgument);
const driverExecutable = path.resolve(driverArgument);
const manifestPath = path.join(
  workspace,
  "src",
  "test",
  "uiAcceptanceScenarios.json",
);
const screenshotsDirectory = path.join(outputDirectory, "screenshots");
const frontendPort = 1437;
const frontendOrigin = `http://127.0.0.1:${frontendPort}`;
const MAX_SCALED_VIEWPORT_ROUNDING = 4;
const uiTimeoutMilliseconds = Number(
  process.env.MYALBUNS_UI_ACCEPTANCE_TIMEOUT_MS ?? "60000",
);
const deviceScaleFactor = Number(
  process.env.MYALBUNS_UI_DEVICE_SCALE_FACTOR ?? "1",
);
if (
  !Number.isInteger(uiTimeoutMilliseconds) ||
  uiTimeoutMilliseconds < 5_000 ||
  uiTimeoutMilliseconds > 180_000
) {
  throw new Error(
    "MYALBUNS_UI_ACCEPTANCE_TIMEOUT_MS must be an integer between 5000 and 180000",
  );
}
if (
  !Number.isFinite(deviceScaleFactor) ||
  deviceScaleFactor < 0.5 ||
  deviceScaleFactor > 4
) {
  throw new Error(
    "MYALBUNS_UI_DEVICE_SCALE_FACTOR must be a number between 0.5 and 4",
  );
}
const viewportRoundingTolerance =
  deviceScaleFactor === 1 ? 0 : MAX_SCALED_VIEWPORT_ROUNDING;
const delay = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

for (const [label, candidate] of [
  ["workspace", workspace],
  ["manifest", manifestPath],
  ["Microsoft Edge", edgeExecutable],
  ["Microsoft Edge WebDriver", driverExecutable],
]) {
  if (!existsSync(candidate)) throw new Error(`${label} was not found: ${candidate}`);
}

mkdirSync(screenshotsDirectory, { recursive: true });
const manifest = validateUiAcceptanceManifest(
  JSON.parse(readFileSync(manifestPath, "utf8")),
);
const requestedScenarioIds = (
  process.env.MYALBUNS_UI_SCENARIO_IDS ?? ""
)
  .split(",")
  .map((id) => id.trim())
  .filter(Boolean);
const requestedScenarioIdSet = new Set(requestedScenarioIds);
if (requestedScenarioIdSet.size !== requestedScenarioIds.length) {
  throw new Error("MYALBUNS_UI_SCENARIO_IDS must not contain duplicates");
}
const knownScenarioIds = new Set(manifest.scenarios.map((scenario) => scenario.id));
const unknownScenarioIds = requestedScenarioIds.filter(
  (id) => !knownScenarioIds.has(id),
);
if (unknownScenarioIds.length > 0) {
  throw new Error(
    `MYALBUNS_UI_SCENARIO_IDS contains unknown ids: ${unknownScenarioIds.join(", ")}`,
  );
}
const scenariosToCapture =
  requestedScenarioIds.length === 0
    ? manifest.scenarios
    : manifest.scenarios.filter((scenario) =>
        requestedScenarioIdSet.has(scenario.id),
      );

function gitOutput(arguments_) {
  try {
    return execFileSync("git.exe", arguments_, {
      cwd: workspace,
      encoding: "utf8",
      windowsHide: true,
    }).trim();
  } catch {
    return "unavailable";
  }
}

function captureSourceInputs() {
  const gitCommit = gitOutput(["rev-parse", "HEAD"]);
  const status = gitOutput(["status", "--porcelain"]);
  return {
    dirty: status === "unavailable" ? null : status !== "",
    gitCommit,
  };
}

const initialSourceInputs = captureSourceInputs();

const evidence = {
  schemaVersion: 2,
  gate: "ui-acceptance",
  collectedAtUtc: new Date().toISOString(),
  gitCommit: initialSourceInputs.gitCommit,
  sourceInputsDirty: initialSourceInputs.dirty,
  sourceInputs: {
    initial: initialSourceInputs,
    final: null,
  },
  captureStatus: "capture-failed",
  reviewStatus: "not-reviewed",
  scenarioFilter:
    requestedScenarioIds.length === 0 ? null : requestedScenarioIds,
  browser: {
    deviceScaleFactor,
    name: "Microsoft Edge",
    version: process.env.MYALBUNS_UI_BROWSER_VERSION ?? "unknown",
    driverVersion: process.env.MYALBUNS_UI_DRIVER_VERSION ?? "unknown",
    mode: "headless-new",
    viewportRoundingTolerance,
  },
  cleanupCompleted: false,
  scenarios: [],
};

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

async function waitForHttp(url, label, timeoutMilliseconds = 30_000) {
  const deadline = Date.now() + timeoutMilliseconds;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
      lastError = new Error(`${label} returned HTTP ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(100);
  }
  throw new Error(
    `${label} did not become ready: ${lastError instanceof Error ? lastError.message : String(lastError)}`,
  );
}

function webdriverClient(baseUrl) {
  return async (method, endpoint, body, timeoutMilliseconds = 30_000) => {
    let response;
    try {
      response = await fetch(`${baseUrl}${endpoint}`, {
        method,
        headers:
          body === undefined ? undefined : { "content-type": "application/json" },
        body: body === undefined ? undefined : JSON.stringify(body),
        signal: AbortSignal.timeout(timeoutMilliseconds),
      });
    } catch (error) {
      throw new Error(
        `${method} ${endpoint} did not complete within ${timeoutMilliseconds} ms: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
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

function startLoggedProcess(label, executablePath, arguments_, logPath) {
  const logDescriptor = openSync(logPath, "a");
  let child;
  try {
    child = spawn(executablePath, arguments_, {
      cwd: workspace,
      windowsHide: true,
      stdio: ["ignore", logDescriptor, logDescriptor],
    });
  } finally {
    closeSync(logDescriptor);
  }
  if (!child.pid) throw new Error(`${label} returned no process id`);
  return { child, instance: null, label };
}

async function captureManagedRoot(managedProcess) {
  managedProcess.instance = await waitForProcessInstance(
    managedProcess.child.pid,
    managedProcess.label,
  );
  return managedProcess;
}

function processDepth(instance, byId, rootIds) {
  let depth = 0;
  let current = instance;
  const visited = new Set();
  while (!rootIds.has(current.processId) && byId.has(current.parentProcessId)) {
    if (visited.has(current.processId)) break;
    visited.add(current.processId);
    current = byId.get(current.parentProcessId);
    depth += 1;
  }
  return depth;
}

async function terminateManagedProcesses(managedProcesses) {
  const roots = managedProcesses.map((entry) => entry.instance).filter(Boolean);
  for (const entry of managedProcesses) {
    if (!entry.instance && entry.child.exitCode === null) entry.child.kill();
  }
  const forest = mergeProcessInstances(
    roots,
    roots.flatMap((root) => processForestInstances([root])),
  );
  const byId = new Map(forest.map((instance) => [instance.processId, instance]));
  const rootIds = new Set(roots.map((instance) => instance.processId));
  const deepestFirst = [...forest].sort(
    (left, right) =>
      processDepth(right, byId, rootIds) - processDepth(left, byId, rootIds),
  );
  for (const instance of deepestFirst) terminateProcessInstance(instance);

  const deadline = Date.now() + 30_000;
  let alive = aliveProcessInstances(forest);
  while (alive.length > 0 && Date.now() < deadline) {
    for (const instance of alive) terminateProcessInstance(instance);
    await delay(100);
    alive = aliveProcessInstances(forest);
  }
  if (alive.length > 0) {
    throw new Error(
      `managed processes remained alive: ${alive.map((entry) => `${entry.name}#${entry.processId}`).join(", ")}`,
    );
  }
}

async function waitForElement(request, sessionId, selector, label) {
  return waitForLocatedElement(
    request,
    sessionId,
    { using: "css selector", value: selector },
    label,
  );
}

async function waitForLocatedElement(request, sessionId, locator, label) {
  const deadline = Date.now() + uiTimeoutMilliseconds;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const element = await request("POST", `/session/${sessionId}/element`, {
        using: locator.using,
        value: locator.value,
      });
      const id = webdriverElementId(element);
      const displayed = await request(
        "GET",
        `/session/${sessionId}/element/${encodeURIComponent(id)}/displayed`,
      );
      if (displayed) return id;
    } catch (error) {
      lastError = error;
    }
    await delay(100);
  }
  throw new Error(
    `${label} did not expose ${locator.value}: ${lastError instanceof Error ? lastError.message : String(lastError ?? "not displayed")}`,
  );
}

function xpathLiteral(value) {
  if (!value.includes("'")) return `'${value}'`;
  if (!value.includes('"')) return `"${value}"`;
  return `concat(${value
    .split("'")
    .map((part) => `'${part}'`)
    .join(`, "'", `)})`;
}

async function waitForButtonText(request, sessionId, text, label) {
  return waitForLocatedElement(
    request,
    sessionId,
    {
      using: "xpath",
      value: `//button[normalize-space(.)=${xpathLiteral(text)}]`,
    },
    label,
  );
}

async function waitForNavigation(request, sessionId, expectedUrl, label) {
  const deadline = Date.now() + uiTimeoutMilliseconds;
  let lastState;
  while (Date.now() < deadline) {
    try {
      const currentUrl = await request("GET", `/session/${sessionId}/url`);
      const readyState = await execute(
        request,
        sessionId,
        "return document.readyState;",
      );
      lastState = { currentUrl, readyState };
      if (
        new URL(currentUrl).href === new URL(expectedUrl).href &&
        readyState !== "loading"
      ) {
        return;
      }
    } catch (error) {
      lastState = {
        error: error instanceof Error ? error.message : String(error),
      };
    }
    await delay(100);
  }
  throw new Error(`${label} navigation did not settle: ${JSON.stringify(lastState)}`);
}

async function execute(request, sessionId, script, args = []) {
  return request("POST", `/session/${sessionId}/execute/sync`, {
    script,
    args,
  });
}

async function setExactViewport(request, sessionId, viewport) {
  await request("POST", `/session/${sessionId}/ms/cdp/execute`, {
    cmd: "Emulation.clearDeviceMetricsOverride",
    params: {},
  });
  let outerWidth = viewport.width;
  let outerHeight = viewport.height;
  for (let attempt = 0; attempt < 4; attempt += 1) {
    await request("POST", `/session/${sessionId}/window/rect`, {
      x: 0,
      y: 0,
      width: outerWidth,
      height: outerHeight,
    });
    const measured = await execute(
      request,
      sessionId,
      "return { width: window.innerWidth, height: window.innerHeight };",
    );
    if (
      Math.abs(measured.width - viewport.width) <=
        viewportRoundingTolerance &&
      Math.abs(measured.height - viewport.height) <=
        viewportRoundingTolerance
    ) {
      return;
    }
    outerWidth += viewport.width - measured.width;
    outerHeight += viewport.height - measured.height;
  }
  await request("POST", `/session/${sessionId}/ms/cdp/execute`, {
    cmd: "Emulation.setDeviceMetricsOverride",
    params: {
      width: viewport.width,
      height: viewport.height,
      deviceScaleFactor,
      mobile: false,
    },
  });
  const measured = await execute(
    request,
    sessionId,
    "return { width: window.innerWidth, height: window.innerHeight };",
  );
  if (
    Math.abs(measured.width - viewport.width) <= viewportRoundingTolerance &&
    Math.abs(measured.height - viewport.height) <= viewportRoundingTolerance
  ) {
    return;
  }
  throw new Error(
    `viewport is ${measured.width} × ${measured.height}, expected ${viewport.width} × ${viewport.height} within ${viewportRoundingTolerance} px`,
  );
}

async function settleDocument(request, sessionId) {
  await execute(
    request,
    sessionId,
    `
      let style = document.getElementById("myalbuns-ui-acceptance-stability");
      if (!style) {
        style = document.createElement("style");
        style.id = "myalbuns-ui-acceptance-stability";
        style.textContent = "*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}html{scrollbar-width:none!important}body::-webkit-scrollbar{display:none!important}";
        document.head.appendChild(style);
      }
      return true;
    `,
  );
  await request("POST", `/session/${sessionId}/execute/async`, {
    script: `
      const done = arguments[arguments.length - 1];
      const images = Array.from(document.images);
      const imagesReady = Promise.all(images.map((image) => image.complete ? Promise.resolve() : new Promise((resolve) => {
        image.addEventListener("load", resolve, { once: true });
        image.addEventListener("error", resolve, { once: true });
      })));
      const fontsReady = document.fonts?.ready ?? Promise.resolve();
      const boundedAssets = Promise.race([
        Promise.all([imagesReady, fontsReady]),
        new Promise((resolve) => setTimeout(resolve, 2000)),
      ]);
      boundedAssets.then(() => requestAnimationFrame(() => requestAnimationFrame(() => done(true))));
    `,
    args: [],
  });
}

async function navigateAndCapture({
  request,
  sessionId,
  scenario,
  servedPath,
  readySelector,
  actions,
  captureSelector,
  screenshotPath,
  label,
}) {
  await setExactViewport(request, sessionId, scenario.viewport);
  try {
    const targetUrl = new URL(servedPath, frontendOrigin);
    targetUrl.searchParams.set("ui-acceptance-scenario", label);
    await request("POST", `/session/${sessionId}/url`, {
      url: targetUrl.href,
    });
    await waitForNavigation(request, sessionId, targetUrl.href, label);
    await settleDocument(request, sessionId);
    await neutralizeUiAcceptancePointer({
      request,
      sessionId,
      viewport: scenario.viewport,
    });
    await settleDocument(request, sessionId);
    const guardBrowserZoom = requiresBrowserZoomInvariant(actions);
    const browserZoomBefore = guardBrowserZoom
      ? await captureBrowserZoomState({
          execute: (script) => execute(request, sessionId, script),
        })
      : null;
    const locateSelector = (selector) =>
      waitForElement(request, sessionId, selector, `${label} action`);
    const locateText = (text) =>
      waitForButtonText(request, sessionId, text, `${label} action`);
    for (const action of actions) {
      await performUiAcceptanceAction({
        action,
        execute: (script, args) => execute(request, sessionId, script, args),
        locateSelector,
        locateText,
        request,
        sessionId,
      });
      await settleDocument(request, sessionId);
    }
    await waitForElement(request, sessionId, readySelector, label);
    await settleDocument(request, sessionId);
    if (browserZoomBefore) {
      const browserZoomAfter = await captureBrowserZoomState({
        execute: (script) => execute(request, sessionId, script),
      });
      assertBrowserZoomUnchanged({
        after: browserZoomAfter,
        before: browserZoomBefore,
        label,
      });
    }
    const screenshot = await captureUiAcceptanceScreenshot({
      captureSelector,
      locateSelector: (selector) =>
        waitForElement(request, sessionId, selector, `${label} capture`),
      request,
      sessionId,
    });
    if (typeof screenshot !== "string" || !screenshot) {
      throw new Error(`${label} returned no screenshot`);
    }
    writeFileSync(screenshotPath, Buffer.from(screenshot, "base64"));
    return {
      browserZoomStatus: guardBrowserZoom ? "unchanged" : undefined,
    };
  } finally {
    await request("DELETE", `/session/${sessionId}/actions`);
  }
}

const managedProcesses = [];
let request;
let sessionId;
let fatalError;
let cleanupError;

try {
  const preexistingListener = captureListeningProcessInstance(frontendPort);
  if (preexistingListener) {
    throw new Error(
      `port ${frontendPort} is already owned by ${preexistingListener.name}#${preexistingListener.processId}; the gate will not reuse or stop it`,
    );
  }

  const viteEntry = path.join(workspace, "node_modules", "vite", "bin", "vite.js");
  if (!existsSync(viteEntry)) throw new Error("Vite is not installed; run npm ci");
  const vite = startLoggedProcess(
    "Vite",
    process.execPath,
    [viteEntry, "--host", "127.0.0.1", "--port", String(frontendPort), "--strictPort"],
    path.join(outputDirectory, "vite.log"),
  );
  managedProcesses.push(vite);
  await captureManagedRoot(vite);
  await waitForHttp(`${frontendOrigin}/ui-acceptance.html`, "Vite");
  console.log(`UI acceptance frontend ready at ${frontendOrigin}`);

  const driverPort = await freePort();
  const driver = startLoggedProcess(
    "Microsoft Edge WebDriver",
    driverExecutable,
    [`--port=${driverPort}`, "--host=127.0.0.1"],
    path.join(outputDirectory, "webdriver.log"),
  );
  managedProcesses.push(driver);
  await captureManagedRoot(driver);
  const driverOrigin = `http://127.0.0.1:${driverPort}`;
  await waitForHttp(`${driverOrigin}/status`, "Microsoft Edge WebDriver");
  console.log(`Microsoft Edge WebDriver ready at ${driverOrigin}`);
  request = webdriverClient(driverOrigin);

  const session = await request("POST", "/session", {
    capabilities: {
      alwaysMatch: {
        browserName: "MicrosoftEdge",
        pageLoadStrategy: "none",
        "ms:edgeOptions": {
          binary: edgeExecutable,
          args: [
            "--headless=new",
            "--disable-gpu",
            "--disable-background-networking",
            "--disable-features=msEdgeFirstRunExperience",
            "--force-color-profile=srgb",
            `--force-device-scale-factor=${deviceScaleFactor}`,
            "--hide-scrollbars",
            "--no-first-run",
          ],
        },
      },
    },
  });
  sessionId = session.sessionId;
  if (!sessionId) throw new Error("Microsoft Edge WebDriver returned no W3C session id");
  console.log("Headless Microsoft Edge session created");
  await request("POST", `/session/${sessionId}/timeouts`, {
    implicit: 0,
    pageLoad: 30_000,
    script: 30_000,
  });

  for (const scenario of scenariosToCapture) {
    console.log(`Capturing ${scenario.id}`);
    const paired = scenario.comparison.kind === "paired";
    const implementationName = `${scenario.id}-implementation.png`;
    const referenceName = `${scenario.id}-reference.png`;
    const implementationPath = path.join(screenshotsDirectory, implementationName);
    const referencePath = path.join(screenshotsDirectory, referenceName);
    const result = {
      ...scenario,
      captureStatus: "capture-failed",
      comparisonStatus: paired
        ? "paired-unreviewed"
        : "reference-unavailable",
      reviewStatus: "not-reviewed",
      implementationUrl: `${frontendOrigin}${scenario.implementationPath}`,
      implementationScreenshot: `screenshots/${implementationName}`,
      ...(paired
        ? {
            referenceUrl: `${frontendOrigin}${scenario.referencePath}`,
            referenceScreenshot: `screenshots/${referenceName}`,
          }
        : {}),
    };
    try {
      const implementationCapture = await navigateAndCapture({
        request,
        sessionId,
        scenario,
        servedPath: scenario.implementationPath,
        readySelector: scenario.readySelector,
        actions: scenario.actions,
        captureSelector:
          scenario.comparison.implementationCaptureSelector,
        screenshotPath: implementationPath,
        label: `${scenario.id} implementation`,
      });
      if (implementationCapture.browserZoomStatus) {
        result.browserZoomStatus = implementationCapture.browserZoomStatus;
      }
      if (paired) {
        await navigateAndCapture({
          request,
          sessionId,
          scenario,
          servedPath: scenario.referencePath,
          readySelector: scenario.referenceReadySelector ?? "body",
          actions: scenario.referenceActions ?? [],
          captureSelector: scenario.comparison.referenceCaptureSelector,
          screenshotPath: referencePath,
          label: `${scenario.id} reference`,
        });
      }
      result.captureStatus = "captured-unreviewed";
      console.log(`Captured ${scenario.id}`);
    } catch (error) {
      result.error = error instanceof Error ? error.stack ?? error.message : String(error);
      console.error(`Capture failed for ${scenario.id}: ${result.error}`);
    }
    evidence.scenarios.push(result);
  }
} catch (error) {
  fatalError = error;
  const message = error instanceof Error ? error.stack ?? error.message : String(error);
  for (const scenario of scenariosToCapture.slice(evidence.scenarios.length)) {
    const paired = scenario.comparison.kind === "paired";
    evidence.scenarios.push({
      ...scenario,
      captureStatus: "capture-failed",
      comparisonStatus: paired
        ? "paired-unreviewed"
        : "reference-unavailable",
      reviewStatus: "not-reviewed",
      implementationUrl: `${frontendOrigin}${scenario.implementationPath}`,
      ...(paired
        ? { referenceUrl: `${frontendOrigin}${scenario.referencePath}` }
        : {}),
      error: message,
    });
  }
} finally {
  if (request && sessionId) {
    try {
      await request("DELETE", `/session/${sessionId}`);
      await delay(2_000);
    } catch {
      // Exact process-instance cleanup below remains authoritative.
    }
  }
  try {
    await terminateManagedProcesses(managedProcesses);
    const listener = captureListeningProcessInstance(frontendPort);
    if (listener) {
      throw new Error(
        `port ${frontendPort} remained owned by ${listener.name}#${listener.processId}`,
      );
    }
    evidence.cleanupCompleted = true;
  } catch (error) {
    cleanupError = error;
    evidence.cleanupError =
      error instanceof Error ? error.stack ?? error.message : String(error);
  }
}

const allCaptured = evidence.scenarios.every(
  (scenario) => scenario.captureStatus === "captured-unreviewed",
);
evidence.captureStatus = allCaptured
  ? "captured-unreviewed"
  : "capture-failed";
const sourceInputsResult = finalizeUiAcceptanceSourceEvidence(
  evidence,
  captureSourceInputs(),
);
const evidencePath = path.join(outputDirectory, "evidence.json");
const reportPath = path.join(outputDirectory, "report.html");
writeFileSync(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`, "utf8");
writeFileSync(reportPath, renderUiAcceptanceReport(evidence), "utf8");

console.log(
  JSON.stringify({
    captureStatus: evidence.captureStatus,
    cleanupCompleted: evidence.cleanupCompleted,
    evidencePath,
    reportPath,
    sourceInputs: evidence.sourceInputs,
  }),
);
if (
  fatalError ||
  cleanupError ||
  !allCaptured ||
  sourceInputsResult.changedDuringCapture ||
  !sourceInputsResult.snapshotsKnown
) {
  process.exitCode = 1;
}
