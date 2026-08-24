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
    assert.equal(existsSync(servedFilePath(workspace, scenario.referencePath)), true, `${scenario.id} reference is missing`);
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
      referenceUrl: `http://127.0.0.1:1437${scenario.referencePath}`,
      implementationScreenshot: `screenshots/${scenario.id}-implementation.png`,
      referenceScreenshot: `screenshots/${scenario.id}-reference.png`,
    })),
  };
  const html = renderUiAcceptanceReport(evidence);
  assert.match(html, /Nenhuma captura foi aprovada automaticamente/);
  assert.match(html, /Capturado · não revisado/);
  for (const scenario of manifest.scenarios) assert.match(html, new RegExp(scenario.id));
});
