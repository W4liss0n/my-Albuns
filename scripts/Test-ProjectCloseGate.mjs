import assert from "node:assert/strict";
import test from "node:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createProjectCloseEnvironment } from "./ProjectCloseGateEnvironment.mjs";
import { assertProjectCloseEvidence, summarizeCloseStages } from "./ProjectCloseGateEvidence.mjs";

function proof() {
  const originalHost = { processId: 101, creationTimeUtc: "2026-09-04T10:00:00Z" };
  const copyHost = { processId: 202, creationTimeUtc: "2026-09-04T10:00:01Z" };
  const original = { projectId: "original", revision: 1, dpi: 320, sha256: "a".repeat(64) };
  const copy = { projectId: "copy", revision: 2, dpi: 420, sha256: "b".repeat(64) };
  return {
    originalHost, copyHost,
    before: { original, copy }, after: structuredClone({ original, copy }),
    aliveBeforeCleanup: [copyHost], originalWindowsGone: true, replacementGlobalReady: true,
    requestedAtUtc: "2026-09-04T10:01:00Z",
    replacementGlobal: { processId: 303, parentProcessId: 101, creationTimeUtc: "2026-09-04T10:01:01Z" },
    records: ["project_close_command_received", "clean_project_close_requested"].map((event) => ({
      timestamp: "2026-09-04T10:01:01Z", event, process_id: 101, window_label: "project",
    })),
    copyUi: { dpi: "420", state: {
      readyState: "complete", dialogCount: 0, workspaceInert: false, workspaceBusy: null,
      alerts: [], commandMenus: [{ label: "Arquivo", disabled: false }],
    } },
  };
}

test("accepts clean original exit only while the independently saved copy remains usable", () => {
  const result = assertProjectCloseEvidence(proof());
  assert.equal(result.scenario, "saved-original-close");
  assert.equal(result.originalHostExited, true);
  assert.equal(result.copyHostAlive, true);
});

for (const [name, invalidate] of [
  ["the original is merely killed by later cleanup", (e) => e.aliveBeforeCleanup.push(e.originalHost)],
  ["the copy also exited", (e) => e.aliveBeforeCleanup = []],
  ["a reused copy PID replaced the original instance", (e) => e.aliveBeforeCleanup[0] = { ...e.copyHost, creationTimeUtc: "2026-09-04T10:02:00Z" }],
  ["the copy supplied the clean-close event", (e) => e.records[1].process_id = 202],
  ["the terminal belongs to an earlier attempt", (e) => e.records[1].timestamp = "2026-09-04T10:00:00Z"],
  ["the original crashed without a clean-close terminal", (e) => e.records.pop()],
  ["the copy retained the original identity", (e) => e.before.copy.projectId = "original"],
  ["the original save changed after closing", (e) => e.after.original.sha256 = "c".repeat(64)],
  ["the copy save changed after closing", (e) => e.after.copy.sha256 = "c".repeat(64)],
  ["the copy is blocked", (e) => e.copyUi.state.commandMenus[0].disabled = true],
  ["the copy is an empty diagnostic page", (e) => e.copyUi.state.commandMenus = []],
  ["the original native window remains", (e) => e.originalWindowsGone = false],
  ["an unrelated Global is mistaken for the replacement", (e) => e.replacementGlobal.parentProcessId = 202],
  ["the replacement Global never became ready", (e) => e.replacementGlobalReady = false],
]) {
  test(`rejects a passing receipt when ${name}`, () => {
    const evidence = proof();
    invalidate(evidence);
    assert.throws(() => assertProjectCloseEvidence(evidence));
  });
}

test("diagnosis distinguishes the exact Host stages from uncorrelated frontend clues", () => {
  const evidence = proof();
  evidence.records.push({ timestamp: "2026-09-04T10:01:02Z", process_id: 202, event: "project_window_destroyed" });
  evidence.records.push({ timestamp: "2026-09-04T10:01:02Z", component: "project-close", event: "project_close_requested" });
  const stages = summarizeCloseStages(evidence.records, evidence.originalHost, evidence.requestedAtUtc);
  assert.equal(stages.lastHostStage, "clean_project_close_requested");
  assert.equal(stages.frontendEvents.length, 1);
  assert.equal(summarizeCloseStages([], evidence.originalHost, evidence.requestedAtUtc).lastHostStage, null);
});


test("retained close evidence preserves clean-source reuse without hiding other scratch changes", () => {
  const repo = mkdtempSync(path.join(tmpdir(), "myalbuns-close-evidence-"));
  const workspace = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const git = (...args) => {
    const result = spawnSync("git", ["-C", repo, ...args], { encoding: "utf8", windowsHide: true });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout;
  };
  try {
    git("init", "--quiet");
    writeFileSync(path.join(repo, ".gitignore"), readFileSync(path.join(workspace, ".gitignore")));
    git("add", ".gitignore");
    git("-c", "user.name=Gate test", "-c", "user.email=gate@example.invalid", "commit", "--quiet", "-m", "fixture");
    const evidence = path.join(repo, ".scratch", "project-close-evidence", "run-1");
    mkdirSync(evidence, { recursive: true });
    writeFileSync(path.join(evidence, "failure-project-close.json"), "{}");
    writeFileSync(path.join(evidence, "failure-original.png"), "synthetic evidence");
    assert.equal(git("status", "--porcelain", "--untracked-files=all"), "");
    writeFileSync(path.join(repo, ".scratch", "unrelated-source.txt"), "must stay visible");
    assert.match(git("status", "--porcelain", "--untracked-files=all"), /unrelated-source\.txt/u);
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

test("direct runner execution fails before starting an application", () => {
  const result = spawnSync(process.execPath, [fileURLToPath(new URL("./Run-ProjectCloseGate.mjs", import.meta.url))], {
    encoding: "utf8", windowsHide: true,
    env: { ...process.env, MYALBUNS_NATIVE_GATE_AUTHORIZED: "" },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /direct launches are disabled/u);
});


test("the normal Save As transition cannot be bypassed by an inherited automation flag, even false", () => {
  for (const value of ["false", "true", ""]) {
    const inherited = { TAURI_WEBVIEW_AUTOMATION: value, MYALBUNS_TAURI_WEBDRIVER_PROJECT: "unexpected-project",
      MYALBUNS_DEV_ALTERNATE_HOST_WEBVIEW_DEBUG_PORT: "40999", KEEP: "unchanged" };
    const result = createProjectCloseEnvironment(inherited, {
      scratch: "/fixture", label: "original", globalPort: 40000, hostPort: 40001, dialogPort: 40002, saveAsPort: 40003,
    });
    assert.equal(Object.hasOwn(result, "TAURI_WEBVIEW_AUTOMATION"), false);
    assert.equal(Object.hasOwn(result, "MYALBUNS_TAURI_WEBDRIVER_PROJECT"), false);
    assert.equal(Object.hasOwn(result, "MYALBUNS_DEV_ALTERNATE_HOST_WEBVIEW_DEBUG_PORT"), false);
    assert.equal(result.MYALBUNS_DEV_SAVE_AS_WEBVIEW_DEBUG_PORT, "40003");
    assert.equal(result.MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT, "40001");
    assert.equal(result.KEEP, "unchanged");
    assert.equal(inherited.TAURI_WEBVIEW_AUTOMATION, value);
  }
});
