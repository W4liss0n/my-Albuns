import assert from "node:assert/strict";
import { spawn } from "node:child_process";
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
