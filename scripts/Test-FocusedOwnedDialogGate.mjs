import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { once } from "node:events";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { waitForProcessInstance } from "./DevLifecycleProcessInstances.mjs";
import { nativeOwnedWindowState } from "./NativeWindowObservation.mjs";

const scripts = path.dirname(fileURLToPath(import.meta.url));
const workspace = path.resolve(scripts, "..");

function source(name) {
  return readFileSync(path.join(scripts, name), "utf8");
}

function resolveCargoTargetDirectory(cargoTargetDirectory) {
  const environment = { ...process.env };
  for (const name of Object.keys(environment)) {
    if (name.toUpperCase() === "CARGO_TARGET_DIR") delete environment[name];
  }
  if (cargoTargetDirectory !== undefined) {
    environment.CARGO_TARGET_DIR = cargoTargetDirectory;
  }
  environment.MYALBUNS_GATE_WORKSPACE = workspace;
  const result = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      String.raw`
$ErrorActionPreference = 'Stop'
. (Join-Path $env:MYALBUNS_GATE_WORKSPACE 'scripts\Local-Toolchain.ps1')
$script:WorkspaceRoot = [System.IO.Path]::GetFullPath($env:MYALBUNS_GATE_WORKSPACE)
[Console]::Out.Write((Resolve-MyAlbunsCargoTargetDirectory))
`,
    ],
    {
      cwd: workspace,
      encoding: "utf8",
      env: environment,
      windowsHide: true,
    },
  );
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return path.normalize(result.stdout.trim());
}

function resolveWorkspaceRelativePath(candidatePath) {
  const environment = {
    ...process.env,
    MYALBUNS_GATE_CANDIDATE: candidatePath,
    MYALBUNS_GATE_WORKSPACE: workspace,
  };
  const result = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      String.raw`
$ErrorActionPreference = 'Stop'
. (Join-Path $env:MYALBUNS_GATE_WORKSPACE 'scripts\Local-Toolchain.ps1')
$script:WorkspaceRoot = [System.IO.Path]::GetFullPath($env:MYALBUNS_GATE_WORKSPACE)
$relativePath = Resolve-MyAlbunsWorkspaceRelativePath -Path $env:MYALBUNS_GATE_CANDIDATE
@{ relativePath = $relativePath } | ConvertTo-Json -Compress
`,
    ],
    {
      cwd: workspace,
      encoding: "utf8",
      env: environment,
      windowsHide: true,
    },
  );
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return JSON.parse(result.stdout).relativePath ?? null;
}

function fingerprintWithClobberedWindowsPowerShellModulePath(candidatePath) {
  const clobberedModuleRoot = mkdtempSync(
    path.join(tmpdir(), "myalbuns-powershell-modules-"),
  );
  const incompatibleUtilityModule = path.join(
    clobberedModuleRoot,
    "Microsoft.PowerShell.Utility",
  );
  mkdirSync(incompatibleUtilityModule);
  writeFileSync(
    path.join(
      incompatibleUtilityModule,
      "Microsoft.PowerShell.Utility.psd1",
    ),
    String.raw`@{
  RootModule = 'Microsoft.PowerShell.Utility.Core.dll'
  ModuleVersion = '99.0.0.0'
  PowerShellVersion = '7.0'
  CompatiblePSEditions = @('Core')
  CmdletsToExport = @('Get-FileHash')
}
`,
  );
  const environment = { ...process.env };
  for (const name of Object.keys(environment)) {
    if (name.toUpperCase() === "PSMODULEPATH") delete environment[name];
  }
  environment.MYALBUNS_GATE_CANDIDATE = candidatePath;
  environment.MYALBUNS_GATE_WORKSPACE = workspace;
  environment.PSModulePath = clobberedModuleRoot;

  try {
    const result = spawnSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        String.raw`
$ErrorActionPreference = 'Stop'
. (Join-Path $env:MYALBUNS_GATE_WORKSPACE 'scripts\Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain
$candidate = Get-Item -LiteralPath $env:MYALBUNS_GATE_CANDIDATE
$fingerprint = Get-FileHash -LiteralPath $candidate.FullName -Algorithm SHA256
[ordered]@{
  powershellVersion = $PSVersionTable.PSVersion.ToString()
  powershellHome = $PSHOME
  hashCommandModule = (Get-Command Get-FileHash).ModuleName
  hashCommandModulePath = (Get-Command Get-FileHash).Module.Path
  sha256 = $fingerprint.Hash.ToLowerInvariant()
  length = [long] $candidate.Length
  lastWriteUtc = $candidate.LastWriteTimeUtc.ToString('o')
} | ConvertTo-Json -Compress
`,
      ],
      {
        cwd: workspace,
        encoding: "utf8",
        env: environment,
        windowsHide: true,
      },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);
    return JSON.parse(result.stdout);
  } finally {
    rmSync(clobberedModuleRoot, { force: true, recursive: true });
  }
}

function findAnotherVolumeCandidate() {
  const result = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      String.raw`
$workspaceRoot = [System.IO.Path]::GetPathRoot(
  [System.IO.Path]::GetFullPath($env:MYALBUNS_GATE_WORKSPACE)
)
$otherRoot = Get-PSDrive -PSProvider FileSystem |
  ForEach-Object { $_.Root } |
  Where-Object {
    -not [string]::IsNullOrWhiteSpace($_) -and
    -not [string]::Equals(
      [System.IO.Path]::GetPathRoot($_),
      $workspaceRoot,
      [System.StringComparison]::OrdinalIgnoreCase
    )
  } |
  Select-Object -First 1
if ($otherRoot) {
  $candidate = Join-Path $otherRoot 'myalbuns-cross-volume-artifact.exe'
  $actualVolume = $true
}
else {
  $workspaceLetter = $workspaceRoot.Substring(0, 1).ToUpperInvariant()
  $alternateLetter = if ($workspaceLetter -eq 'Z') { 'Y' } else { 'Z' }
  $candidate = $alternateLetter + ':\myalbuns-cross-volume-artifact.exe'
  $actualVolume = $false
}
@{
  actualVolume = $actualVolume
  candidate = $candidate
} | ConvertTo-Json -Compress
`,
    ],
    {
      cwd: workspace,
      encoding: "utf8",
      env: { ...process.env, MYALBUNS_GATE_WORKSPACE: workspace },
      windowsHide: true,
    },
  );
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return JSON.parse(result.stdout);
}

const hiddenOwnedWindowFixture = String.raw`
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
$owner = New-Object System.Windows.Forms.Form
$owner.Text = 'Headless owner'
$dialog = New-Object System.Windows.Forms.Form
$dialog.Text = 'Headless dialog'
$dialog.Owner = $owner
[void]$owner.Handle
[void]$dialog.Handle
[Console]::Out.WriteLine('READY')
[Console]::Out.Flush()
Start-Sleep -Seconds 30
`;

async function waitForFixtureReady(child) {
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  let stdout = "";
  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error(`hidden HWND fixture timed out: ${stderr}`)),
      10_000,
    );
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
      if (stdout.includes("READY")) {
        clearTimeout(timeout);
        resolve();
      }
    });
    child.once("error", reject);
    child.once("close", (code) =>
      reject(new Error(`hidden HWND fixture exited ${code}: ${stderr}`)),
    );
  });
}

test("the focused native gate has an exact closed two-scenario catalog", async () => {
  const { FOCUSED_OWNED_DIALOG_SCENARIOS } = await import(
    "./FocusedOwnedDialogScenarios.mjs"
  );

  assert.deepEqual(FOCUSED_OWNED_DIALOG_SCENARIOS, [
    "external-copy-opening-owner",
    "late-graphics-project-dialog",
  ]);
  assert.equal(new Set(FOCUSED_OWNED_DIALOG_SCENARIOS).size, 2);
});

test("the focused gate cannot call or chain the full productive journey", () => {
  const runner = source("Run-FocusedOwnedDialogGate.mjs");
  const wrapper = source("Test-FocusedOwnedDialogGate.ps1");
  const combined = `${runner}\n${wrapper}`;

  assert.doesNotMatch(combined, /Run-ProductiveJourneyGate/u);
  assert.doesNotMatch(combined, /Test-ProductiveJourney/u);
  assert.doesNotMatch(combined, /test:productive-journey/u);
  for (const unrelatedFlow of [
    "Novo Projeto",
    "SaveAs",
    "Undo",
    "Redo",
    "Export",
    "project-recovery",
  ]) {
    assert.doesNotMatch(combined, new RegExp(unrelatedFlow, "u"));
  }
});

test("the focused runner reuses exact process, HWND, WebDriver, and scratch helpers", () => {
  const runner = source("Run-FocusedOwnedDialogGate.mjs");
  const wrapper = source("Test-FocusedOwnedDialogGate.ps1");

  assert.match(runner, /from "\.\/DevLifecycleProcessInstances\.mjs"/u);
  assert.match(runner, /from "\.\/NativeWindowObservation\.mjs"/u);
  assert.match(runner, /from "\.\/GateWebDriver\.mjs"/u);
  assert.match(runner, /sameProcessInstance/u);
  assert.match(runner, /nativeOwnedWindowState/u);
  assert.match(runner, /attemptId/u);
  assert.match(runner, /sourceRevision/u);
  assert.match(runner, /webglcontextlost/u);
  assert.match(runner, /workspaceInert/u);
  assert.match(wrapper, /Gate-ScratchDirectory\.ps1/u);
  assert.match(wrapper, /Remove-GateScratchDirectory/u);
});

test("the wrapper builds and fingerprints the exact custom-protocol debug application", () => {
  const wrapper = source("Test-FocusedOwnedDialogGate.ps1");
  const staleApplicationRemoval = wrapper.indexOf(
    "Remove-Item -LiteralPath $applicationPath -Force",
  );
  const tauriBuild = wrapper.indexOf("Invoke-MyAlbunsTauriBuild");
  const applicationLookup = wrapper.indexOf(
    "Test-Path -LiteralPath $applicationPath -PathType Leaf",
  );
  const fixturePreparation = wrapper.indexOf(
    "--example prepare_focused_owned_dialog_fixtures",
  );

  assert.ok(
    staleApplicationRemoval !== -1 &&
      staleApplicationRemoval < tauriBuild &&
      tauriBuild < applicationLookup &&
      applicationLookup < fixturePreparation,
  );
  assert.match(wrapper, /Local-TauriBuild\.ps1/u);
  assert.match(
    wrapper,
    /Invoke-MyAlbunsTauriBuild\s+`\s*\n\s*-TauriArguments @\('--debug', '--no-bundle'\)/u,
  );
  assert.match(
    wrapper,
    /Get-FileHash\s+`\s*\n\s*-LiteralPath \$applicationPath\s+`\s*\n\s*-Algorithm SHA256/u,
  );
  assert.match(wrapper, /applicationArtifact/u);
});

test("the focused gate excludes only its retained evidence root from source provenance", () => {
  const wrapper = source("Test-FocusedOwnedDialogGate.ps1");

  assert.match(
    wrapper,
    /\$retainedEvidenceRoot\s*=\s*\[System\.IO\.Path\]::GetFullPath\(\s*\(Join-Path\s+\$scratchParent\s+'focused-owned-dialog-evidence'\)\s*\)/su,
  );
  assert.equal(
    wrapper.match(/-RetainedEvidenceRoot \$retainedEvidenceRoot/gu)?.length,
    2,
  );
  assert.doesNotMatch(
    wrapper,
    /-RetainedEvidenceRoot \$(?:scratchParent|workspaceRoot)/u,
  );
});

test("public hashing consumers restore Windows PowerShell 5.1 artifact metadata", () => {
  const candidatePath = path.join(scripts, "FocusedOwnedDialogScenarios.mjs");
  const candidateContents = readFileSync(candidatePath);
  const candidateStat = statSync(candidatePath);
  const metadata = fingerprintWithClobberedWindowsPowerShellModulePath(
    candidatePath,
  );

  assert.match(metadata.powershellVersion, /^5\.1\./u);
  assert.equal(metadata.hashCommandModule, "Microsoft.PowerShell.Utility");
  assert.equal(
    path.dirname(metadata.hashCommandModulePath).toLowerCase(),
    path
      .join(
        metadata.powershellHome,
        "Modules",
        "Microsoft.PowerShell.Utility",
      )
      .toLowerCase(),
  );
  assert.equal(
    metadata.sha256,
    createHash("sha256").update(candidateContents).digest("hex"),
  );
  assert.equal(metadata.length, candidateContents.length);
  assert.ok(
    Math.abs(
      new Date(metadata.lastWriteUtc).getTime() - candidateStat.mtime.getTime(),
    ) <= 1,
  );
  for (const consumerName of [
    "Test-FocusedOwnedDialogGate.ps1",
    "Test-Issue10IdentityGate.ps1",
  ]) {
    const consumer = source(consumerName);
    const initialization = consumer.indexOf("Initialize-MyAlbunsToolchain");
    const fingerprint = consumer.indexOf("Get-FileHash");

    assert.match(consumer, /Local-Toolchain\.ps1/u, consumerName);
    assert.doesNotMatch(
      consumer,
      /Import-Module\s+Microsoft\.PowerShell\.Utility/u,
      consumerName,
    );
    assert.ok(
      initialization !== -1 &&
        fingerprint !== -1 &&
        initialization < fingerprint,
      consumerName,
    );
  }
});

test("local Tauri builds and the focused gate consume one shared build pipeline", () => {
  const buildPipeline = source("Local-TauriBuild.ps1");
  const localTauri = source("Invoke-LocalTauri.ps1");
  const focusedGate = source("Test-FocusedOwnedDialogGate.ps1");

  for (const consumer of [localTauri, focusedGate]) {
    assert.match(consumer, /Local-TauriBuild\.ps1/u);
    assert.match(consumer, /Invoke-MyAlbunsTauriBuild/u);
    assert.doesNotMatch(consumer, /Prepare-Sidecar\.ps1/u);
    assert.doesNotMatch(consumer, /node_modules\\\.bin\\tauri\.cmd/u);
    assert.doesNotMatch(consumer, /& \$tauri(?:Command)? build/u);
  }

  assert.match(buildPipeline, /\$sidecarProfile/u);
  assert.match(buildPipeline, /Prepare-Sidecar\.ps1/u);
  assert.match(buildPipeline, /node_modules\\\.bin\\tauri\.cmd/u);
  assert.match(buildPipeline, /& \$tauriCommand build @TauriArguments/u);
});

test("the shared Sidecar pipeline keeps external Cargo targets inside their canonical owner", () => {
  const sidecar = source("Prepare-Sidecar.ps1");

  assert.match(
    sidecar,
    /\$baseTargetDirectory = Resolve-MyAlbunsCargoTargetDirectory/u,
  );
  assert.match(
    sidecar,
    /\$source\.StartsWith\(\s*\$targetDirectoryPrefix,/u,
  );
  assert.match(
    sidecar,
    /\$runtimeDestination\.StartsWith\(\s*\$targetDirectoryPrefix,/u,
  );
  assert.match(
    sidecar,
    /\$destination\.StartsWith\(\s*\$workspaceDirectoryPrefix,/u,
  );
  assert.doesNotMatch(
    sidecar,
    /\$(?:source|runtimeDestination)\.StartsWith\(\$script:WorkspaceRoot,/u,
  );
});

test("the focused artifact follows the canonical Cargo target directory", () => {
  const absoluteTarget = path.join(workspace, ".scratch", "absolute-target");

  assert.equal(
    resolveCargoTargetDirectory(undefined),
    path.join(workspace, "target"),
  );
  assert.equal(
    resolveCargoTargetDirectory(absoluteTarget),
    absoluteTarget,
  );
  assert.equal(
    resolveCargoTargetDirectory(path.join(".scratch", "relative-target")),
    path.join(workspace, ".scratch", "relative-target"),
  );

  const wrapper = source("Test-FocusedOwnedDialogGate.ps1");
  assert.match(
    wrapper,
    /\$cargoTargetDirectory = Resolve-MyAlbunsCargoTargetDirectory/u,
  );
  assert.match(
    wrapper,
    /\$applicationPath = Join-Path\s+`\s*\n\s*\$cargoTargetDirectory\s+`\s*\n\s*'debug\\myalbuns-desktop\.exe'/u,
  );
  assert.match(wrapper, /path = \$applicationPath/u);
  assert.match(wrapper, /relativePath = \$applicationRelativePath/u);
  assert.match(
    wrapper,
    /Resolve-MyAlbunsWorkspaceRelativePath\s+`\s*\n\s*-Path \$applicationPath/u,
  );
  assert.doesNotMatch(
    wrapper,
    /Resolve-Path -LiteralPath \$applicationPath -Relative/u,
  );
});

test("artifact metadata is relative only when it round-trips from the workspace", () => {
  const workspaceArtifact = path.join(
    workspace,
    "target",
    "debug",
    "myalbuns-desktop.exe",
  );
  const otherVolume = findAnotherVolumeCandidate();

  assert.equal(
    resolveWorkspaceRelativePath(workspaceArtifact),
    "target/debug/myalbuns-desktop.exe",
  );
  assert.notEqual(
    path.parse(otherVolume.candidate).root.toLowerCase(),
    path.parse(workspace).root.toLowerCase(),
  );
  assert.equal(resolveWorkspaceRelativePath(otherVolume.candidate), null);
  assert.equal(typeof otherVolume.actualVolume, "boolean");
});

test("external-copy readiness requires dispatch and its public terminal", async () => {
  const { confirmExternalCopyActivationLifecycle } = await import(
    "./FocusedOwnedDialogEvidence.mjs"
  );
  const host = {
    creationTimeUtc: "2026-08-31T02:00:01.000Z",
    processId: 4100,
  };
  const correlatedTerminal = {
    attempt_id: "attempt-1",
    event: "external_copy_activation_terminal",
    host_process_id: 4100,
    outcome: "cancelled",
    timestamp: "2026-08-31T02:00:02.000Z",
  };
  const activationTerminal = {
    event: "global_activation_batch_completed",
    failed_count: 1,
    focused_count: 0,
    opened_count: 0,
    project_count: 1,
    timestamp: "2026-08-31T02:00:03.000Z",
  };
  const evidence = {
    activationTerminals: [activationTerminal],
    attemptId: "attempt-1",
    correlatedTerminals: [correlatedTerminal],
    pendingHost: host,
  };

  assert.deepEqual(
    confirmExternalCopyActivationLifecycle(evidence),
    {
      activationDispatched: true,
      hostCorrelated: true,
      publicTerminalObserved: true,
    },
  );
  assert.throws(
    () =>
      confirmExternalCopyActivationLifecycle({
        ...evidence,
        correlatedTerminals: [
          { ...correlatedTerminal, attempt_id: "another-attempt" },
        ],
      }),
    /attempt/u,
  );
  assert.throws(
    () =>
      confirmExternalCopyActivationLifecycle({
        ...evidence,
        correlatedTerminals: [
          { ...correlatedTerminal, host_process_id: 4200 },
        ],
      }),
    /Host/u,
  );
  assert.throws(
    () =>
      confirmExternalCopyActivationLifecycle({
        ...evidence,
        correlatedTerminals: [correlatedTerminal, correlatedTerminal],
      }),
    /exactly one correlated terminal/u,
  );
  assert.throws(
    () =>
      confirmExternalCopyActivationLifecycle({
        ...evidence,
        activationTerminals: [activationTerminal, activationTerminal],
      }),
    /exactly one activation terminal/u,
  );
  assert.throws(
    () =>
      confirmExternalCopyActivationLifecycle({
        ...evidence,
        correlatedTerminals: [
          { ...correlatedTerminal, timestamp: host.creationTimeUtc },
        ],
      }),
    /strictly after the pending Host/u,
  );

  const runner = source("Run-FocusedOwnedDialogGate.mjs");
  const scenarioStart = runner.indexOf(
    "async function observeExternalCopyScenario()",
  );
  const scenarioEnd = runner.indexOf(
    "async function observeGraphicsScenario()",
    scenarioStart,
  );
  const scenario = runner.slice(scenarioStart, scenarioEnd);
  const terminalObservation = scenario.indexOf(
    '"external_copy_activation_terminal"',
  );
  const lifecycleConfirmation = scenario.indexOf(
    "confirmExternalCopyActivationLifecycle",
  );
  const readyReturn = scenario.indexOf("return {", lifecycleConfirmation);

  assert.ok(
    terminalObservation !== -1 &&
      terminalObservation < lifecycleConfirmation &&
      lifecycleConfirmation < readyReturn,
  );
  assert.match(scenario, /\.slice\(correlatedTerminalCount\)/u);
  assert.match(scenario, /\.slice\(activationTerminalCount\)/u);
  assert.doesNotMatch(scenario, /\.at\(-1\)/u);
});

test("Global publishes the exact external-copy attempt and Host terminal", () => {
  const runtime = readFileSync(
    path.join(workspace, "src-tauri", "src", "global_runtime.rs"),
    "utf8",
  );
  const supervisor = readFileSync(
    path.join(
      workspace,
      "src-tauri",
      "src",
      "project_bootstrap",
      "supervisor.rs",
    ),
    "utf8",
  );
  const cancellationStart = runtime.indexOf(
    "OpeningExternalCopyDecision::Cancel =>",
  );
  const saveCopyStart = runtime.indexOf(
    "OpeningExternalCopyDecision::SaveCopyAs =>",
    cancellationStart,
  );
  const cancellation = runtime.slice(cancellationStart, saveCopyStart);

  assert.match(
    supervisor,
    /pub\(crate\) fn host_process_id\(&mut self\) -> u32/u,
  );
  assert.match(cancellation, /host_process_id = pending\.host_process_id\(\)/u);
  assert.match(cancellation, /attempt_id = %attempt_id/u);
  assert.match(cancellation, /host_process_id/u);
  assert.match(cancellation, /event = "external_copy_activation_terminal"/u);
});

test("external-copy discovery observes its public decision before sampling the pending Host", () => {
  const runner = source("Run-FocusedOwnedDialogGate.mjs");
  const start = runner.indexOf("async function observeExternalCopyScenario()");
  const end = runner.indexOf("async function observeGraphicsScenario()", start);
  const scenario = runner.slice(start, end);
  const decisionTarget = scenario.indexOf(
    'const target = await waitFor("external-copy decision target"',
  );
  const attemptCorrelation = scenario.indexOf(
    'const attemptId = new URL(target.url).searchParams.get("attemptId")',
  );
  const pendingHost = scenario.indexOf(
    'await waitForNewApplication(\n      isBootstrapHost,\n      [],\n      "external-copy pending Host"',
  );

  assert.ok(
    decisionTarget !== -1 &&
      decisionTarget < attemptCorrelation &&
      attemptCorrelation < pendingHost,
  );
});

test("the automated Project retires its inherited debug port before owned dialogs", () => {
  const productRuntime = readFileSync(
    path.join(workspace, "src-tauri", "src", "product_runtime.rs"),
    "utf8",
  );
  const normalOwnerReady = productRuntime.indexOf(
    "(window, Some(policy_readiness))",
  );
  const inheritedDebugPortRetired = productRuntime.indexOf(
    "desktop_webview_policy::retire_inherited_debug_arguments_before_replacement()?",
  );
  const ownerConfigured = productRuntime.indexOf(
    "project_window.set_title(&initial_window_title)?",
  );

  assert.ok(
    normalOwnerReady !== -1 &&
      normalOwnerReady < inheritedDebugPortRetired &&
      inheritedDebugPortRetired < ownerConfigured,
  );
});

test("the HWND probe closes over native owner chains from GUI-thread fallbacks", () => {
  const observer = source("NativeWindowObservation.mjs");

  assert.match(observer, /MainWindowHandle/u);
  assert.match(observer, /GetGUIThreadInfo/u);
  assert.match(observer, /GetAncestor/u);
  assert.match(observer, /ObserveOwnerChain/u);
});

test("the HWND probe observes an exact process with a hidden owned-window pair", async () => {
  const child = spawn(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", hiddenOwnedWindowFixture],
    { windowsHide: true, stdio: ["ignore", "pipe", "pipe"] },
  );
  try {
    await waitForFixtureReady(child);
    const instance = await waitForProcessInstance(
      child.pid,
      "hidden owned-window fixture",
    );
    const state = nativeOwnedWindowState(instance);
    const owner = state.windows.find(
      (window) => window.title === "Headless owner",
    );
    const dialog = state.windows.find(
      (window) => window.title === "Headless dialog",
    );
    assert.ok(owner);
    assert.ok(dialog);
    assert.equal(dialog.ownerHwnd, owner.hwnd);
    assert.equal(owner.visible, false);
    assert.equal(dialog.visible, false);
    assert.equal(state.dialogCount, 0);
  } finally {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    if (child.exitCode === null && child.signalCode === null) {
      await once(child, "close");
    }
  }
});

test("a failed visible run retains its exact diagnostics before scratch cleanup", () => {
  const wrapper = source("Test-FocusedOwnedDialogGate.ps1");

  assert.match(wrapper, /catch\s*\{/u);
  assert.match(wrapper, /failure\.json/u);
  assert.match(wrapper, /focused-native\.log/u);
  assert.match(wrapper, /process-logs/u);
});

test("the visible gate is opt-in and absent from standard headless commands", () => {
  const packageJson = JSON.parse(
    readFileSync(path.join(workspace, "package.json"), "utf8"),
  );
  const focusedCommand = packageJson.scripts["test:native-owned-dialogs"];

  assert.equal(
    focusedCommand,
    "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Test-FocusedOwnedDialogGate.ps1",
  );
  for (const standard of ["test", "build", "typecheck", "test:rust", "quality:rust"]) {
    assert.doesNotMatch(packageJson.scripts[standard], /FocusedOwnedDialog/u);
    assert.doesNotMatch(packageJson.scripts[standard], /native-owned-dialogs/u);
  }
});

test("the operational policy reserves the full journey for integration or explicit permission", () => {
  const policy = readFileSync(
    path.join(workspace, "docs", "agents", "native-ui-gates.md"),
    "utf8",
  );

  assert.match(policy, /headless.*interactive work/isu);
  assert.match(policy, /focused native gate.*once after GREEN/isu);
  assert.match(
    policy,
    /full productive journey.*integration.*release.*explicit\s+permission/isu,
  );
  assert.match(policy, /MYALBUNS_UI_SCENARIO_IDS/u);
});
