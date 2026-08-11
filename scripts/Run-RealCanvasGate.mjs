import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import net from "node:net";
import path from "node:path";
import { stripVTControlCharacters } from "node:util";

const [
  evidenceDirectoryArgument,
  screenshotArgument,
  applicationArgument,
  tauriDriverArgument,
  nativeDriverArgument,
] = process.argv.slice(2);
if (
  !evidenceDirectoryArgument ||
  !screenshotArgument ||
  !applicationArgument ||
  !tauriDriverArgument ||
  !nativeDriverArgument
) {
  throw new Error(
    "Usage: Run-RealCanvasGate.mjs <evidence-directory> <screenshot> <application> <tauri-driver> <native-driver>",
  );
}

const evidenceDirectory = path.resolve(evidenceDirectoryArgument);
const screenshotPath = path.resolve(screenshotArgument);
const applicationPath = path.resolve(applicationArgument);
const tauriDriverPath = path.resolve(tauriDriverArgument);
const nativeDriverPath = path.resolve(nativeDriverArgument);
for (const [label, candidate] of [
  ["application", applicationPath],
  ["tauri-driver", tauriDriverPath],
  ["native WebDriver", nativeDriverPath],
]) {
  if (!existsSync(candidate)) throw new Error(`${label} was not found: ${candidate}`);
}

const canvasEvidence = JSON.parse(
  readFileSync(path.join(evidenceDirectory, "canvas.json"), "utf8"),
);
const projectPath = path.resolve(canvasEvidence.tauriProjectPath);
if (!existsSync(projectPath)) {
  throw new Error(`The retained Tauri Project was not found: ${projectPath}`);
}
const projectDocument = JSON.parse(readFileSync(projectPath, "utf8"));
if (typeof projectDocument.projectId !== "string" || !projectDocument.projectId) {
  throw new Error("The retained Tauri Project has no projectId");
}
const expectedPreviewCount = canvasEvidence.compositionMediaOrder.length;
const processDataRoot = path.resolve(
  process.env.MYALBUNS_PROCESS_GATE_DATA_ROOT ?? "",
);
if (!process.env.MYALBUNS_PROCESS_GATE_DATA_ROOT) {
  throw new Error("MYALBUNS_PROCESS_GATE_DATA_ROOT is required");
}
const projectNamespace = `project-${createHash("sha256")
  .update(projectDocument.projectId, "utf8")
  .digest("hex")}`;
const webviewDataDirectory = path.join(
  processDataRoot,
  "Local",
  "MyAlbuns2",
  "State",
  "WebView2",
  projectNamespace,
);
const desktopLogDirectory = path.join(
  processDataRoot,
  "Local",
  "MyAlbuns2",
  "Logs",
);

function desktopLogOffsets() {
  if (!existsSync(desktopLogDirectory)) return new Map();
  return new Map(
    readdirSync(desktopLogDirectory)
      .filter((name) => /^myalbuns-desktop\..+\.jsonl$/.test(name))
      .map((name) => {
        const file = path.join(desktopLogDirectory, name);
        return [file, statSync(file).size];
      }),
  );
}

function appendedDesktopLogs(offsets) {
  if (!existsSync(desktopLogDirectory)) return "";
  return readdirSync(desktopLogDirectory)
    .filter((name) => /^myalbuns-desktop\..+\.jsonl$/.test(name))
    .map((name) => {
      const file = path.join(desktopLogDirectory, name);
      return readFileSync(file).subarray(offsets.get(file) ?? 0).toString("utf8");
    })
    .join("\n");
}

const initialDesktopLogOffsets = desktopLogOffsets();
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

function collectOutput(child) {
  let output = "";
  child.stdout?.on("data", (chunk) => {
    output += chunk.toString();
  });
  child.stderr?.on("data", (chunk) => {
    output += chunk.toString();
  });
  return () => output;
}

function eventCount(output, event) {
  return (
    output.split(`"event":"${event}"`).length - 1 +
    output.split(`event="${event}"`).length - 1
  );
}

function hasEvent(output, event) {
  return eventCount(output, event) > 0;
}

async function waitForDriver(baseUrl, timeoutMilliseconds) {
  const deadline = Date.now() + timeoutMilliseconds;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${baseUrl}/status`);
      if (response.ok) return;
      lastError = new Error(`WebDriver status returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(100);
  }
  throw lastError ?? new Error("tauri-driver did not become ready");
}

function webdriverClient(baseUrl) {
  return async (method, endpoint, body) => {
    const response = await fetch(`${baseUrl}${endpoint}`, {
      method,
      headers: body === undefined ? undefined : { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
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

process.env.MYALBUNS_TAURI_WEBDRIVER_PROJECT = projectPath;
process.env.TAURI_WEBVIEW_AUTOMATION = "true";

const driverPort = await freePort();
const nativePort = await freePort();
const driver = spawn(
  tauriDriverPath,
  [
    "--port",
    String(driverPort),
    "--native-port",
    String(nativePort),
    "--native-driver",
    nativeDriverPath,
  ],
  {
    cwd: path.dirname(applicationPath),
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
  },
);
const driverOutput = collectOutput(driver);
const baseUrl = `http://127.0.0.1:${driverPort}`;
const request = webdriverClient(baseUrl);
let sessionId;
let driverTerminated = false;

function terminateDriverTree() {
  if (driverTerminated || !driver.pid) return;
  driverTerminated = true;
  spawnSync("taskkill.exe", ["/PID", String(driver.pid), "/T", "/F"], {
    windowsHide: true,
    stdio: "ignore",
  });
}

try {
  await waitForDriver(baseUrl, 30_000);
  const session = await request("POST", "/session", {
    capabilities: {
      alwaysMatch: {
        pageLoadStrategy: "none",
        "tauri:options": {
          application: applicationPath,
          args: [],
          webviewOptions: {
            userDataFolder: webviewDataDirectory,
          },
        },
      },
    },
  });
  sessionId = session.sessionId;
  if (!sessionId) throw new Error("tauri-driver returned no W3C session id");
  const gateTimeoutMilliseconds = Number(
    process.env.MYALBUNS_WEBVIEW_GATE_TIMEOUT_MS ?? "90000",
  );
  await request("POST", `/session/${sessionId}/timeouts`, {
    implicit: Math.min(gateTimeoutMilliseconds, 1_000),
    pageLoad: 5_000,
    script: 5_000,
  });

  const windowDeadline = Date.now() + gateTimeoutMilliseconds;
  let handles = [];
  let elementId;
  let lastWindowError;
  let lastWindowState;
  while (!elementId && Date.now() < windowDeadline) {
    handles = await request("GET", `/session/${sessionId}/window/handles`);
    if (handles.length > 1) {
      throw new Error(`Expected one productive Project WebView, received ${handles.length}`);
    }
    if (handles.length === 1) {
      try {
        await request("POST", `/session/${sessionId}/window`, { handle: handles[0] });
        const element = await request("POST", `/session/${sessionId}/element`, {
          using: "css selector",
          value: "canvas.pixi-canvas",
        });
        elementId = element["element-6066-11e4-a52e-4f735466cecf"];
      } catch (error) {
        lastWindowError = error;
        try {
          lastWindowState = {
            url: await request("GET", `/session/${sessionId}/url`),
            source: (await request("GET", `/session/${sessionId}/source`)).slice(0, 2_000),
          };
        } catch {
          // The handle can disappear while the Project WebView is starting.
        }
      }
    }
    if (!elementId) await delay(100);
  }
  if (!elementId) {
    throw new Error(
      `The productive Canvas WebView did not become stable: ${lastWindowError ?? "no window"}; state=${JSON.stringify(lastWindowState)}`,
    );
  }
  const canvasRect = await request(
    "GET",
    `/session/${sessionId}/element/${encodeURIComponent(elementId)}/rect`,
  );
  const pageSource = await request("GET", `/session/${sessionId}/source`);
  const originalPathExposed = ["background-original.png", "overlay-original.png"].some(
    (name) => pageSource.includes(name),
  );
  if (originalPathExposed) {
    throw new Error("An Original pathname crossed the productive WebView boundary");
  }

  await delay(Math.min(gateTimeoutMilliseconds, 5_000));
  const screenshot = await request(
    "GET",
    `/session/${sessionId}/screenshot`,
  );
  if (typeof screenshot !== "string" || !screenshot) {
    throw new Error("The productive Tauri WebView2 returned no W3C screenshot");
  }
  writeFileSync(screenshotPath, Buffer.from(screenshot, "base64"));

  const tracerDeadline = Date.now() + 5_000;
  let liveDriverOutput = "";
  do {
    liveDriverOutput = stripVTControlCharacters(driverOutput());
    const liveCount = eventCount(
      liveDriverOutput,
      "canvas_opaque_preview_texture_loaded",
    );
    if (liveCount >= expectedPreviewCount) break;
    await delay(100);
  } while (Date.now() < tracerDeadline);

  terminateDriverTree();
  await delay(500);
  const observableOutputs = [
    appendedDesktopLogs(initialDesktopLogOffsets),
    liveDriverOutput,
    driverOutput(),
  ].map(stripVTControlCharacters);
  const opaqueTextureLoadCount = Math.max(
    ...observableOutputs.map(
      (output) => eventCount(output, "canvas_opaque_preview_texture_loaded"),
    ),
  );
  const observed = (event) =>
    observableOutputs.some((output) => hasEvent(output, event));

  if (
    opaqueTextureLoadCount !== expectedPreviewCount ||
    !observed("media_preview_completed") ||
    observed("canvas_texture_load_failed") ||
    observed("canvas_preview_texture_transport_rejected")
  ) {
    throw new Error(
      `The productive Pixi tracer did not confirm every opaque preview (${JSON.stringify({ expectedPreviewCount, opaqueTextureLoadCount })})`,
    );
  }

  console.log(
    JSON.stringify({
      browserProcess: "Tauri WebView2 via tauri-driver",
      actualTauriApp: true,
      actualAlbumCanvas: true,
      actualPixiRuntime: true,
      originalPathExposedToWebView: false,
      opaqueResourceCount: opaqueTextureLoadCount,
      canvasWidth: canvasRect.width,
      canvasHeight: canvasRect.height,
      screenshotPath,
    }),
  );
} catch (error) {
  throw new Error(
    `${error instanceof Error ? error.message : String(error)}\ntauri-driver:\n${driverOutput()}`,
  );
} finally {
  terminateDriverTree();
}
