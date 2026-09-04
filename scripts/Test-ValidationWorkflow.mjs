import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { selectFocusedOwnedDialogScenarios } from "./FocusedOwnedDialogScenarios.mjs";

const scripts = path.dirname(fileURLToPath(import.meta.url));
const workspace = path.dirname(scripts);
function powershell(script, environment = {}) {
  return spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", 'Import-Module (Join-Path $PSHOME "Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"); ' + script], {
    cwd: workspace, encoding: "utf8", windowsHide: true,
    env: { ...process.env, GITHUB_ACTIONS: "", RUNNER_ENVIRONMENT: "", ...environment },
  });
}

test("native scenarios are independent selections and unknown selections fail closed", () => {
  assert.deepEqual(selectFocusedOwnedDialogScenarios("external-copy-opening-owner"), ["external-copy-opening-owner"]);
  assert.deepEqual(selectFocusedOwnedDialogScenarios("late-graphics-project-dialog"), ["late-graphics-project-dialog"]);
  assert.equal(selectFocusedOwnedDialogScenarios().length, 2);
  assert.throws(() => selectFocusedOwnedDialogScenarios("typo"), /Unknown/);
});

test("local desktop gates stop before toolchain, build or application launch", () => {
  for (const script of ["Test-FocusedOwnedDialogGate.ps1", "Test-ProductiveJourney.ps1", "Test-DevLifecycle.ps1", "Test-Issue14OpeningLockGate.ps1", "Test-WindowsPathGate.ps1", "Test-ImagingRecovery.ps1", "Test-SaveAsJourney.ps1", "Test-SessionRecoveryJourney.ps1"]) {
    const result = powershell('& $env:MYALBUNS_TEST_GATE', { MYALBUNS_TEST_GATE: path.join(scripts, script) });
    assert.notEqual(result.status, 0, script);
    assert.match(result.stderr, /Visible native tests are disabled locally/, script);
  }
});

test("only explicit desktop opt-in or a hosted CI environment permits native execution", () => {
  const command = '. $env:MYALBUNS_TEST_POLICY; Assert-NativeGateExecutionAllowed';
  const env = { MYALBUNS_TEST_POLICY: path.join(scripts, "Native-GatePolicy.ps1") };
  assert.notEqual(powershell(command, { ...env, GITHUB_ACTIONS: "true", RUNNER_ENVIRONMENT: "self-hosted" }).status, 0);
  assert.equal(powershell(command, { ...env, GITHUB_ACTIONS: "true", RUNNER_ENVIRONMENT: "github-hosted" }).status, 0);
  assert.equal(powershell(command + ' -AllowVisibleWindows', env).status, 0);
});

test("native build reuse rejects changed source, binary or commit", () => {
  const repo = mkdtempSync(path.join(tmpdir(), "myalbuns-build-proof-"));
  const git = (...args) => {
    const result = spawnSync("git", ["-C", repo, ...args], { encoding: "utf8", windowsHide: true });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  try {
    git("init", "--quiet");
    writeFileSync(path.join(repo, ".gitignore"), ".tools/\n");
    git("add", ".gitignore");
    git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "--quiet", "-m", "fixture");
    mkdirSync(path.join(repo, ".tools"));
    const bytes = Buffer.from("fixture executable identity");
    const binary = path.join(repo, ".tools", "fixture.exe");
    writeFileSync(binary, bytes);
    const artifact = { path: binary, length: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") };
    const manifest = { schemaVersion: 1, buildMode: "tauri-debug-custom-protocol", gitCommit: git("rev-parse", "HEAD"), sourceInputsDirty: false, application: artifact, fixture: artifact, processor: artifact };
    const manifestPath = path.join(repo, ".tools", "build.json");
    const save = () => writeFileSync(manifestPath, JSON.stringify(manifest));
    save();
    const command = '$ErrorActionPreference="Stop"; . $env:MYALBUNS_TEST_BUILD_MODULE; Read-NativeGateBuild -ManifestPath $env:MYALBUNS_TEST_MANIFEST -WorkspaceRoot $env:MYALBUNS_TEST_REPO | Out-Null';
    const env = { MYALBUNS_TEST_BUILD_MODULE: path.join(scripts, "Native-GateBuild.ps1"), MYALBUNS_TEST_MANIFEST: manifestPath, MYALBUNS_TEST_REPO: repo };
    const accepted = powershell(command, env);
    assert.equal(accepted.status, 0, accepted.stderr || accepted.stdout);
    const changedSnapshot = powershell(command + '; Assert-NativeGateBuildSource -Build ([pscustomobject]@{ gitCommit="A" }) -Source ([pscustomobject]@{ gitCommit="B"; sourceInputsDirty=$false }); throw "fixture must not run"', env);
    assert.match(changedSnapshot.stderr, /same clean source commit/);
    assert.doesNotMatch(changedSnapshot.stderr, /Exception: fixture must not run/);
    writeFileSync(binary, "changed");
    assert.match(powershell(command, env).stderr, /artifact changed/);
    writeFileSync(binary, bytes);
    writeFileSync(path.join(repo, "uncommitted.txt"), "source change");
    assert.match(powershell(command, env).stderr, /same clean source commit/);
    rmSync(path.join(repo, "uncommitted.txt"));
    manifest.gitCommit = "0".repeat(40); save();
    assert.match(powershell(command, env).stderr, /same clean source commit/);
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

test("default validation calls only the declared headless checks", () => {
  const validation = readFileSync(path.join(scripts, "Validate-Headless.ps1"), "utf8");
  assert.doesNotMatch(validation, /Test-FocusedOwnedDialogGate|Test-ProductiveJourney|Run-RealCanvasGate|AllowVisibleWindows/);
  for (const command of ["sidecar:prepare", "build", "test:automation", "quality:rust", "test:rust"]) assert.ok(validation.includes(command));
  assert.ok(validation.indexOf("sidecar:prepare") < validation.indexOf("frontend-build"), "the processor must exist before Tauri generates IPC contracts in a fresh checkout");
  const workflow = readFileSync(path.join(workspace, ".github/workflows/validation.yml"), "utf8");
  assert.match(workflow, /windows-2022/);
  assert.match(workflow, /-Scenario external-copy-opening-owner/);
  assert.doesNotMatch(workflow, /test:productive-journey|continue-on-error: true/);
});
