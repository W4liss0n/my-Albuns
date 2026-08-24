import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  renderUiAcceptanceReport,
  servedFilePath,
  validateUiAcceptanceManifest,
} from "./UiAcceptance.mjs";

const scriptsDirectory = path.dirname(fileURLToPath(import.meta.url));
const workspace = path.resolve(scriptsDirectory, "..");
const manifestPath = path.join(workspace, "src", "test", "uiAcceptanceScenarios.json");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));

test("the canonical UI acceptance manifest is valid and points to served files", () => {
  validateUiAcceptanceManifest(manifest);
  for (const scenario of manifest.scenarios) {
    assert.equal(existsSync(servedFilePath(workspace, scenario.implementationPath)), true, `${scenario.id} implementation is missing`);
    if (scenario.comparison.kind === "paired") {
      assert.equal(existsSync(servedFilePath(workspace, scenario.referencePath)), true, `${scenario.id} reference is missing`);
    }
  }
});

test("editor scenarios declare honest, surface-matched comparisons", () => {
  const editorScenarioIds = new Set([
    "album-information-validation-tooltip",
    "canvas-normal",
    "canvas-sheet-editing",
    "canvas-sheet-editing-exit",
    "media-panel",
    "media-panel-decorative-selection",
    "media-panel-popup-keyboard",
    "sheet-grid",
    "sheet-grid-hover",
    "sheet-grid-selection",
  ]);
  const editorScenarios = manifest.scenarios.filter((scenario) =>
    editorScenarioIds.has(scenario.id),
  );

  assert.equal(editorScenarios.length, editorScenarioIds.size);
  for (const scenario of editorScenarios) {
    assert.equal(typeof scenario.comparison.surface, "string");
    if (scenario.comparison.kind === "paired") {
      assert.equal(typeof scenario.comparison.implementationCaptureSelector, "string");
      assert.equal(typeof scenario.comparison.referenceCaptureSelector, "string");
    } else {
      assert.match(scenario.comparison.reason, /referência visual vigente/i);
      assert.equal("referencePath" in scenario, false);
    }
  }

  const sheetGrid = editorScenarios.find((scenario) => scenario.id === "sheet-grid");
  assert.equal(sheetGrid?.comparison.kind, "paired");
  assert.match(sheetGrid?.readySelector ?? "", /sheet-grid/);
  assert.match(sheetGrid?.comparison.referenceCaptureSelector ?? "", /data-sec="grade"/);

  const pairedFingerprints = new Set();
  for (const scenario of editorScenarios.filter(
    (candidate) => candidate.comparison.kind === "paired",
  )) {
    const fingerprint = JSON.stringify({
      actions: scenario.referenceActions ?? [],
      captureSelector: scenario.comparison.referenceCaptureSelector,
      path: scenario.referencePath,
      viewport: scenario.viewport,
    });
    assert.equal(
      pairedFingerprints.has(fingerprint),
      false,
      `${scenario.id} repeats a misleading reference state`,
    );
    pairedFingerprints.add(fingerprint);
  }
});

test("manifest validation rejects duplicate scenario ids", () => {
  const duplicate = structuredClone(manifest);
  duplicate.scenarios[1].id = duplicate.scenarios[0].id;
  assert.throws(() => validateUiAcceptanceManifest(duplicate), /duplicates/);
});

test("manifest validation rejects external and escaping paths", () => {
  for (const invalidPath of ["https://example.com/reference", "/../outside.html", "//example.com/reference"]) {
    const invalid = structuredClone(manifest);
    invalid.scenarios[0].referencePath = invalidPath;
    assert.throws(() => validateUiAcceptanceManifest(invalid), /referencePath/);
  }
});

test("manifest validation rejects incomplete implementation and reference actions", () => {
  const missingSelector = structuredClone(manifest);
  missingSelector.scenarios[0].actions = [{ type: "click" }];
  assert.throws(() => validateUiAcceptanceManifest(missingSelector), /selector is required/);

  const missingText = structuredClone(manifest);
  missingText.scenarios[0].referenceActions = [{ type: "click-text" }];
  assert.throws(() => validateUiAcceptanceManifest(missingText), /text is required/);
});

test("runner executes focus, hover, keyboard and input actions through WebDriver", async () => {
  const { performUiAcceptanceAction } = await import("./UiAcceptanceRunner.mjs");
  const requests = [];
  const executions = [];
  const request = async (method, endpoint, body) => {
    requests.push({ body, endpoint, method });
    return null;
  };
  const locateSelector = async (selector) => `element:${selector}`;
  const locateText = async (text) => `text:${text}`;
  const execute = async (script, args) => {
    executions.push({ args, script });
    return true;
  };
  const common = {
    execute,
    locateSelector,
    locateText,
    request,
    sessionId: "session-1",
  };

  await performUiAcceptanceAction({
    ...common,
    action: { type: "focus", selector: "#target" },
  });
  assert.match(executions[0].script, /focus/);
  assert.deepEqual(executions[0].args, [
    { "element-6066-11e4-a52e-4f735466cecf": "element:#target" },
  ]);

  await performUiAcceptanceAction({
    ...common,
    action: { type: "hover", selector: "#target" },
  });
  assert.deepEqual(requests.at(-1), {
    method: "POST",
    endpoint: "/session/session-1/actions",
    body: {
      actions: [
        {
          type: "pointer",
          id: "acceptance-pointer",
          parameters: { pointerType: "mouse" },
          actions: [
            {
              type: "pointerMove",
              duration: 0,
              origin: {
                "element-6066-11e4-a52e-4f735466cecf": "element:#target",
              },
              x: 0,
              y: 0,
            },
          ],
        },
      ],
    },
  });

  await performUiAcceptanceAction({
    ...common,
    action: { type: "key", key: "Enter" },
  });
  assert.deepEqual(requests.at(-1).body.actions[0].actions, [
    { type: "keyDown", value: "\uE007" },
    { type: "keyUp", value: "\uE007" },
  ]);

  await performUiAcceptanceAction({
    ...common,
    action: { type: "key", key: "Space" },
  });
  assert.deepEqual(requests.at(-1).body.actions[0].actions, [
    { type: "keyDown", value: "\uE00D" },
    { type: "keyUp", value: "\uE00D" },
  ]);

  await performUiAcceptanceAction({
    ...common,
    action: { type: "input", selector: "#target", value: "600.0001" },
  });
  assert.deepEqual(requests.slice(-2), [
    {
      method: "POST",
      endpoint: "/session/session-1/element/element%3A%23target/clear",
      body: {},
    },
    {
      method: "POST",
      endpoint: "/session/session-1/element/element%3A%23target/value",
      body: { text: "600.0001" },
    },
  ]);
});

test("runner neutralizes pointer state before each captured surface", async () => {
  const { neutralizeUiAcceptancePointer } = await import(
    "./UiAcceptanceRunner.mjs"
  );
  const requests = [];
  await neutralizeUiAcceptancePointer({
    request: async (method, endpoint, body) => {
      requests.push({ body, endpoint, method });
    },
    sessionId: "session-1",
    viewport: { width: 1280, height: 720 },
  });

  assert.deepEqual(requests, [
    {
      body: undefined,
      endpoint: "/session/session-1/actions",
      method: "DELETE",
    },
    {
      endpoint: "/session/session-1/actions",
      method: "POST",
      body: {
        actions: [
          {
            type: "pointer",
            id: "acceptance-pointer",
            parameters: { pointerType: "mouse" },
            actions: [
              {
                type: "pointerMove",
                duration: 0,
                origin: "viewport",
                x: 2,
                y: 718,
              },
            ],
          },
        ],
      },
    },
  ]);
});

test("runner captures the declared surface instead of the whole viewport", async () => {
  const { captureUiAcceptanceScreenshot } = await import("./UiAcceptanceRunner.mjs");
  const requests = [];
  const request = async (method, endpoint) => {
    requests.push({ endpoint, method });
    return "c2NyZWVuc2hvdA==";
  };
  const screenshot = await captureUiAcceptanceScreenshot({
    captureSelector: ".media-panel",
    locateSelector: async () => "media-panel-id",
    request,
    sessionId: "session-1",
  });

  assert.equal(screenshot, "c2NyZWVuc2hvdA==");
  assert.deepEqual(requests, [
    {
      method: "GET",
      endpoint: "/session/session-1/element/media-panel-id/screenshot",
    },
  ]);
});

test("the report labels captures as unreviewed and includes every scenario", () => {
  const evidence = {
    collectedAtUtc: "2026-08-24T12:00:00.000Z",
    gitCommit: "abc123",
    sourceInputsDirty: true,
    captureStatus: "captured-unreviewed",
    scenarios: manifest.scenarios.map((scenario) => ({
      ...scenario,
      captureStatus: "captured-unreviewed",
      implementationUrl: `http://127.0.0.1:1437${scenario.implementationPath}`,
      implementationScreenshot: `screenshots/${scenario.id}-implementation.png`,
      ...(scenario.comparison.kind === "paired"
        ? {
            referenceUrl: `http://127.0.0.1:1437${scenario.referencePath}`,
            referenceScreenshot: `screenshots/${scenario.id}-reference.png`,
          }
        : {}),
    })),
  };
  const html = renderUiAcceptanceReport(evidence);
  assert.match(html, /Nenhuma captura foi aprovada automaticamente/);
  assert.match(html, /Capturado · não revisado/);
  assert.match(html, /Sem referência visual equivalente/);
  assert.match(html, /superfície: <code>media-panel<\/code>/);
  assert.match(html, /A referência visual vigente não representa o Modo de edição/);
  assert.doesNotMatch(html, /undefined/);
  for (const scenario of manifest.scenarios) assert.match(html, new RegExp(scenario.id));
});
