import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  aliveProcessInstances, assertNoPreexistingProcessInstances, sameProcessInstance,
  terminateProcessInstance, waitForProcessInstance,
} from "./DevLifecycleProcessInstances.mjs";
import {
  attachWebView2Driver, disposeConfirmedWebDriver, findFreeTcpPortInRange,
  switchToWebDriverWindow,
} from "./GateWebDriver.mjs";
import { nativeOwnedWindowState } from "./NativeWindowObservation.mjs";
import { createNativeGateRuntime, readProjectInteractionState } from "./NativeGateRuntime.mjs";
import { createProjectCloseEnvironment } from "./ProjectCloseGateEnvironment.mjs";
import { assertProjectCloseEvidence, PROJECT_CLOSE_SCENARIO, summarizeCloseStages } from "./ProjectCloseGateEvidence.mjs";

if (process.env.MYALBUNS_NATIVE_GATE_AUTHORIZED !== "1") {
  throw new Error("Use Test-ProjectCloseGate.ps1 with the native execution policy; direct launches are disabled.");
}
const [workspaceArg, scratchArg, applicationArg, driverArg, scenario] = process.argv.slice(2);
if (![workspaceArg, scratchArg, applicationArg, driverArg].every(Boolean) || scenario !== PROJECT_CLOSE_SCENARIO) {
  throw new Error("Usage: Run-ProjectCloseGate.mjs <workspace> <scratch> <application> <driver> saved-original-close");
}
const [workspace, scratch, applicationPath, nativeDriverPath] =
  [workspaceArg, scratchArg, applicationArg, driverArg].map((value) => path.resolve(value));
const fixture = JSON.parse(readFileSync(path.join(scratch, "focused-owned-dialog-fixture.json"), "utf8"));
const projectPath = path.resolve(fixture.originalPath);
const copyPath = path.join(scratch, "Cópia do fechamento.myalbuns");
if (path.dirname(projectPath) !== scratch || existsSync(copyPath)) {
  throw new Error("The close scenario requires its own fresh fixture inside the scratch root.");
}
const timeout = Number(process.env.MYALBUNS_PROJECT_CLOSE_TIMEOUT_MS ?? 60_000);
if (!Number.isSafeInteger(timeout) || timeout < 1_000 || timeout > 180_000) throw new Error("Invalid close gate timeout");
const runtime = createNativeGateRuntime({
  applicationPath, defaultTimeoutMilliseconds: timeout, processDataRoot: path.join(scratch, "process-data"),
  workspace, nativeDialogDriver: path.join(workspace, "scripts", "Drive-NativeSaveDialog.ps1"),
});
const { applicationProcesses, collectChildOutput, driveNativeDialog, recordsFor, logRecords, waitFor, waitForExit, waitForNewApplication } = runtime;
const drivers = new Map();
const outputs = new Map();
const steps = [];
let originalHost;
let copyHost;
let originalSession;
let copySession;
let requestedAtUtc;
let evidence;
let projectClose;
let scenarioFailure;
let cleanupFailure;
const isHost = (instance) => instance.commandLine.includes("--myalbuns-project-host");
const writeJson = (name, value) => writeFileSync(path.join(scratch, name), JSON.stringify(value, null, 2) + "\n");

async function step(name, operation) {
  const entry = { name, startedAtUtc: new Date().toISOString(), status: "running" };
  steps.push(entry);
  writeJson("project-close-progress.json", { scenario, steps });
  try {
    const result = await operation();
    entry.status = "passed";
    return result;
  } catch (error) {
    entry.status = "failed";
    entry.error = String(error);
    throw error;
  } finally {
    entry.finishedAtUtc = new Date().toISOString();
    writeJson("project-close-progress.json", { scenario, steps });
  }
}

async function attach(debugPort, label, dialogPort) {
  const driver = await attachWebView2Driver({
    debugPort, label, projectDialogDebugPort: dialogPort, nativeDriverPath,
    driverLogPath: path.join(scratch, `webdriver-${label}.log`), workingDirectory: workspace,
    sessionTimeoutMilliseconds: Math.min(timeout, 30_000),
  });
  drivers.set(label, driver);
  return driver;
}

async function dispose(label) {
  const driver = drivers.get(label);
  if (!driver) return;
  await disposeConfirmedWebDriver(driver);
  drivers.delete(label);
}

async function find(driver, using, value) {
  return waitFor(value, async () => {
    const element = await driver.request("POST", `/session/${driver.sessionId}/element`, { using, value });
    return element["element-6066-11e4-a52e-4f735466cecf"];
  });
}
const endpoint = (driver, element) => `/session/${driver.sessionId}/element/${encodeURIComponent(element)}`;
async function clickElement(driver, element) {
  await waitFor("command enabled", () => driver.request("GET", `${endpoint(driver, element)}/enabled`));
  // Each user action is sent once. Poll readiness, never repeat a possibly completed action.
  await driver.request("POST", `${endpoint(driver, element)}/click`, {});
}
async function click(driver, using, value) { await clickElement(driver, await find(driver, using, value)); }
async function openFileMenu(driver) {
  const menu = await find(driver, "xpath", "//nav[@aria-label='Menu principal']//button[normalize-space()='Arquivo']");
  if (await driver.request("GET", `${endpoint(driver, menu)}/attribute/aria-expanded`) !== "true") await clickElement(driver, menu);
  return menu;
}
async function command(driver, label) {
  await openFileMenu(driver);
  await click(driver, "xpath", `//*[@role='menu' and @aria-label='Arquivo']//button[@aria-label='${label}']`);
}
async function dpiInput(driver) {
  const section = await find(driver, "css selector", "button[aria-label='Informações do Álbum']");
  if (await driver.request("GET", `${endpoint(driver, section)}/attribute/aria-expanded`) !== "true") await clickElement(driver, section);
  return find(driver, "css selector", "input[aria-label='DPI']");
}
async function readDpi(driver) {
  return driver.request("GET", `${endpoint(driver, await dpiInput(driver))}/attribute/value`);
}
async function changeDpi(session, dpi, revision) {
  const input = await dpiInput(session.driver);
  await session.driver.request("POST", `${endpoint(session.driver, input)}/clear`, {});
  await session.driver.request("POST", `${endpoint(session.driver, input)}/value`, { text: String(dpi), value: [...String(dpi)] });
  await click(session.driver, "css selector", "button[form='album-information-settings']");
  const dialogLabel = `${session.label}-apply`;
  const dialog = await attach(session.dialogPort, dialogLabel);
  try {
    await switchToWebDriverWindow(dialog, (url) => new URL(url).pathname.endsWith("/project-dialog.html"), "Album confirmation");
    await click(dialog, "xpath", "//*[@role='dialog' and @aria-labelledby = //*[normalize-space()='Aplicar alterações no Álbum?']/@id]//button[normalize-space()='Aplicar']");
  } finally { await dispose(dialogLabel); }
  await waitFor("exact Host mutation", () => recordsFor("project_intent_applied").find((record) =>
    Number(record.process_id) === session.host.processId && Number(record.revision) === revision,
  ));
}

function savedFile(filePath) {
  const bytes = readFileSync(filePath);
  const document = JSON.parse(bytes.toString("utf8"));
  return { projectId: document.projectId, revision: document.revision, dpi: document.project.document.dpi,
    sha256: createHash("sha256").update(bytes).digest("hex") };
}
async function save(session, filePath, revision) {
  await command(session.driver, "Salvar");
  const projectId = savedFile(filePath).projectId;
  await waitFor("exact identity save", () => recordsFor("project_save_completed").find((record) =>
    record.project_id === projectId && Number(record.revision) === revision,
  ));
  return savedFile(filePath);
}

async function openOriginal(label, knownHosts) {
  const globalPort = await findFreeTcpPortInRange(40_000, 44_999);
  const hostPort = await findFreeTcpPortInRange(40_000, 44_999);
  const dialogPort = await findFreeTcpPortInRange(40_000, 44_999);
  const saveAsPort = await findFreeTcpPortInRange(40_000, 44_999);
  const environment = createProjectCloseEnvironment(process.env, {
    scratch, label, globalPort, hostPort, dialogPort, saveAsPort,
  });
  const child = spawn(applicationPath, [projectPath], { cwd: workspace, windowsHide: true, stdio: ["ignore", "pipe", "pipe"], env: environment });
  outputs.set(label, collectChildOutput(child));
  const global = await waitForProcessInstance(child.pid, `${label} Global`);
  try {
    const host = await waitForNewApplication(isHost, knownHosts, `${label} Host`);
    await waitFor(`${label} productive UI`, () => recordsFor("project_ui_ready").some((record) => Number(record.process_id) === host.processId));
    await waitForExit(global, `${label} Global handoff`);
    const driver = await attach(hostPort, label, dialogPort);
    await find(driver, "css selector", "canvas.pixi-canvas");
    return { label, host, driver, global, globalPort, dialogPort, saveAsPort };
  } catch (error) {
    // Safe mode may prevent Host UI readiness. Retain the opening page when reachable.
    for (const [port, surface] of [[globalPort, "global"], [hostPort, "host"]]) {
      try { await attach(port, `${label}-startup-${surface}`); }
      catch { /* Driver traces and process logs remain available. */ }
    }
    throw error;
  }
}

async function retainFailure(error) {
  const diagnostics = { error: String(error), steps, requestedAtUtc, originalHost, copyHost, evidence,
    records: logRecords(), processes: applicationProcesses(), pages: {}, nativeWindows: [],
    childOutput: Object.fromEntries([...outputs].map(([name, get]) => [name, get()])),
  };
  if (requestedAtUtc) diagnostics.closeStages = summarizeCloseStages(diagnostics.records, originalHost, requestedAtUtc);
  for (const instance of diagnostics.processes) {
    try { diagnostics.nativeWindows.push({ instance, state: nativeOwnedWindowState(instance) }); }
    catch (failure) { diagnostics.nativeWindows.push({ instance, observationError: String(failure) }); }
  }
  for (const [label, driver] of drivers) {
    const page = {};
    diagnostics.pages[label] = page;
    try { page.interaction = await readProjectInteractionState(driver); }
    catch (failure) { page.interactionError = String(failure); }
    try {
      page.document = await driver.request("POST", `/session/${driver.sessionId}/execute/sync`, {
        script: "return {title: document.title, url: location.href, text: document.body?.innerText ?? ''};", args: [],
      }, 5_000);
    } catch (failure) { page.documentError = String(failure); }
    try {
      const screenshot = await driver.request("GET", `/session/${driver.sessionId}/screenshot`, undefined, 5_000);
      writeFileSync(path.join(scratch, `failure-${label}.png`), Buffer.from(screenshot, "base64"));
    } catch (failure) { page.screenshotError = String(failure); }
  }
  writeJson("failure-project-close.json", diagnostics);
}

assertNoPreexistingProcessInstances(applicationPath, path.basename(applicationPath));
assertNoPreexistingProcessInstances(nativeDriverPath, path.basename(nativeDriverPath));
try {
  copySession = await step("open-initial-original", () => openOriginal("copy", []));
  copyHost = copySession.host;
  const baseRevision = Number(fixture.sourceRevision);
  await step("change-before-save-as", () => changeDpi(copySession, 360, baseRevision + 1));
  await step("save-as-independent-copy", async () => {
    await command(copySession.driver, "Salvar como…");
    driveNativeDialog(copyHost, "select", "Salvar Projeto como", copyPath);
    await waitFor("Save As adoption", () => existsSync(copyPath) && recordsFor("project_save_as_completed").find((record) =>
      record.project_id === savedFile(copyPath).projectId && Number(record.revision) === baseRevision + 1,
    ));
    await dispose("copy");
    copySession.driver = await attach(copySession.saveAsPort, "copy", copySession.dialogPort);
    await find(copySession.driver, "css selector", "canvas.pixi-canvas");
  });
  originalSession = await step("reopen-original", () => openOriginal("original", [copyHost]));
  originalHost = originalSession.host;
  await step("change-original", () => changeDpi(originalSession, 320, baseRevision + 1));
  const originalSaved = await step("save-original", () => save(originalSession, projectPath, baseRevision + 1));
  await step("change-copy", () => changeDpi(copySession, 420, baseRevision + 2));
  const copySaved = await step("save-copy", () => save(copySession, copyPath, baseRevision + 2));
  evidence = { originalHost, copyHost, before: { original: originalSaved, copy: copySaved } };
  await step("close-original-once", async () => {
    requestedAtUtc = new Date().toISOString();
    evidence.requestedAtUtc = requestedAtUtc;
    writeJson("project-close-before.json", { ...evidence, originalUi: await readProjectInteractionState(originalSession.driver) });
    await command(originalSession.driver, "Fechar Projeto");
    await waitFor("original clean close terminal", () => recordsFor("clean_project_close_requested").find((record) =>
      Number(record.process_id) === originalHost.processId && record.timestamp >= requestedAtUtc,
    ));
    await waitForExit(originalHost, "original Host exit before cleanup");
  });
  await step("verify-copy-and-global-before-cleanup", async () => {
    const replacement = await waitForNewApplication((instance) =>
      !isHost(instance) && instance.parentProcessId === originalHost.processId && instance.creationTimeUtc >= requestedAtUtc,
      [copySession.global, originalSession.global], "replacement Global");
    await waitFor("replacement Global ready", () => nativeOwnedWindowState(replacement).windows.some((window) =>
      window.ownerHwnd === 0 && window.visible && window.enabled,
    ));
    const menu = await openFileMenu(copySession.driver);
    await find(copySession.driver, "xpath", "//*[@role='menu' and @aria-label='Arquivo']//button[@aria-label='Salvar']");
    await clickElement(copySession.driver, menu);
    evidence.after = { original: savedFile(projectPath), copy: savedFile(copyPath) };
    evidence.aliveBeforeCleanup = aliveProcessInstances([originalHost, copyHost]);
    evidence.originalWindowsGone = !evidence.aliveBeforeCleanup.some((item) => sameProcessInstance(item, originalHost));
    evidence.replacementGlobal = replacement;
    evidence.replacementGlobalReady = true;
    evidence.copyUi = { dpi: await readDpi(copySession.driver), state: await readProjectInteractionState(copySession.driver) };
    evidence.records = logRecords();
    projectClose = assertProjectCloseEvidence(evidence);
    writeJson("project-close-observations.json", { ...evidence, projectClose, steps });
  });
} catch (error) {
  scenarioFailure = error;
  try { await retainFailure(error); }
  catch (diagnosticError) { console.error(`Failure diagnostics incomplete: ${diagnosticError}`); }
} finally {
  for (const label of [...drivers.keys()]) {
    try { await dispose(label); } catch (error) { cleanupFailure ??= error; }
  }
  try {
    for (const instance of applicationProcesses()) terminateProcessInstance(instance);
    await waitFor("close gate process cleanup", () => applicationProcesses().length === 0);
  } catch (error) { cleanupFailure ??= error; }
  if (existsSync(fixture.externalCopyPath)) {
    spawnSync("attrib.exe", ["-R", fixture.externalCopyPath], { windowsHide: true, stdio: "ignore" });
  }
}
if (scenarioFailure) throw scenarioFailure;
if (cleanupFailure) throw cleanupFailure;
console.log(JSON.stringify({ schemaVersion: 1, gate: PROJECT_CLOSE_SCENARIO, scenarios: [PROJECT_CLOSE_SCENARIO], projectClose,
  cleanupCompleted: applicationProcesses().length === 0 }));
