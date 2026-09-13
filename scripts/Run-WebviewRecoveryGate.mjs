// Crash only an isolated fixture's browser and verify the unsaved Host session.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { findFreeTcpPort } from "./GateWebDriver.mjs";
import { captureListeningProcessInstance, captureProcessInstance, processInstancesByExecutable, terminateProcessInstance, waitForProcessInstance } from "./DevLifecycleProcessInstances.mjs";
import { nativeOwnedWindowState } from "./NativeWindowObservation.mjs";
import { startWebviewFailureCapture } from "./WebviewFailureCapture.mjs";

const [executableArgument, fixtureArgument, outputArgument] = process.argv.slice(2);
assert.ok(executableArgument && fixtureArgument && outputArgument,
  "Usage: Run-WebviewRecoveryGate.mjs <application> <isolated-project-fixture> <new-output> [--procdump <signed-procdump64.exe>]");
const procdumpIndex = process.argv.indexOf("--procdump");
const procdump = procdumpIndex < 0 ? null : path.resolve(process.argv[procdumpIndex + 1]);
const executable = path.resolve(executableArgument);
const fixture = path.resolve(fixtureArgument);
const output = path.resolve(outputArgument);
assert.ok(!existsSync(output)); mkdirSync(output, { recursive: true });
const root = path.join(output, "process-data");
const hostPort = await findFreeTcpPort();
const environment = { ...process.env, MYALBUNS_PROCESS_GATE_DATA_ROOT: root,
  MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT: String(hostPort) };
delete environment.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS;
delete environment.TAURI_WEBVIEW_AUTOMATION;
const sockets = [], instances = [], monitors = [];
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const application = spawn(executable, [fixture], { windowsHide: true, stdio: "ignore", env: environment });
instances.push(await waitForProcessInstance(application.pid, "recovery gate Global"));
const records = () => {
  const directory = path.join(root, "Local", "MyAlbuns2", "Logs");
  return existsSync(directory) ? readdirSync(directory).flatMap(file => readFileSync(path.join(directory, file), "utf8")
    .split(/\r?\n/).flatMap(line => { try { return [JSON.parse(line)]; } catch { return []; } })) : [];
};
function currentBrowser() {
  const diagnostic = records().findLast(row => row.event === "webview_diagnostics_ready" && row.webview_label === "project");
  // The native event identifies the new browser even when Windows' listener
  // and process-creation snapshots disagree transiently during replacement.
  return diagnostic ? captureProcessInstance(diagnostic.browser_process_id) : captureListeningProcessInstance(hostPort);
}
async function waitUntil(predicate, description, timeout = 20000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { if (await predicate()) return; await pause(100); }
  throw new Error(description);
}
async function connect() {
  let target;
  await waitUntil(async () => {
    try {
      const list = await (await fetch(`http://127.0.0.1:${hostPort}/json/list`, { signal: AbortSignal.timeout(500) })).json();
      target = list.find(item => item.type === "page" && ["/", "/index.html"].includes(new URL(item.url).pathname));
    } catch {}
    return target;
  }, "The Project WebView must become reachable");
  const socket = new WebSocket(target.webSocketDebuggerUrl); sockets.push(socket);
  await new Promise((resolve, reject) => { socket.addEventListener("open", resolve, { once: true }); socket.addEventListener("error", reject, { once: true }); });
  let sequence = 0;
  const pending = new Map();
  socket.addEventListener("message", event => {
    const message = JSON.parse(event.data), item = pending.get(message.id);
    if (!item) return; pending.delete(message.id); clearTimeout(item.timer);
    if (message.error) item.reject(new Error(JSON.stringify(message.error))); else item.resolve(message.result);
  });
  socket.addEventListener("close", () => { for (const item of pending.values()) { clearTimeout(item.timer); item.reject(new Error("Browser disconnected")); } pending.clear(); });
  return async expression => {
    const result = await new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => { pending.delete(id); reject(new Error(`The Project did not respond: ${expression}`)); }, 15000);
      pending.set(id, { resolve, reject, timer });
      socket.send(JSON.stringify({ id, method: "Runtime.evaluate", params: { expression, awaitPromise: true, returnByValue: true } }));
    });
    if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
}
try {
  await waitUntil(() => records().some(row => row.event === "project_ui_ready"), "The initial Project must be ready");
  // Startup can itself exercise recovery; attach to the currently ready page.
  let evaluate = await connect();
  const host = records().find(row => row.event === "project_host_started");
  const instance = await waitForProcessInstance(host.process_id, "recovery gate Project"); instances.push(instance);
  const initial = await evaluate("window.__TAURI_INTERNALS__.invoke('project_state', {operationId:'before-crash'})");
  const dpi = initial.state.document.dpi === 301 ? 302 : 301;
  await evaluate(`window.__TAURI_INTERNALS__.invoke('apply_project_intent', {intent:{kind:'setDpi',dpi:${dpi}},onProgress:'__CHANNEL__:'+window.__TAURI_INTERNALS__.transformCallback(()=>{})})`);
  const expected = await evaluate("window.__TAURI_INTERNALS__.invoke('project_state', {operationId:'unsaved-before-crash'})");
  assert.equal(expected.state.dirty, true);
  assert.equal(expected.state.canUndo, true);
  const beforeWindow = nativeOwnedWindowState(instance).windows.find(window => window.visible && window.title.includes(initial.state.projectName));
  assert.ok(beforeWindow);
  const startupRecoveries = records().filter(row => row.event === "webview_recovery_ready" && row.webview_label === "project").length;
  if (startupRecoveries) {
    console.log("Startup recovered a spontaneous browser failure; waiting for its retry window before the two injected crashes.");
    await pause(61000);
  }
  for (let cycle = 1; cycle <= 2; cycle++) {
    // Keep the two deliberate crashes separate from the Runtime's spontaneous
    // startup failures. The retry limit itself is covered by the Rust guard tests.
    if (cycle > 1) await pause(61000);
    const beforeRecoveries = records().filter(row => row.event === "webview_recovery_ready" && row.webview_label === "project").length;
    const browser = currentBrowser();
    assert.equal(browser?.parentProcessId, host.process_id);
    assert.ok(browser.commandLine.includes(path.join(root, "Local", "MyAlbuns2", "State", "WebView2")));
    if (procdump) {
      const monitor = startWebviewFailureCapture(procdump, browser, path.join(output, `dump-${cycle}`));
      monitors.push(monitor);
      await monitor.ready();
    }
    const version = await (await fetch(`http://127.0.0.1:${hostPort}/json/version`)).json();
    const socket = new WebSocket(version.webSocketDebuggerUrl); sockets.push(socket);
    await new Promise((resolve, reject) => { socket.addEventListener("open", resolve, { once: true }); socket.addEventListener("error", reject, { once: true }); });
    socket.send(JSON.stringify({ id: 1, method: "Browser.crash" }));
    await waitUntil(() => records().filter(row => row.event === "webview_recovery_ready" && row.webview_label === "project").length > beforeRecoveries,
      "The Project must rebuild after its browser crashes");
    await waitUntil(() => {
      const log = records();
      const latest = log.filter(row => row.event === "webview_recovery_ready" && row.webview_label === "project").at(-1);
      return log.some(row => row.event === "media_preview_completed" && row.timestamp > latest.timestamp);
    }, "The recovered editor must finish displaying its images");
    evaluate = await connect();
    assert.notEqual(currentBrowser()?.processId, browser.processId);
    await waitUntil(() => evaluate("Boolean(document.querySelector('canvas.pixi-canvas'))"), "The recovered editor must render");
    const actual = await evaluate(`window.__TAURI_INTERNALS__.invoke('project_state', {operationId:'after-crash-${cycle}'})`);
    assert.deepEqual(actual, expected, "Recovery must preserve the full projection, unsaved changes and Undo");
    const afterWindow = nativeOwnedWindowState(instance).windows.find(window => window.hwnd === beforeWindow.hwnd);
    assert.ok(afterWindow?.visible, "Recovery must preserve the native window");
    assert.equal(afterWindow.clientWidth, beforeWindow.clientWidth);
    assert.equal(afterWindow.clientHeight, beforeWindow.clientHeight);
  }
  const result = { passed: true, hostProcessId: host.process_id, nativeWindow: beforeWindow.hwnd,
    revision: expected.state.revision, savedRevision: expected.state.savedRevision,
    events: records().filter(row => row.event.startsWith("webview_")) };
  writeFileSync(path.join(output, "result.json"), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ passed: true, recoveredCrashes: 2, unsavedChangesPreserved: true }));
} finally {
  for (const socket of sockets) socket.close();
  await Promise.all(monitors.map(monitor => monitor.stop()));
  for (const instance of processInstancesByExecutable(executable, "myalbuns-desktop.exe")) {
    if (instances.some(owner => owner.processId === instance.parentProcessId) && !instances.some(owner => owner.processId === instance.processId)) instances.push(instance);
  }
  for (const instance of instances.reverse()) terminateProcessInstance(instance);
}
