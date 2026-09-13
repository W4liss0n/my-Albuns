// Exercise native Windows pixels, including loss of the hidden Global browser.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  captureListeningProcessInstance,
  processInstancesByExecutable,
  terminateProcessInstance,
  waitForProcessInstance,
} from "./DevLifecycleProcessInstances.mjs";
import { findFreeTcpPort } from "./GateWebDriver.mjs";

const [applicationArgument, projectArgument, outputArgument, countArgument] = process.argv.slice(2);
assert.ok(applicationArgument && projectArgument && outputArgument && countArgument,
  "Usage: Run-OpeningProgressPaintGate.mjs <application> <fixture-project> <new-output> <image-count> [--fail-owner-browser]");
const application = path.resolve(applicationArgument);
const project = path.resolve(projectArgument);
const output = path.resolve(outputArgument);
const expectedImages = Number(countArgument);
assert.ok(Number.isSafeInteger(expectedImages) && expectedImages > 0);
assert.ok(existsSync(application) && existsSync(project));
assert.ok(!existsSync(output), "Use a new evidence directory and an isolated fixture Project");
mkdirSync(output, { recursive: true });
const dataRoot = path.join(output, "process-data");
const environment = { ...process.env, MYALBUNS_PROCESS_GATE_DATA_ROOT: dataRoot };
for (const key of Object.keys(environment)) {
  if (key.startsWith("MYALBUNS_DEV_") || key === "TAURI_WEBVIEW_AUTOMATION" ||
      key === "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS") delete environment[key];
}
const failOwner = process.argv.includes("--fail-owner-browser");
let globalPort;
if (failOwner) {
  globalPort = await findFreeTcpPort();
  environment.MYALBUNS_DEV_GLOBAL_WEBVIEW_DEBUG_PORT = String(globalPort);
  environment.MYALBUNS_DEV_OPENING_DIALOG_WEBVIEW_DEBUG_PORT = String(await findFreeTcpPort());
  environment.MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT = String(await findFreeTcpPort());
  environment.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = `--remote-debugging-port=${globalPort}`;
}
const pause = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));
const instances = [];
const global = spawn(application, [], { windowsHide: true, stdio: "ignore", env: environment });
instances.push(await waitForProcessInstance(global.pid, "opening paint Global"));
let observer;
let observerCompletion;
try {
  observer = spawn("powershell.exe", ["-NoProfile", "-File", "scripts/Observe-OpeningProgressPaint.ps1",
    "-TargetProcessId", String(global.pid), "-OutputPath", output],
  { windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
  let observerLog = "";
  observer.stdout.on("data", (data) => { observerLog += data; });
  observer.stderr.on("data", (data) => { observerLog += data; });
  observerCompletion = new Promise((resolve) => observer.once("exit", (code) => resolve(code)));
  for (let attempt = 0; attempt < 100 && !existsSync(path.join(output, "observer.ready")); attempt++) await pause(50);
  assert.ok(existsSync(path.join(output, "observer.ready")), "The native observer must start");
  const browser = failOwner ? captureListeningProcessInstance(globalPort) : null;
  if (failOwner) {
    assert.equal(browser?.parentProcessId, global.pid, "Only the test Global browser may be terminated");
    assert.ok(browser.commandLine.includes(path.join(dataRoot, "Local", "MyAlbuns2", "State", "WebView2", "global")));
  }
  spawn(application, [project], { windowsHide: true, stdio: "ignore", env: environment });
  if (failOwner) {
    for (let attempt = 0; attempt < 100 && !existsSync(path.join(output, "painted.png")); attempt++) await pause(50);
    assert.ok(existsSync(path.join(output, "painted.png")), "The dialog must paint before browser failure");
    writeFileSync(path.join(output, "injected-failure.json"), JSON.stringify({ utc: Date.now(), browser }, null, 2));
    terminateProcessInstance(browser);
  }
  const observerCode = await observerCompletion;
  writeFileSync(path.join(output, "observer.log"), observerLog);
  assert.equal(observerCode, 0, observerLog);
  const samples = JSON.parse(readFileSync(path.join(output, "paint-samples.json"), "utf8").replace(/^\uFEFF/, ""));
  const exposed = samples.filter((sample) => sample.Exposed);
  assert.ok(exposed.length >= 2, "The native dialog must be exposed for measurement");
  let blackStart = null;
  let longestBlackMilliseconds = 0;
  for (const sample of samples) {
    if (sample.Exposed && sample.BlackRatio > 0.96) {
      blackStart ??= sample.Milliseconds;
      longestBlackMilliseconds = Math.max(longestBlackMilliseconds, sample.Milliseconds - blackStart);
    } else blackStart = null;
  }
  const cacheDirectory = path.join(dataRoot, "Local", "MyAlbuns2", "Cache");
  const cacheEntries = readdirSync(cacheDirectory).reduce((count, namespace) => {
    const metadata = path.join(cacheDirectory, namespace, "metadata.json");
    return count + (existsSync(metadata) ? JSON.parse(readFileSync(metadata, "utf8")).entries.length : 0);
  }, 0);
  const logs = path.join(dataRoot, "Local", "MyAlbuns2", "Logs");
  const records = readdirSync(logs).flatMap((file) => readFileSync(path.join(logs, file), "utf8")
    .split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line)));
  const ready = records.find((record) => record.event === "project_ui_ready");
  writeFileSync(path.join(output, "result.json"), JSON.stringify({
    application, failOwner, expectedImages, cacheEntries, longestBlackMilliseconds,
    exposedSamples: exposed.length, projectReady: Boolean(ready),
  }, null, 2));
  assert.ok(longestBlackMilliseconds < 500, `The native dialog stayed black for ${longestBlackMilliseconds} ms`);
  assert.ok(existsSync(path.join(output, "progress.png")), "Measured progress must actually paint beyond the initial indeterminate indicator");
  assert.equal(cacheEntries, expectedImages, "All fixture images must be prepared before release");
  assert.ok(ready, "The Project must complete its UI readiness handshake");
  assert.ok(!records.some((record) => record.event === "imaging_process_spawned" &&
    record.process_id === ready.process_id && record.operation === "cache" && record.timestamp > ready.timestamp),
  "Cache reconstruction must finish before the Project is released");
  console.log(JSON.stringify({ passed: true, longestBlackMilliseconds, cacheEntries }));
} finally {
  for (const instance of processInstancesByExecutable(application, "myalbuns-desktop.exe")) {
    if (instances.some((owner) => owner.processId === instance.parentProcessId)) instances.push(instance);
  }
  for (const instance of instances.reverse()) terminateProcessInstance(instance);
  // The observer exits when its exact Global process exits.
  if (observerCompletion) await observerCompletion;
}
