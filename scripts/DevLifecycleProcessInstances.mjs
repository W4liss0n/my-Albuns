import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

const delay = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

export function powershellJson(script, environment = {}) {
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
    throw new Error(
      `Windows process observation failed: ${result.stderr || result.stdout}`,
    );
  }
  return result.stdout.trim() ? JSON.parse(result.stdout) : null;
}

export function processInstances(processIds) {
  const requested = [
    ...new Set(
      processIds.filter(
        (processId) => Number.isInteger(processId) && processId > 0,
      ),
    ),
  ];
  if (requested.length === 0) return [];
  return (
    powershellJson(
      `$requested = @($env:MYALBUNS_GATE_PROCESS_IDS -split ',' | ForEach-Object { [int]$_ }); $instances = @(Get-CimInstance Win32_Process -ErrorAction Stop | Where-Object { $requested -contains [int]$_.ProcessId } | ForEach-Object { [ordered]@{ processId = [int]$_.ProcessId; parentProcessId = [int]$_.ParentProcessId; creationTimeUtc = $_.CreationDate.ToUniversalTime().ToString('O'); name = [string]$_.Name; commandLine = [string]$_.CommandLine } }); [Console]::Out.Write((ConvertTo-Json -InputObject $instances -Compress))`,
      { MYALBUNS_GATE_PROCESS_IDS: requested.join(",") },
    ) ?? []
  );
}

export function processInstancesByExecutable(executablePath, executableName) {
  return (
    powershellJson(
      `$items = @(Get-CimInstance Win32_Process -Filter "Name = '$env:MYALBUNS_GATE_EXECUTABLE_NAME'" -ErrorAction Stop | Where-Object { [StringComparer]::OrdinalIgnoreCase.Equals($_.ExecutablePath, $env:MYALBUNS_GATE_EXECUTABLE_PATH) } | ForEach-Object { [ordered]@{ processId = [int]$_.ProcessId; parentProcessId = [int]$_.ParentProcessId; creationTimeUtc = $_.CreationDate.ToUniversalTime().ToString('O'); name = [string]$_.Name; commandLine = [string]$_.CommandLine } }); [Console]::Out.Write((ConvertTo-Json -InputObject $items -Compress))`,
      {
        MYALBUNS_GATE_EXECUTABLE_NAME: executableName,
        MYALBUNS_GATE_EXECUTABLE_PATH: executablePath,
      },
    ) ?? []
  );
}

export function assertNoPreexistingProcessInstances(
  executablePath,
  executableName,
) {
  const existing = processInstancesByExecutable(executablePath, executableName);
  if (existing.length !== 0) {
    throw new Error(
      `Development lifecycle gate found ${existing.length} pre-existing application process instance(s)`,
    );
  }
}

export function captureProcessInstance(processId) {
  return processInstances([processId])[0] ?? null;
}

export function captureListeningProcessInstance(port) {
  if (!Number.isInteger(port) || port <= 0 || port > 65_535) {
    throw new Error(`Invalid listener port: ${port}`);
  }
  return powershellJson(
    String.raw`
$port = [int]$env:MYALBUNS_GATE_LISTENER_PORT
$listener = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1
if ($null -eq $listener) {
    [Console]::Out.Write('null')
    exit 0
}
$process = Get-Process -Id ([int]$listener.OwningProcess) -ErrorAction SilentlyContinue
if ($null -eq $process) {
    [Console]::Out.Write('null')
    exit 0
}
[void]$process.Handle
if ($process.HasExited) {
    [Console]::Out.Write('null')
    exit 0
}
$startTimeUtc = $process.StartTime.ToUniversalTime()
$creationTimeUtc = $startTimeUtc.AddTicks(-($startTimeUtc.Ticks % 10)).ToString('O')
$observed = Get-CimInstance Win32_Process -Filter "ProcessId = $([int]$process.Id)" -ErrorAction SilentlyContinue
$confirmedListener = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue | Where-Object { [int]$_.OwningProcess -eq [int]$process.Id } | Select-Object -First 1
if ($null -eq $observed -or $null -eq $confirmedListener -or $process.HasExited -or $observed.CreationDate.ToUniversalTime().ToString('O') -cne $creationTimeUtc) {
    [Console]::Out.Write('null')
    exit 0
}
$instance = [ordered]@{ processId = [int]$observed.ProcessId; parentProcessId = [int]$observed.ParentProcessId; creationTimeUtc = $creationTimeUtc; name = [string]$observed.Name; commandLine = [string]$observed.CommandLine }
[Console]::Out.Write((ConvertTo-Json -InputObject $instance -Compress))
`,
    { MYALBUNS_GATE_LISTENER_PORT: String(port) },
  );
}

export function startProcessInstanceInOwnConsole({
  executablePath,
  arguments: processArguments = [],
  workingDirectory,
  standardOutputPath,
  standardErrorPath,
  authorityPath,
  environment = {},
}) {
  const request = {
    executablePath,
    arguments: processArguments,
    workingDirectory,
    standardOutputPath,
    standardErrorPath,
    authorityPath,
  };
  const result = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      String.raw`
$ErrorActionPreference = 'Stop'
$request = $env:MYALBUNS_GATE_PROCESS_LAUNCH | ConvertFrom-Json
$start = @{
    FilePath = [string]$request.executablePath
    WorkingDirectory = [string]$request.workingDirectory
    PassThru = $true
    WindowStyle = 'Hidden'
    RedirectStandardOutput = [string]$request.standardOutputPath
    RedirectStandardError = [string]$request.standardErrorPath
}
if (@($request.arguments).Count -gt 0) {
    $start.ArgumentList = [string[]]$request.arguments
}
$process = Start-Process @start
[void]$process.Handle
if ($process.HasExited) {
    throw 'launched process exited before exact instance capture'
}
$startTimeUtc = $process.StartTime.ToUniversalTime()
$creationTimeUtc = $startTimeUtc.AddTicks(-($startTimeUtc.Ticks % 10)).ToString('O')
if ($process.HasExited) {
    throw 'launched process instance could not be validated'
}
$instance = [ordered]@{ processId = [int]$process.Id; parentProcessId = 0; creationTimeUtc = $creationTimeUtc; name = [string]$process.ProcessName; commandLine = '' }
[IO.File]::WriteAllText([string]$request.authorityPath, (ConvertTo-Json -InputObject $instance -Compress), [Text.UTF8Encoding]::new($false))
$process.Dispose()
[Environment]::Exit(0)
`,
    ],
    {
      windowsHide: true,
      stdio: "ignore",
      env: {
        ...process.env,
        ...environment,
        MYALBUNS_GATE_PROCESS_LAUNCH: JSON.stringify(request),
      },
    },
  );
  if (result.status !== 0) {
    throw new Error("Exact process launch failed");
  }
  return JSON.parse(readFileSync(authorityPath, "utf8"));
}

export async function waitForProcessInstance(
  processId,
  label,
  timeoutMilliseconds = 10_000,
) {
  const deadline = Date.now() + timeoutMilliseconds;
  while (Date.now() < deadline) {
    const instance = captureProcessInstance(processId);
    if (instance) return instance;
    await delay(25);
  }
  throw new Error(`${label} process instance did not become observable`);
}

export function processInstanceKey(instance) {
  return `${instance.processId}:${instance.creationTimeUtc}`;
}

export function sameProcessInstance(left, right) {
  return (
    left != null &&
    right != null &&
    processInstanceKey(left) === processInstanceKey(right)
  );
}

export function mergeProcessInstances(...collections) {
  return [
    ...new Map(
      collections
        .flat()
        .filter(Boolean)
        .map((instance) => [processInstanceKey(instance), instance]),
    ).values(),
  ];
}

export function aliveProcessInstances(expectedInstances) {
  const expected = expectedInstances.filter(Boolean);
  const currentByKey = new Map(
    processInstances(expected.map((instance) => instance.processId)).map(
      (instance) => [processInstanceKey(instance), instance],
    ),
  );
  return expected.filter((instance) =>
    currentByKey.has(processInstanceKey(instance)),
  );
}

export function processForestInstances(rootInstances) {
  const roots = mergeProcessInstances(rootInstances);
  if (roots.length === 0) return [];
  const all =
    powershellJson(
      `$instances = @(Get-CimInstance Win32_Process -ErrorAction Stop | ForEach-Object { [ordered]@{ processId = [int]$_.ProcessId; parentProcessId = [int]$_.ParentProcessId; creationTimeUtc = $_.CreationDate.ToUniversalTime().ToString('O'); name = [string]$_.Name; commandLine = [string]$_.CommandLine } }); [Console]::Out.Write((ConvertTo-Json -InputObject $instances -Compress))`,
    ) ?? [];
  const currentKeys = new Set(all.map(processInstanceKey));
  const forestById = new Map(
    roots
      .filter((root) => currentKeys.has(processInstanceKey(root)))
      .map((root) => [root.processId, root]),
  );
  let previousSize;
  do {
    previousSize = forestById.size;
    for (const candidate of all) {
      const parent = forestById.get(candidate.parentProcessId);
      if (
        parent &&
        candidate.creationTimeUtc >= parent.creationTimeUtc &&
        !forestById.has(candidate.processId)
      ) {
        forestById.set(candidate.processId, candidate);
      }
    }
  } while (forestById.size > previousSize);
  return all.filter((candidate) => forestById.has(candidate.processId));
}

const exactProcessAuthorityPrelude = String.raw`
$ErrorActionPreference = 'Stop'
$expected = $env:MYALBUNS_GATE_PROCESS_INSTANCE | ConvertFrom-Json

function Test-MyAlbunsExactProcessInstance {
    param($Expected, $Process)
    if ($null -eq $Process -or $Process.HasExited) {
        return $false
    }
    $observed = Get-CimInstance Win32_Process -Filter "ProcessId = $([int]$Expected.processId)" -ErrorAction SilentlyContinue
    return $null -ne $observed -and $observed.CreationDate.ToUniversalTime().ToString('O') -ceq [string]$Expected.creationTimeUtc
}

function Open-MyAlbunsExactProcessInstance {
    param($Expected)
    $candidate = Get-Process -Id ([int]$Expected.processId) -ErrorAction Stop
    [void]$candidate.Handle
    if (-not (Test-MyAlbunsExactProcessInstance -Expected $Expected -Process $candidate)) {
        $candidate.Dispose()
        throw 'process instance no longer matches'
    }
    return $candidate
}

function Assert-MyAlbunsExactProcessInstance {
    param($Expected, $Process)
    if (-not (Test-MyAlbunsExactProcessInstance -Expected $Expected -Process $Process)) {
        throw 'process instance no longer matches'
    }
}

$process = Open-MyAlbunsExactProcessInstance -Expected $expected
`;

export function closeMainWindow(expectedInstance) {
  powershellJson(
    String.raw`
${exactProcessAuthorityPrelude}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class MyAlbunsWindowSignal {
    public delegate bool EnumWindowsCallback(IntPtr window, IntPtr parameter);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool EnumWindows(EnumWindowsCallback callback, IntPtr parameter);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    public static extern IntPtr SendMessageTimeoutW(
        IntPtr window,
        uint message,
        UIntPtr wParam,
        IntPtr lParam,
        uint flags,
        uint timeoutMilliseconds,
        out UIntPtr result);

    public static IntPtr FindVisibleTopLevelWindow(uint expectedProcessId) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, _) => {
            uint processId;
            GetWindowThreadProcessId(window, out processId);
            if (processId == expectedProcessId && IsWindowVisible(window)) {
                found = window;
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@
$window = [MyAlbunsWindowSignal]::FindVisibleTopLevelWindow([uint32]$expected.processId)
if ($window -eq [IntPtr]::Zero) {
    throw 'the exact process exposes no visible top-level window'
}
$windowProcessId = [uint32]0
if ([MyAlbunsWindowSignal]::GetWindowThreadProcessId($window, [ref]$windowProcessId) -eq 0 -or $windowProcessId -ne [uint32]$expected.processId) {
    throw 'main window no longer belongs to the exact process instance'
}
$messageResult = [UIntPtr]::Zero
Assert-MyAlbunsExactProcessInstance -Expected $expected -Process $process
$sent = [MyAlbunsWindowSignal]::SendMessageTimeoutW(
    $window,
    0x0010,
    [UIntPtr]::Zero,
    [IntPtr]::Zero,
    0x0003,
    5000,
    [ref]$messageResult)
if ($sent -eq [IntPtr]::Zero) {
    throw 'bounded WM_CLOSE delivery timed out or failed'
}
[Console]::Out.Write('true')
`,
    {
      MYALBUNS_GATE_PROCESS_INSTANCE: JSON.stringify(expectedInstance),
    },
  );
}

export function terminateProcessInstance(expectedInstance) {
  if (!expectedInstance) return false;
  return (
    powershellJson(
      String.raw`
try {
${exactProcessAuthorityPrelude}
    Assert-MyAlbunsExactProcessInstance -Expected $expected -Process $process
}
catch {
    [Console]::Out.Write('false')
    exit 0
}
$process.Kill()
[Console]::Out.Write('true')
`,
      {
        MYALBUNS_GATE_PROCESS_INSTANCE: JSON.stringify(expectedInstance),
      },
    ) === true
  );
}

export function sendCtrlC(expectedInstance) {
  const script = String.raw`
${exactProcessAuthorityPrelude}
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
if (-not [MyAlbunsConsoleSignal]::AttachConsole([uint32]$expected.processId)) {
    throw "AttachConsole failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
}
try {
    if (-not [MyAlbunsConsoleSignal]::SetConsoleCtrlHandler([IntPtr]::Zero, $true)) {
        throw "SetConsoleCtrlHandler failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
    }
    Assert-MyAlbunsExactProcessInstance -Expected $expected -Process $process
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
        MYALBUNS_GATE_PROCESS_INSTANCE: JSON.stringify(expectedInstance),
      },
    },
  );
  if (result.status !== 0) {
    throw new Error(
      `CTRL+C delivery failed: ${result.stderr || result.stdout}`,
    );
  }
}
