import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const scripts = path.dirname(fileURLToPath(import.meta.url));
const workspace = path.resolve(scripts, "..");

function source(name) {
  return readFileSync(path.join(scripts, name), "utf8");
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
