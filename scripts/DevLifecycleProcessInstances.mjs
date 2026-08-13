import { spawnSync } from "node:child_process";

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

export function captureProcessInstance(processId) {
  return processInstances([processId])[0] ?? null;
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

export function closeMainWindow(expectedInstance) {
  const closed = powershellJson(
    `$expected = $env:MYALBUNS_GATE_PROCESS_INSTANCE | ConvertFrom-Json; $observed = Get-CimInstance Win32_Process -Filter "ProcessId = $([int]$expected.processId)" -ErrorAction SilentlyContinue; if ($null -eq $observed -or $observed.CreationDate.ToUniversalTime().ToString('O') -cne [string]$expected.creationTimeUtc) { throw 'process instance no longer matches' }; $process = Get-Process -Id ([int]$expected.processId) -ErrorAction Stop; $confirmed = Get-CimInstance Win32_Process -Filter "ProcessId = $([int]$expected.processId)" -ErrorAction SilentlyContinue; if ($null -eq $confirmed -or $confirmed.CreationDate.ToUniversalTime().ToString('O') -cne [string]$expected.creationTimeUtc) { throw 'process instance no longer matches' }; [Console]::Out.Write(($process.CloseMainWindow() | ConvertTo-Json -Compress))`,
    {
      MYALBUNS_GATE_PROCESS_INSTANCE: JSON.stringify(expectedInstance),
    },
  );
  if (closed !== true) {
    throw new Error(
      `The Project Host exposed no closeable native window (${expectedInstance.processId})`,
    );
  }
}
