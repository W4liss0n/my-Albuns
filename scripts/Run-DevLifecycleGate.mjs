import { spawn } from "node:child_process";
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
  captureListeningProcessInstance,
  closeMainWindow,
  mergeProcessInstances,
  processForestInstances,
  processInstancesByExecutable,
  sameProcessInstance,
  sendCtrlC,
  startProcessInstanceInOwnConsole,
  terminateProcessInstance,
  waitForChildProcessClose,
  waitForProcessInstance,
} from "./DevLifecycleProcessInstances.mjs";

const [
  workspaceArgument,
  projectArgument,
  processDataArgument,
  screenshotArgument,
  applicationArgument,
  nativeDriverArgument,
] = process.argv.slice(2);

if (
  !workspaceArgument ||
  !projectArgument ||
  !processDataArgument ||
  !screenshotArgument ||
  !applicationArgument ||
  !nativeDriverArgument
) {
  throw new Error(
    "Usage: Run-DevLifecycleGate.mjs <workspace> <project> <process-data> <screenshot> <application> <native-driver>",
  );
}

const workspace = path.resolve(workspaceArgument);
const projectPath = path.resolve(projectArgument);
const processDataRoot = path.resolve(processDataArgument);
const screenshotPath = path.resolve(screenshotArgument);
const applicationPath = path.resolve(applicationArgument);
const nativeDriverPath = path.resolve(nativeDriverArgument);
const desktopBinary = path.join(
  workspace,
  "target",
  "debug",
  "myalbuns-desktop.exe",
);
const devUrl = "http://localhost:1437";
const gateTimeoutMilliseconds = Number(
  process.env.MYALBUNS_DEV_LIFECYCLE_GATE_TIMEOUT_MS ?? "300000",
);
const cleanupTimeoutMilliseconds = 60_000;

for (const [label, candidate] of [
  ["workspace", workspace],
  ["Project", projectPath],
  ["development supervisor", applicationPath],
  ["native WebDriver", nativeDriverPath],
]) {
  if (!existsSync(candidate)) {
    throw new Error(`${label} was not found: ${candidate}`);
  }
}
mkdirSync(path.dirname(screenshotPath), { recursive: true });
mkdirSync(processDataRoot, { recursive: true });

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

async function waitForHttp(url, timeoutMilliseconds, label) {
  const deadline = Date.now() + timeoutMilliseconds;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(1_000) });
      if (response.ok) return;
      lastError = new Error(`${label} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(100);
  }
  throw lastError ?? new Error(`${label} did not become ready`);
}

function webdriverClient(baseUrl) {
  return async (method, endpoint, body, timeout = 5_000) => {
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

function applicationProcesses() {
  if (!existsSync(desktopBinary)) return [];
  return processInstancesByExecutable(desktopBinary, "myalbuns-desktop.exe");
}

function frontendServerProcessInstance() {
  return captureListeningProcessInstance(1437);
}

function frontendServerProcessId() {
  return frontendServerProcessInstance()?.processId ?? null;
}

async function frontendResponds() {
  try {
    const response = await fetch(devUrl, {
      signal: AbortSignal.timeout(1_000),
    });
    return response.ok;
  } catch {
    return false;
  }
}

function desktopLogs() {
  const directory = path.join(processDataRoot, "Local", "MyAlbuns2", "Logs");
  if (!existsSync(directory)) return "";
  return readdirSync(directory)
    .filter((name) => /^myalbuns-(?:desktop|global)\..+\.jsonl$/.test(name))
    .map((name) => readFileSync(path.join(directory, name), "utf8"))
    .join("\n");
}

function hasLogEvent(event) {
  const logs = desktopLogs();
  return (
    logs.includes(`"event":"${event}"`) || logs.includes(`event="${event}"`)
  );
}

function captureDevelopmentForest(
  supervisorInstance,
  applications,
  knownRoots = [],
) {
  return processForestInstances([
    supervisorInstance,
    ...knownRoots,
    ...applications,
  ]);
}

function projectHostProcess(applications) {
  return applications.find((entry) =>
    entry.commandLine.includes("--myalbuns-project-host"),
  );
}

function globalProcess(applications) {
  return applications.find(
    (entry) => !entry.commandLine.includes("--myalbuns-project-host"),
  );
}

async function waitForOwnedDevelopmentEnvironment({
  label,
  supervisorInstance,
  timeoutMilliseconds,
  requireIndependentHost = false,
}) {
  let globalInstance;
  let hostInstance;
  let viteInstance;
  let observation;
  const deadline = Date.now() + timeoutMilliseconds;
  while (Date.now() < deadline) {
    const applications = applicationProcesses();
    globalInstance ??= globalProcess(applications);
    hostInstance ??= projectHostProcess(applications);
    viteInstance ??= frontendServerProcessInstance();
    const vitePid = viteInstance?.processId;
    const forestInstances = captureDevelopmentForest(
      supervisorInstance,
      applications,
      [globalInstance, hostInstance],
    );
    const forest = forestInstances.map((instance) => instance.processId);
    const hostForestInstances = processForestInstances([hostInstance]);
    const hostForest = hostForestInstances.map(
      (instance) => instance.processId,
    );
    const globalAlive = applications.some((entry) =>
      sameProcessInstance(entry, globalInstance),
    );
    const hostAlive = applications.some((entry) =>
      sameProcessInstance(entry, hostInstance),
    );
    const globalPid = globalInstance?.processId;
    const hostPid = hostInstance?.processId;
    observation = {
      supervisorPid: supervisorInstance.processId,
      globalPid,
      hostPid,
      vitePid,
      forest,
      hostForest,
      globalAlive,
      hostAlive,
      frontendResponding: await frontendResponds(),
    };
    const baseReady =
      viteInstance &&
      forest.includes(supervisorInstance.processId) &&
      forestInstances.some((entry) =>
        sameProcessInstance(entry, viteInstance),
      ) &&
      applications.length > 0 &&
      observation.frontendResponding;
    const handoffReady =
      globalInstance &&
      hostInstance &&
      !globalAlive &&
      hostAlive &&
      hostForest.includes(hostPid) &&
      hostForest.length >= 2;
    if (hostInstance && !hostAlive) {
      throw new Error(
        `${label} Project Host instance exited before the observed terminal`,
      );
    }
    if (baseReady && (!requireIndependentHost || handoffReady)) {
      return {
        ...observation,
        globalInstance,
        hostInstance,
        viteInstance,
        forestInstances,
      };
    }
    if (aliveProcessInstances([supervisorInstance]).length === 0) {
      throw new Error(
        `${label} supervisor exited during startup: ${JSON.stringify(observation)}`,
      );
    }
    await delay(100);
  }
  throw new Error(
    `${label} process forest did not become observable: ${JSON.stringify(observation)}`,
  );
}

async function assertDevelopmentCleanup(
  label,
  processForest,
  processForestInstances,
) {
  let observation;
  const deadline = Date.now() + cleanupTimeoutMilliseconds;
  while (Date.now() < deadline) {
    observation = {
      processForest,
      aliveProcessInstances: aliveProcessInstances(processForestInstances),
      applicationProcessIds: applicationProcesses().map(
        (entry) => entry.processId,
      ),
      frontendProcessId: frontendServerProcessId(),
      frontendResponding: await frontendResponds(),
    };
    if (
      observation.aliveProcessInstances.length === 0 &&
      observation.applicationProcessIds.length === 0 &&
      observation.frontendProcessId === null &&
      !observation.frontendResponding
    ) {
      return;
    }
    await delay(100);
  }
  throw new Error(
    `${label} left development descendants alive: ${JSON.stringify(observation)}`,
  );
}

if (frontendServerProcessId() !== null || (await frontendResponds())) {
  throw new Error(
    "The lifecycle gate requires port 1437 to be unowned before launch",
  );
}

const driverPort = await freePort();
const hostDebugPort = await freePort();

function supervisorEnvironment() {
  return {
    ...process.env,
    MYALBUNS_PROCESS_GATE_DATA_ROOT: processDataRoot,
    MYALBUNS_DEV_WORKSPACE_ROOT: workspace,
    MYALBUNS_DEV_PROJECT_PATH: projectPath,
    MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT: String(hostDebugPort),
  };
}

function launchSupervisor(launcherArguments = [], environment = {}) {
  return spawn(applicationPath, launcherArguments, {
    cwd: workspace,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...supervisorEnvironment(), ...environment },
  });
}

async function runSupervisorFailurePhase({
  label,
  launcherArguments = [],
  environment = {},
}) {
  const supervisor = launchSupervisor(launcherArguments, environment);
  const readOutput = collectOutput(supervisor);
  let authority;
  let terminal;
  let phaseError;
  let cleanupError;
  let processIds = [];
  let processInstances = [];

  const observeProcessForest = () => {
    if (!authority) return;
    const observedInstances = captureDevelopmentForest(
      authority,
      applicationProcesses(),
    );
    processIds = [
      ...new Set([
        ...processIds,
        ...observedInstances.map((instance) => instance.processId),
      ]),
    ];
    processInstances = mergeProcessInstances(
      processInstances,
      observedInstances,
    );
  };

  try {
    authority = await waitForProcessInstance(
      supervisor.pid,
      `${label} supervisor`,
    );
    observeProcessForest();
    terminal = {
      exitCode: await waitForChildProcessClose(
        supervisor,
        gateTimeoutMilliseconds,
        `${label} supervisor`,
        observeProcessForest,
      ),
    };
  } catch (error) {
    phaseError = error;
  } finally {
    try {
      observeProcessForest();
      if (authority) {
        terminateProcessInstance(authority);
      } else if (supervisor.exitCode === null) {
        supervisor.kill();
      }
      await waitForChildProcessClose(
        supervisor,
        cleanupTimeoutMilliseconds,
        `${label} supervisor cleanup`,
        observeProcessForest,
      );
      await assertDevelopmentCleanup(label, processIds, processInstances);
    } catch (error) {
      cleanupError = error;
    }
  }

  if (cleanupError) {
    throw new Error(
      `${label} cleanup failed: ${cleanupError instanceof Error ? cleanupError.message : String(cleanupError)}\n${readOutput()}`,
    );
  }

  if (phaseError) {
    throw new Error(
      `${label} failed: ${phaseError instanceof Error ? phaseError.message : String(phaseError)}\n${readOutput()}`,
    );
  }

  const output = readOutput();
  if (terminal.exitCode === 0) {
    throw new Error(`${label} supervisor unexpectedly exited successfully`);
  }
  if (!output.includes('"event":"dev_environment_cleanup_completed"')) {
    throw new Error(`${label} supervisor omitted its typed cleanup terminal`);
  }
  if (aliveProcessInstances([authority]).length !== 0) {
    throw new Error(
      `${label} supervisor authority remained alive after cleanup`,
    );
  }

  return {
    authority,
    output,
    terminal,
    processForest: {
      processIds,
      instances: processInstances,
    },
  };
}

function launchSupervisorInOwnConsole() {
  const standardOutputPath = path.join(
    processDataRoot,
    "ctrl-c-supervisor.out.log",
  );
  const standardErrorPath = path.join(
    processDataRoot,
    "ctrl-c-supervisor.err.log",
  );
  const authorityPath = path.join(
    processDataRoot,
    "ctrl-c-supervisor.instance.json",
  );
  const processInstance = startProcessInstanceInOwnConsole({
    executablePath: applicationPath,
    workingDirectory: workspace,
    standardOutputPath,
    standardErrorPath,
    authorityPath,
    environment: supervisorEnvironment(),
  });
  return {
    processInstance,
    output: () =>
      [standardOutputPath, standardErrorPath]
        .map((candidate) => {
          try {
            return existsSync(candidate) ? readFileSync(candidate, "utf8") : "";
          } catch (error) {
            return `<log unavailable: ${error instanceof Error ? error.message : String(error)}>`;
          }
        })
        .join("\n"),
  };
}

assertNoPreexistingProcessInstances(desktopBinary, "myalbuns-desktop.exe");

const supervisor = launchSupervisor();
const supervisorOutput = collectOutput(supervisor);
const supervisorPid = supervisor.pid;
const supervisorInstance = await waitForProcessInstance(
  supervisorPid,
  "Development supervisor",
);
let driver;
let driverInstance;
let driverOutput = () => "";
let abruptSupervisor;
let abruptSupervisorOutput = () => "";
let abruptSupervisorPid;
let abruptSupervisorInstance;
let ctrlCSupervisorOutput = () => "";
let ctrlCSupervisorPid;
let ctrlCSupervisorInstance;
let bootstrapFailure;
let containmentFailure;
let frontendFailureSupervisor;
let frontendFailureOutput = () => "";
let frontendFailureSupervisorPid;
let frontendFailureSupervisorInstance;
let sessionId;
let globalInstance;
let hostInstance;
let globalPid;
let hostPid;
let vitePid;
let viteInstance;

try {
  const handoffDeadline = Date.now() + gateTimeoutMilliseconds;
  while (Date.now() < handoffDeadline) {
    const processes = applicationProcesses();
    globalInstance ??= globalProcess(processes);
    hostInstance ??= projectHostProcess(processes);
    globalPid = globalInstance?.processId;
    hostPid = hostInstance?.processId;
    viteInstance ??= frontendServerProcessInstance();
    vitePid = viteInstance?.processId;
    const globalAlive = processes.some((entry) =>
      sameProcessInstance(entry, globalInstance),
    );
    const hostAlive = processes.some((entry) =>
      sameProcessInstance(entry, hostInstance),
    );
    if (supervisor.exitCode !== null) {
      throw new Error(
        `The supervisor exited before handoff (${supervisor.exitCode})`,
      );
    }
    if (hostPid && !hostAlive) {
      throw new Error(
        "The Project Host exited before the public UI-ready terminal",
      );
    }
    if (
      globalPid &&
      hostPid &&
      !globalAlive &&
      hostAlive &&
      hasLogEvent("host_ready") &&
      hasLogEvent("project_ui_ready") &&
      hasLogEvent("global_exited_after_project_handoff")
    ) {
      break;
    }
    await delay(100);
  }

  const postHandoffProcesses = applicationProcesses();
  const globalAlive = postHandoffProcesses.some((entry) =>
    sameProcessInstance(entry, globalInstance),
  );
  const hostAlive = postHandoffProcesses.some((entry) =>
    sameProcessInstance(entry, hostInstance),
  );
  const viteResponding = await frontendResponds();
  const supervisorAlive =
    aliveProcessInstances([supervisorInstance]).length === 1;
  if (
    !supervisorPid ||
    !globalPid ||
    !hostPid ||
    globalAlive ||
    !hostAlive ||
    !supervisorAlive ||
    !viteResponding
  ) {
    throw new Error(
      `Development handoff failed: ${JSON.stringify({ supervisorPid, globalPid, hostPid, globalAlive, hostAlive, supervisorAlive, vitePid, viteResponding })}`,
    );
  }

  await waitForHttp(
    `http://127.0.0.1:${hostDebugPort}/json/version`,
    30_000,
    "Project Host DevTools endpoint",
  );
  driver = spawn(
    nativeDriverPath,
    [`--port=${driverPort}`, "--host=127.0.0.1"],
    {
      cwd: workspace,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  driverOutput = collectOutput(driver);
  driverInstance = await waitForProcessInstance(driver.pid, "WebDriver");
  const driverBaseUrl = `http://127.0.0.1:${driverPort}`;
  await waitForHttp(
    `${driverBaseUrl}/status`,
    30_000,
    "Microsoft Edge WebDriver",
  );
  const request = webdriverClient(driverBaseUrl);
  const session = await request(
    "POST",
    "/session",
    {
      capabilities: {
        alwaysMatch: {
          browserName: "webview2",
          pageLoadStrategy: "none",
          "ms:edgeChromium": true,
          "ms:edgeOptions": {
            debuggerAddress: `127.0.0.1:${hostDebugPort}`,
          },
        },
      },
    },
    30_000,
  );
  sessionId = session.sessionId;
  if (!sessionId)
    throw new Error("Microsoft Edge WebDriver returned no session id");
  await request("POST", `/session/${sessionId}/timeouts`, {
    implicit: 250,
    pageLoad: 5_000,
    script: 5_000,
  });

  const uiDeadline = Date.now() + 30_000;
  let appShellElementId;
  let lastUiError;
  while (!appShellElementId && Date.now() < uiDeadline) {
    try {
      const element = await request("POST", `/session/${sessionId}/element`, {
        using: "css selector",
        value: ".app-shell",
      });
      appShellElementId = element["element-6066-11e4-a52e-4f735466cecf"];
    } catch (error) {
      lastUiError = error;
      await delay(100);
    }
  }
  if (!appShellElementId) {
    throw new Error(
      `Project UI did not render after the handoff: ${lastUiError ?? "no WebView"}`,
    );
  }

  const screenshot = await request(
    "GET",
    `/session/${sessionId}/element/${encodeURIComponent(appShellElementId)}/screenshot`,
    undefined,
    10_000,
  );
  if (typeof screenshot !== "string" || screenshot.length === 0) {
    throw new Error("The Project WebView returned no rendered screenshot");
  }
  writeFileSync(screenshotPath, Buffer.from(screenshot, "base64"));

  const normalApplications = applicationProcesses();
  const normalHostForestInstances = processForestInstances([hostInstance]);
  const normalHostForest = normalHostForestInstances.map(
    (instance) => instance.processId,
  );
  const normalTreeInstances = captureDevelopmentForest(
    supervisorInstance,
    normalApplications,
    [globalInstance, hostInstance],
  );
  const normalTree = normalTreeInstances.map((instance) => instance.processId);
  if (
    !normalTree.includes(supervisorPid) ||
    !normalTree.includes(hostPid) ||
    !normalTreeInstances.some((entry) =>
      sameProcessInstance(entry, viteInstance),
    ) ||
    normalHostForest.length < 2 ||
    !normalHostForest.every((processId) => normalTree.includes(processId))
  ) {
    throw new Error(
      `The normal lifecycle forest was incomplete before shutdown: ${JSON.stringify({ normalTree, normalHostForest, supervisorPid, globalPid, hostPid, vitePid })}`,
    );
  }
  // The attach-only WebDriver has already produced its evidence. Detach the
  // observer before exercising the native close terminal so the instrument is
  // no longer an active participant in the WebView being closed.
  if (driverInstance) {
    if (!driver || !terminateProcessInstance(driverInstance)) {
      throw new Error(
        "The exact WebDriver process instance was not terminable",
      );
    }
    await waitForChildProcessClose(driver, 10_000, "WebDriver");
    driver = undefined;
    driverInstance = undefined;
  }
  sessionId = undefined;
  closeMainWindow(hostInstance);
  await assertDevelopmentCleanup(
    "Normal Host close",
    normalTree,
    normalTreeInstances,
  );
  if (
    !supervisorOutput().includes('"event":"dev_environment_cleanup_completed"')
  ) {
    throw new Error(
      "The supervisor exited without confirming frontend cleanup",
    );
  }
  abruptSupervisor = launchSupervisor();
  abruptSupervisorOutput = collectOutput(abruptSupervisor);
  abruptSupervisorPid = abruptSupervisor.pid;
  abruptSupervisorInstance = await waitForProcessInstance(
    abruptSupervisorPid,
    "Abrupt-cleanup supervisor",
  );
  const abruptEnvironment = await waitForOwnedDevelopmentEnvironment({
    label: "Abrupt-cleanup",
    supervisorInstance: abruptSupervisorInstance,
    timeoutMilliseconds: gateTimeoutMilliseconds,
  });
  const abruptTree = abruptEnvironment.forest;
  const abruptTreeInstances = abruptEnvironment.forestInstances;
  if (!terminateProcessInstance(abruptSupervisorInstance)) {
    throw new Error(
      "Windows refused to terminate only the development supervisor root",
    );
  }
  await assertDevelopmentCleanup(
    "Abrupt supervisor termination",
    abruptTree,
    abruptTreeInstances,
  );

  const ctrlCLaunch = launchSupervisorInOwnConsole();
  ctrlCSupervisorOutput = ctrlCLaunch.output;
  ctrlCSupervisorInstance = ctrlCLaunch.processInstance;
  ctrlCSupervisorPid = ctrlCSupervisorInstance.processId;
  const ctrlCEnvironment = await waitForOwnedDevelopmentEnvironment({
    label: "CTRL+C",
    supervisorInstance: ctrlCSupervisorInstance,
    timeoutMilliseconds: Math.min(gateTimeoutMilliseconds, 60_000),
    requireIndependentHost: true,
  });
  const ctrlCTree = ctrlCEnvironment.forest;
  const ctrlCTreeInstances = ctrlCEnvironment.forestInstances;
  const ctrlCHostForest = ctrlCEnvironment.hostForest;
  sendCtrlC(ctrlCSupervisorInstance);
  await assertDevelopmentCleanup("CTRL+C", ctrlCTree, ctrlCTreeInstances);

  bootstrapFailure = await runSupervisorFailurePhase({
    label: "Bootstrap failure",
    launcherArguments: ["--myalbuns-invalid-development-option"],
  });
  const bootstrapFailureTerminalObserved = bootstrapFailure.output.includes(
    '"event":"dev_frontend_ready"',
  );
  if (!bootstrapFailureTerminalObserved) {
    throw new Error(
      `Bootstrap failure did not clean its environment: ${JSON.stringify({ exitCode: bootstrapFailure.terminal.exitCode, supervisorPid: bootstrapFailure.authority.processId })}`,
    );
  }

  containmentFailure = await runSupervisorFailurePhase({
    label: "Descendant containment failure",
    environment: {
      MYALBUNS_DEV_DESCENDANT_JOB_FAILURE_PROBE: "1",
    },
  });
  const containmentFailureTerminalObserved =
    containmentFailure.output.includes(
      '"event":"desktop_start_failed","stage":"initialize","code":"dev_descendant_job_install_failed"',
    ) &&
    !containmentFailure.output.includes('"event":"dev_global_only_exited"');
  if (!containmentFailureTerminalObserved) {
    throw new Error(
      `Containment failure did not fail closed: ${JSON.stringify({ exitCode: containmentFailure.terminal.exitCode, supervisorPid: containmentFailure.authority.processId })}`,
    );
  }

  frontendFailureSupervisor = launchSupervisor();
  frontendFailureOutput = collectOutput(frontendFailureSupervisor);
  frontendFailureSupervisorPid = frontendFailureSupervisor.pid;
  frontendFailureSupervisorInstance = await waitForProcessInstance(
    frontendFailureSupervisorPid,
    "Frontend-failure supervisor",
  );
  const frontendFailureEnvironment = await waitForOwnedDevelopmentEnvironment({
    label: "Frontend-failure",
    supervisorInstance: frontendFailureSupervisorInstance,
    timeoutMilliseconds: gateTimeoutMilliseconds,
  });
  const failedViteInstance = frontendFailureEnvironment.viteInstance;
  const frontendFailureTree = frontendFailureEnvironment.forest;
  const frontendFailureTreeInstances =
    frontendFailureEnvironment.forestInstances;
  if (!terminateProcessInstance(failedViteInstance)) {
    throw new Error("The exact Vite process instance was not terminable");
  }
  const frontendFailureExit = await waitForChildProcessClose(
    frontendFailureSupervisor,
    30_000,
    "Frontend-failure supervisor",
  );
  await assertDevelopmentCleanup(
    "Frontend failure",
    frontendFailureTree,
    frontendFailureTreeInstances,
  );
  if (
    frontendFailureExit === 0 ||
    !frontendFailureOutput().includes(
      '"event":"dev_environment_cleanup_completed"',
    )
  ) {
    throw new Error(
      `Frontend failure returned the wrong terminal: ${JSON.stringify({ frontendFailureExit, frontendFailureTree })}`,
    );
  }

  console.log(
    JSON.stringify({
      globalPid,
      hostPid,
      supervisorPid,
      vitePid,
      webdriverMode: "attach-existing-webview2",
      globalExitedAfterUiReady: true,
      hostSurvivedGlobal: true,
      viteSurvivedGlobal: true,
      projectUiRendered: true,
      screenshotPath,
      cleanupCompleted: true,
      cleanupLogged: true,
      normalTreeProcessCount: normalTree.length,
      normalHostTreeProcessCount: normalHostForest.length,
      abruptCleanupCompleted: true,
      abruptTreeProcessCount: abruptTree.length,
      ctrlCCleanupCompleted: true,
      ctrlCTreeProcessCount: ctrlCTree.length,
      ctrlCHostTreeProcessCount: ctrlCHostForest.length,
      bootstrapFailureCleanupCompleted: true,
      bootstrapFailureTerminalObserved,
      bootstrapFailureTreeProcessCount:
        bootstrapFailure.processForest.processIds.length,
      containmentFailureCleanupCompleted: true,
      containmentFailureTerminalObserved,
      containmentFailureTreeProcessCount:
        containmentFailure.processForest.processIds.length,
      frontendFailureCleanupCompleted: true,
    }),
  );
} catch (error) {
  throw new Error(
    `${error instanceof Error ? error.message : String(error)}\nSupervisor:\n${supervisorOutput()}\nAbrupt supervisor:\n${abruptSupervisorOutput()}\nCTRL+C supervisor:\n${ctrlCSupervisorOutput()}\nBootstrap-failure supervisor:\n${bootstrapFailure?.output ?? ""}\nContainment-failure supervisor:\n${containmentFailure?.output ?? ""}\nFrontend-failure supervisor:\n${frontendFailureOutput()}\nWebDriver:\n${driverOutput()}\nLogs:\n${desktopLogs().slice(-12_000)}`,
  );
} finally {
  terminateProcessInstance(driverInstance);
  terminateProcessInstance(frontendFailureSupervisorInstance);
  terminateProcessInstance(ctrlCSupervisorInstance);
  terminateProcessInstance(abruptSupervisorInstance);
  terminateProcessInstance(supervisorInstance);
}
