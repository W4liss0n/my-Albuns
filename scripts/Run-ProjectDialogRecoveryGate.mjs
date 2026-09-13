// Use only a disposable fixture: the gate changes its in-memory DPI without saving.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { findFreeTcpPort } from "./GateWebDriver.mjs";
import { captureProcessInstance, processInstancesByExecutable, terminateProcessInstance, waitForProcessInstance } from "./DevLifecycleProcessInstances.mjs";
import { nativeOwnedWindowState } from "./NativeWindowObservation.mjs";

const [executableArg, fixtureArg, outputArg, mode = "configuration"] = process.argv.slice(2);
assert.ok(executableArg && fixtureArg && outputArg,
  "Usage: Run-ProjectDialogRecoveryGate.mjs <application> <disposable-project> <new-output> [configuration|close|progress]");
assert.ok(["configuration", "close", "progress"].includes(mode));
const executable = path.resolve(executableArg), fixture = path.resolve(fixtureArg), output = path.resolve(outputArg);
assert.ok(!existsSync(output), "Evidence must have a new directory");
mkdirSync(output, { recursive: true });
const root = path.join(output, "process-data");
const ownerPort = await findFreeTcpPort(), dialogPort = await findFreeTcpPort();
const env = { ...process.env, MYALBUNS_PROCESS_GATE_DATA_ROOT: root,
  MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT: String(ownerPort),
  MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DEBUG_PORT: String(dialogPort),
  MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DATA_DIRECTORY: path.join(root, "dialog-webview") };
delete env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS;
delete env.TAURI_WEBVIEW_AUTOMATION;
const sockets = [], instances = [];
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const application = spawn(executable, [fixture], { windowsHide: true, stdio: "ignore", env });
instances.push(await waitForProcessInstance(application.pid, "dialog recovery Global"));
const records = () => {
  const directory = path.join(root, "Local", "MyAlbuns2", "Logs");
  return existsSync(directory) ? readdirSync(directory).flatMap(file => readFileSync(path.join(directory, file), "utf8")
    .split(/\r?\n/).flatMap(line => { try { return [JSON.parse(line)]; } catch { return []; } })) : [];
};
async function waitUntil(predicate, description, timeout = 45000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { if (await predicate()) return; await pause(50); }
  throw new Error(description);
}
async function socketAt(url) {
  const socket = new WebSocket(url); sockets.push(socket);
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  return socket;
}
async function connect(port, pathname) {
  let target;
  await waitUntil(async () => {
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(500) })).json();
      target = list.find(item => item.type === "page" && new URL(item.url).pathname === pathname);
    } catch {}
    return target;
  }, `The ${pathname} WebView must become reachable`);
  const socket = await socketAt(target.webSocketDebuggerUrl);
  let sequence = 0;
  const pending = new Map();
  socket.addEventListener("message", event => {
    const message = JSON.parse(event.data), item = pending.get(message.id);
    if (!item) return;
    pending.delete(message.id); clearTimeout(item.timer);
    if (message.error) item.reject(new Error(JSON.stringify(message.error))); else item.resolve(message.result);
  });
  socket.addEventListener("close", () => {
    for (const item of pending.values()) { clearTimeout(item.timer); item.reject(new Error("Browser disconnected")); }
    pending.clear();
  });
  const call = (method, params) => new Promise((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timed out: ${method}`)); }, 15000);
    pending.set(id, { resolve, reject, timer });
    socket.send(JSON.stringify({ id, method, params }));
  });
  return {
    call,
    async evaluate(expression) {
      const result = await call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
      if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
      return result.result.value;
    },
  };
}
const click = (client, label) => client.evaluate(`(() => {
  const button = [...document.querySelectorAll('button')].find(b => b.textContent.trim() === ${JSON.stringify(label)} || b.getAttribute('aria-label') === ${JSON.stringify(label)});
  if (!button || button.disabled) throw Error('Button unavailable: ' + ${JSON.stringify(label)});
  button.click();
})()`);
try {
  await waitUntil(() => records().some(row => row.event === "project_ui_ready"), "initial Project ready");
  const host = records().find(row => row.event === "project_host_started");
  const instance = await waitForProcessInstance(host.process_id, "dialog recovery Project"); instances.push(instance);
  let owner = await connect(ownerPort, "/");
  await waitUntil(() => owner.evaluate("Boolean(document.querySelector('canvas.pixi-canvas'))"), "initial canvas");
  const initial = await owner.evaluate("window.__TAURI_INTERNALS__.invoke('project_state', {operationId:'dialog-before'})");
  const dpi = initial.state.document.dpi === 301 ? 302 : 301;
  await owner.evaluate(`window.__TAURI_INTERNALS__.invoke('apply_project_intent', {intent:{kind:'setDpi',dpi:${dpi}},onProgress:'__CHANNEL__:'+window.__TAURI_INTERNALS__.transformCallback(()=>{})})`);
  const expected = await owner.evaluate("window.__TAURI_INTERNALS__.invoke('project_state', {operationId:'dialog-dirty'})");
  assert.equal(expected.state.dirty, true);
  assert.equal(expected.state.canUndo, true);
  if (mode === "close") await click(owner, "Fechar janela");
  else await click(owner, "Exportar");
  const dialog = await connect(dialogPort, "/project-dialog.html");
  const current = () => dialog.evaluate("window.__TAURI_INTERNALS__.invoke('current_project_dialog_presentation')");
  await waitUntil(async () => (await current())?.state.kind === (mode === "close" ? "projectCloseConfirmation" : "exportConfiguration"), "original dialog");
  const before = nativeOwnedWindowState(instance);
  assert.equal(before.owner.enabled, false);
  assert.equal(before.dialog.visible, true);
  const browser = captureProcessInstance(records().findLast(row => row.event === "webview_diagnostics_ready" && row.webview_label === "project").browser_process_id);
  assert.equal(browser.parentProcessId, host.process_id);
  assert.ok(browser.commandLine.includes(root));
  const version = await (await fetch(`http://127.0.0.1:${ownerPort}/json/version`)).json();
  const crashSocket = await socketAt(version.webSocketDebuggerUrl);
  if (mode === "progress") {
    const destination = path.join(output, "export"); mkdirSync(destination);
    await dialog.evaluate("document.querySelector('.export-configuration input').focus()");
    await dialog.call("Input.dispatchKeyEvent", { type: "keyDown", key: "a", code: "KeyA", modifiers: 2, windowsVirtualKeyCode: 65 });
    await dialog.call("Input.dispatchKeyEvent", { type: "keyUp", key: "a", code: "KeyA", modifiers: 2, windowsVirtualKeyCode: 65 });
    await dialog.call("Input.insertText", { text: destination });
    await click(dialog, "Exportar");
    await waitUntil(async () => (await current())?.state.kind === "exportProgress", "real Export progress");
  }
  const atFailure = await current();
  const recoveries = records().filter(row => row.event === "webview_recovery_ready" && row.webview_label === "project").length;
  crashSocket.send(JSON.stringify({ id: 1, method: "Browser.crash" }));
  await waitUntil(() => records().filter(row => row.event === "webview_recovery_ready" && row.webview_label === "project").length > recoveries, "editor recovered");
  owner = await connect(ownerPort, "/");
  await waitUntil(() => owner.evaluate("Boolean(document.querySelector('canvas.pixi-canvas'))"), "recovered canvas");
  const after = nativeOwnedWindowState(instance);
  writeFileSync(path.join(output, "observation.json"), JSON.stringify({ before, after, atFailure }, null, 2));
  assert.equal(after.dialogCount, 0, "A failed renderer must not strand its native dialog");
  const recoveredWindow = after.windows.find(window => window.hwnd === before.owner.hwnd);
  assert.ok(recoveredWindow?.enabled && recoveredWindow.visible, "The same Project HWND must respond again");
  assert.deepEqual(await owner.evaluate("window.__TAURI_INTERNALS__.invoke('project_state', {operationId:'dialog-recovered'})"), expected);
  await click(owner, "Exportar");
  const replacement = await connect(dialogPort, "/project-dialog.html");
  await waitUntil(() => replacement.evaluate("document.body.innerText.includes('Destino da exportação')"), "a new export can open");
  await waitUntil(() => nativeOwnedWindowState(instance).dialog?.visible, "the new dialog is visible to its user");
  await click(replacement, "Cancelar");
  await waitUntil(() => owner.evaluate("!document.querySelector('button[aria-label=\"Exportar\"]').disabled"), "new Cancel action finishes");
  assert.equal(nativeOwnedWindowState(instance).dialogCount, 0);
  const result = { passed: true, mode, before, after, atFailure, unsavedChangesPreserved: true,
    events: records().filter(row => row.event.startsWith("webview_") || row.event.includes("export") || row.event.includes("retired")) };
  writeFileSync(path.join(output, "result.json"), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ passed: true, mode, unsavedChangesPreserved: true, newDialogResponds: true }));
} finally {
  for (const socket of sockets) socket.close();
  for (const instance of processInstancesByExecutable(executable, "myalbuns-desktop.exe")) {
    if (instances.some(owner => owner.processId === instance.parentProcessId) && !instances.some(owner => owner.processId === instance.processId)) instances.push(instance);
  }
  for (const instance of instances.reverse()) terminateProcessInstance(instance);
}
