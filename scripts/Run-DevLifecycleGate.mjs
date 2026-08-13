import { spawn, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import net from "node:net";
import path from "node:path";

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

async function waitForProcessExit(
  child,
  timeoutMilliseconds,
  label,
  observe = () => {},
) {
  const deadline = Date.now() + timeoutMilliseconds;
  while (Date.now() < deadline) {
    observe();
    if (child.exitCode !== null) return child.exitCode;
    await delay(50);
  }
  throw new Error(`${label} did not exit within ${timeoutMilliseconds} ms`);
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

function powershellJson(script, environment = {}) {
  const result = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", script],
    {
      windowsHide: true,
      encoding: "utf8",
      env: { ...process.env, ...environment },
    },
  );
  if (result.status !== 0) {
    throw new Error(`Windows process observation failed: ${result.stderr}`);
  }
  return result.stdout.trim() ? JSON.parse(result.stdout) : null;
}

function applicationProcesses() {
  if (!existsSync(desktopBinary)) return [];
  return (
    powershellJson(
      `$items = @(Get-CimInstance Win32_Process -Filter "Name = 'myalbuns-desktop.exe'" -ErrorAction Stop | Where-Object { [StringComparer]::OrdinalIgnoreCase.Equals($_.ExecutablePath, $env:MYALBUNS_GATE_DESKTOP_BINARY) } | ForEach-Object { [ordered]@{ processId = [int]$_.ProcessId; parentProcessId = [int]$_.ParentProcessId; commandLine = [string]$_.CommandLine } }); [Console]::Out.Write((ConvertTo-Json -InputObject $items -Compress))`,
      { MYALBUNS_GATE_DESKTOP_BINARY: desktopBinary },
    ) ?? []
  );
}

function supervisorProcesses() {
  return (
    powershellJson(
      `$items = @(Get-CimInstance Win32_Process -Filter "Name = 'myalbuns-dev.exe'" -ErrorAction Stop | Where-Object { [StringComparer]::OrdinalIgnoreCase.Equals($_.ExecutablePath, $env:MYALBUNS_GATE_SUPERVISOR_BINARY) } | ForEach-Object { [ordered]@{ processId = [int]$_.ProcessId; parentProcessId = [int]$_.ParentProcessId; commandLine = [string]$_.CommandLine } }); [Console]::Out.Write((ConvertTo-Json -InputObject $items -Compress))`,
      { MYALBUNS_GATE_SUPERVISOR_BINARY: applicationPath },
    ) ?? []
  );
}

function frontendServerProcessId() {
  return powershellJson(
    `$listener = Get-NetTCPConnection -LocalPort 1437 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1; if ($null -eq $listener) { [Console]::Out.Write('null') } else { [Console]::Out.Write(([int]$listener.OwningProcess | ConvertTo-Json -Compress)) }`,
  );
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

function terminateProcessTree(processId) {
  if (!Number.isInteger(processId) || processId <= 0) return;
  spawnSync("taskkill.exe", ["/PID", String(processId), "/T", "/F"], {
    windowsHide: true,
    stdio: "ignore",
  });
}

function processForestIds(rootProcessIds) {
  const roots = [
    ...new Set(
      rootProcessIds.filter(
        (processId) => Number.isInteger(processId) && processId > 0,
      ),
    ),
  ];
  if (roots.length === 0) return [];
  return (
    powershellJson(
      `$all = @(Get-CimInstance Win32_Process -ErrorAction Stop); $ids = @($env:MYALBUNS_GATE_ROOT_PROCESS_IDS -split ',' | ForEach-Object { [int]$_ }); do { $before = $ids.Count; $parents = $ids; $ids += @($all | Where-Object { $parents -contains [int]$_.ParentProcessId } | ForEach-Object { [int]$_.ProcessId }); $ids = @($ids | Sort-Object -Unique) } while ($ids.Count -gt $before); [Console]::Out.Write((ConvertTo-Json -InputObject $ids -Compress))`,
      { MYALBUNS_GATE_ROOT_PROCESS_IDS: roots.join(",") },
    ) ?? []
  );
}

function captureDevelopmentForest(
  supervisorPid,
  applications,
  knownRoots = [],
) {
  return processForestIds([
    supervisorPid,
    ...knownRoots,
    ...applications.map((entry) => entry.processId),
  ]);
}

function processInstances(processIds) {
  if (processIds.length === 0) return [];
  return (
    powershellJson(
      `$requested = @($env:MYALBUNS_GATE_PROCESS_IDS -split ',' | ForEach-Object { [int]$_ }); $instances = @(Get-CimInstance Win32_Process -ErrorAction Stop | Where-Object { $requested -contains [int]$_.ProcessId } | ForEach-Object { [ordered]@{ processId = [int]$_.ProcessId; creationTimeUtc = $_.CreationDate.ToUniversalTime().ToString('O'); name = [string]$_.Name; commandLine = [string]$_.CommandLine } }); [Console]::Out.Write((ConvertTo-Json -InputObject $instances -Compress))`,
      { MYALBUNS_GATE_PROCESS_IDS: processIds.join(",") },
    ) ?? []
  );
}

function processInstanceKey(instance) {
  return `${instance.processId}:${instance.creationTimeUtc}`;
}

function mergeProcessInstances(...collections) {
  return [
    ...new Map(
      collections
        .flat()
        .map((instance) => [processInstanceKey(instance), instance]),
    ).values(),
  ];
}

function aliveProcessInstances(expectedInstances) {
  const currentByKey = new Map(
    processInstances(
      expectedInstances.map((instance) => instance.processId),
    ).map((instance) => [processInstanceKey(instance), instance]),
  );
  return expectedInstances.filter((instance) =>
    currentByKey.has(processInstanceKey(instance)),
  );
}

function aliveProcessIds(processIds) {
  return aliveProcessInstances(processInstances(processIds)).map(
    (instance) => instance.processId,
  );
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
  supervisorPid,
  timeoutMilliseconds,
  requireIndependentHost = false,
}) {
  let globalPid;
  let hostPid;
  let vitePid;
  let observation;
  const deadline = Date.now() + timeoutMilliseconds;
  while (Date.now() < deadline) {
    const applications = applicationProcesses();
    globalPid ??= globalProcess(applications)?.processId;
    hostPid ??= projectHostProcess(applications)?.processId;
    vitePid ??= frontendServerProcessId();
    const forest = captureDevelopmentForest(supervisorPid, applications, [
      globalPid,
      hostPid,
    ]);
    const hostForest = processForestIds([hostPid]);
    const globalAlive = applications.some(
      (entry) => entry.processId === globalPid,
    );
    const hostAlive = applications.some((entry) => entry.processId === hostPid);
    const frontendResponding = await frontendResponds();
    observation = {
      supervisorPid,
      globalPid,
      hostPid,
      vitePid,
      forest,
      hostForest,
      globalAlive,
      hostAlive,
      frontendResponding,
    };
    const baseReady =
      vitePid &&
      forest.includes(supervisorPid) &&
      forest.includes(vitePid) &&
      applications.length > 0 &&
      frontendResponding;
    const handoffReady =
      globalPid &&
      hostPid &&
      !globalAlive &&
      hostAlive &&
      hostForest.includes(hostPid) &&
      hostForest.length >= 2;
    if (baseReady && (!requireIndependentHost || handoffReady)) {
      return {
        ...observation,
        forestInstances: processInstances(forest),
      };
    }
    if (aliveProcessIds([supervisorPid]).length === 0) {
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
  const deadline = Date.now() + 30_000;
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

function closeMainWindow(processId) {
  const closed = powershellJson(
    `$process = Get-Process -Id ([int]$env:MYALBUNS_GATE_PROCESS_ID) -ErrorAction Stop; [Console]::Out.Write(($process.CloseMainWindow() | ConvertTo-Json -Compress))`,
    { MYALBUNS_GATE_PROCESS_ID: String(processId) },
  );
  if (closed !== true) {
    throw new Error(
      `The Project Host exposed no closeable native window (${processId})`,
    );
  }
}

function sendCtrlC(processId) {
  const script = String.raw`
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class MyAlbunsConsoleSignal {
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool FreeConsole();

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool AttachConsole(uint processId);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool SetConsoleCtrlHandler(IntPtr handler, bool add);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool GenerateConsoleCtrlEvent(uint eventType, uint processGroupId);
}
'@

[void][MyAlbunsConsoleSignal]::FreeConsole()
if (-not [MyAlbunsConsoleSignal]::AttachConsole([uint32]$env:MYALBUNS_GATE_CTRL_C_PID)) {
    throw "AttachConsole failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
}
try {
    if (-not [MyAlbunsConsoleSignal]::SetConsoleCtrlHandler([IntPtr]::Zero, $true)) {
        throw "SetConsoleCtrlHandler failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
    }
    if (-not [MyAlbunsConsoleSignal]::GenerateConsoleCtrlEvent(0, 0)) {
        throw "GenerateConsoleCtrlEvent failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
    }
}
finally {
    [void][MyAlbunsConsoleSignal]::FreeConsole()
}
`;
  const result = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", script],
    {
      windowsHide: true,
      encoding: "utf8",
      env: {
        ...process.env,
        MYALBUNS_GATE_CTRL_C_PID: String(processId),
      },
    },
  );
  if (result.status !== 0) {
    throw new Error(
      `CTRL+C delivery failed: ${result.stderr || result.stdout}`,
    );
  }
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

function launchSupervisor(launcherArguments = []) {
  return spawn(applicationPath, launcherArguments, {
    cwd: workspace,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
    env: supervisorEnvironment(),
  });
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
  const processIdPath = path.join(processDataRoot, "ctrl-c-supervisor.pid");
  const launchResult = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `$process = Start-Process -FilePath $env:MYALBUNS_GATE_SUPERVISOR_BINARY -WorkingDirectory $env:MYALBUNS_DEV_WORKSPACE_ROOT -PassThru -WindowStyle Hidden -RedirectStandardOutput $env:MYALBUNS_GATE_STDOUT -RedirectStandardError $env:MYALBUNS_GATE_STDERR; [IO.File]::WriteAllText($env:MYALBUNS_GATE_PROCESS_ID_PATH, [string]$process.Id)`,
    ],
    {
      windowsHide: true,
      stdio: "ignore",
      env: {
        ...supervisorEnvironment(),
        MYALBUNS_GATE_SUPERVISOR_BINARY: applicationPath,
        MYALBUNS_GATE_STDOUT: standardOutputPath,
        MYALBUNS_GATE_STDERR: standardErrorPath,
        MYALBUNS_GATE_PROCESS_ID_PATH: processIdPath,
      },
    },
  );
  if (launchResult.status !== 0 || !existsSync(processIdPath)) {
    throw new Error("PowerShell could not start the CTRL+C supervisor console");
  }
  const processId = Number(readFileSync(processIdPath, "utf8").trim());
  if (!Number.isInteger(processId) || processId <= 0) {
    throw new Error(
      `PowerShell returned an invalid supervisor PID: ${processId}`,
    );
  }
  return {
    processId,
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

const supervisor = launchSupervisor();
const supervisorOutput = collectOutput(supervisor);
const supervisorPid = supervisor.pid;
let driver;
let driverOutput = () => "";
let abruptSupervisor;
let abruptSupervisorOutput = () => "";
let abruptSupervisorPid;
let ctrlCSupervisorOutput = () => "";
let ctrlCSupervisorPid;
let bootstrapFailureSupervisor;
let bootstrapFailureOutput = () => "";
let bootstrapFailureSupervisorPid;
let frontendFailureSupervisor;
let frontendFailureOutput = () => "";
let frontendFailureSupervisorPid;
let sessionId;
let globalPid;
let hostPid;
let vitePid;

try {
  const handoffDeadline = Date.now() + gateTimeoutMilliseconds;
  while (Date.now() < handoffDeadline) {
    const processes = applicationProcesses();
    globalPid ??= globalProcess(processes)?.processId;
    hostPid ??= projectHostProcess(processes)?.processId;
    vitePid ??= frontendServerProcessId();
    const globalAlive = processes.some(
      (entry) => entry.processId === globalPid,
    );
    const hostAlive = processes.some((entry) => entry.processId === hostPid);
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
  const globalAlive = postHandoffProcesses.some(
    (entry) => entry.processId === globalPid,
  );
  const hostAlive = postHandoffProcesses.some(
    (entry) => entry.processId === hostPid,
  );
  const viteResponding = await frontendResponds();
  const supervisorAlive = supervisorProcesses().some(
    (entry) => entry.processId === supervisorPid,
  );
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
  const normalHostForest = processForestIds([hostPid]);
  const normalTree = captureDevelopmentForest(
    supervisorPid,
    normalApplications,
    [globalPid, hostPid],
  );
  const normalTreeInstances = processInstances(normalTree);
  if (
    !normalTree.includes(supervisorPid) ||
    !normalTree.includes(hostPid) ||
    !normalTree.includes(vitePid) ||
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
  if (driver?.pid) {
    terminateProcessTree(driver.pid);
    driver = undefined;
  }
  sessionId = undefined;
  closeMainWindow(hostPid);
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
  const abruptEnvironment = await waitForOwnedDevelopmentEnvironment({
    label: "Abrupt-cleanup",
    supervisorPid: abruptSupervisorPid,
    timeoutMilliseconds: gateTimeoutMilliseconds,
  });
  const abruptTree = abruptEnvironment.forest;
  const abruptTreeInstances = abruptEnvironment.forestInstances;
  if (!abruptSupervisor.kill()) {
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
  ctrlCSupervisorPid = ctrlCLaunch.processId;
  const ctrlCEnvironment = await waitForOwnedDevelopmentEnvironment({
    label: "CTRL+C",
    supervisorPid: ctrlCSupervisorPid,
    timeoutMilliseconds: Math.min(gateTimeoutMilliseconds, 60_000),
    requireIndependentHost: true,
  });
  const ctrlCTree = ctrlCEnvironment.forest;
  const ctrlCTreeInstances = ctrlCEnvironment.forestInstances;
  const ctrlCHostForest = ctrlCEnvironment.hostForest;
  sendCtrlC(ctrlCSupervisorPid);
  await assertDevelopmentCleanup("CTRL+C", ctrlCTree, ctrlCTreeInstances);

  bootstrapFailureSupervisor = launchSupervisor([
    "--myalbuns-invalid-development-option",
  ]);
  bootstrapFailureOutput = collectOutput(bootstrapFailureSupervisor);
  bootstrapFailureSupervisorPid = bootstrapFailureSupervisor.pid;
  let bootstrapFailureTree = [];
  let bootstrapFailureTreeInstances = [];
  const bootstrapFailureExit = await waitForProcessExit(
    bootstrapFailureSupervisor,
    gateTimeoutMilliseconds,
    "Bootstrap-failure supervisor",
    () => {
      const observedTree = captureDevelopmentForest(
        bootstrapFailureSupervisorPid,
        applicationProcesses(),
      );
      bootstrapFailureTree = [
        ...new Set([...bootstrapFailureTree, ...observedTree]),
      ];
      bootstrapFailureTreeInstances = mergeProcessInstances(
        bootstrapFailureTreeInstances,
        processInstances(observedTree),
      );
    },
  );
  await assertDevelopmentCleanup(
    "Bootstrap failure",
    bootstrapFailureTree,
    bootstrapFailureTreeInstances,
  );
  const bootstrapFailureText = bootstrapFailureOutput();
  if (
    bootstrapFailureExit === 0 ||
    !bootstrapFailureText.includes('"event":"dev_frontend_ready"') ||
    !bootstrapFailureText.includes(
      '"event":"dev_environment_cleanup_completed"',
    ) ||
    supervisorProcesses().some(
      (entry) => entry.processId === bootstrapFailureSupervisorPid,
    )
  ) {
    throw new Error(
      `Bootstrap failure did not clean its environment: ${JSON.stringify({ bootstrapFailureExit, bootstrapFailureSupervisorPid })}`,
    );
  }

  frontendFailureSupervisor = launchSupervisor();
  frontendFailureOutput = collectOutput(frontendFailureSupervisor);
  frontendFailureSupervisorPid = frontendFailureSupervisor.pid;
  const frontendFailureEnvironment = await waitForOwnedDevelopmentEnvironment({
    label: "Frontend-failure",
    supervisorPid: frontendFailureSupervisorPid,
    timeoutMilliseconds: gateTimeoutMilliseconds,
  });
  const failedVitePid = frontendFailureEnvironment.vitePid;
  const frontendFailureTree = frontendFailureEnvironment.forest;
  const frontendFailureTreeInstances =
    frontendFailureEnvironment.forestInstances;
  terminateProcessTree(failedVitePid);
  const frontendFailureExit = await waitForProcessExit(
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
      bootstrapFailureTreeProcessCount: bootstrapFailureTree.length,
      frontendFailureCleanupCompleted: true,
    }),
  );
} catch (error) {
  throw new Error(
    `${error instanceof Error ? error.message : String(error)}\nSupervisor:\n${supervisorOutput()}\nAbrupt supervisor:\n${abruptSupervisorOutput()}\nCTRL+C supervisor:\n${ctrlCSupervisorOutput()}\nBootstrap-failure supervisor:\n${bootstrapFailureOutput()}\nFrontend-failure supervisor:\n${frontendFailureOutput()}\nWebDriver:\n${driverOutput()}\nLogs:\n${desktopLogs().slice(-12_000)}`,
  );
} finally {
  if (driver?.pid) terminateProcessTree(driver.pid);
  if (frontendFailureSupervisorPid)
    terminateProcessTree(frontendFailureSupervisorPid);
  if (bootstrapFailureSupervisorPid)
    terminateProcessTree(bootstrapFailureSupervisorPid);
  if (ctrlCSupervisorPid) terminateProcessTree(ctrlCSupervisorPid);
  if (abruptSupervisorPid) terminateProcessTree(abruptSupervisorPid);
  if (supervisorPid) terminateProcessTree(supervisorPid);
  if (hostPid) terminateProcessTree(hostPid);
  if (globalPid) terminateProcessTree(globalPid);
}
