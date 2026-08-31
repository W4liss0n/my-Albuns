import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { readFileSync } from "node:fs";
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
  const tauriBuild = wrapper.indexOf("& $tauri build --debug --no-bundle");
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
  assert.match(wrapper, /Prepare-Sidecar\.ps1'\) -Profile debug/u);
  assert.match(wrapper, /node_modules\\\.bin\\tauri\.cmd/u);
  assert.match(wrapper, /& \$tauri build --debug --no-bundle/u);
  assert.match(
    wrapper,
    /Get-FileHash\s+`\s*\n\s*-LiteralPath \$applicationPath\s+`\s*\n\s*-Algorithm SHA256/u,
  );
  assert.match(wrapper, /applicationArtifact/u);
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
  assert.doesNotMatch(
    wrapper,
    /relativePath = 'target\/debug\/myalbuns-desktop\.exe'/u,
  );
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
