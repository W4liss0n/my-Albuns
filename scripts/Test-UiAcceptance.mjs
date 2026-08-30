import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  finalizeUiAcceptanceSourceEvidence,
  renderUiAcceptanceReport,
  servedFilePath,
  validateUiAcceptanceManifest,
  validateUiAcceptanceReview,
} from "./UiAcceptance.mjs";

const scriptsDirectory = path.dirname(fileURLToPath(import.meta.url));
const workspace = path.resolve(scriptsDirectory, "..");
const manifestPath = path.join(workspace, "src", "test", "uiAcceptanceScenarios.json");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));

test("captured pointer gestures have one neutral W3C policy", async () => {
  const {
    buildCapturedPointerGestureActions,
    measureVisiblePointerGeometryScript,
    scrollIntoPointerViewportScript,
  } = await import("./WebDriverPointerGestures.mjs");

  assert.match(scrollIntoPointerViewportScript, /scrollIntoView/u);
  assert.match(measureVisiblePointerGeometryScript, /getBoundingClientRect/u);
  assert.match(measureVisiblePointerGeometryScript, /window\.innerWidth/u);
  assert.match(measureVisiblePointerGeometryScript, /sourceXRatio/u);
  assert.deepEqual(
    buildCapturedPointerGestureActions({
      phase: "preview",
      source: { x: 100, y: 200 },
      target: { x: 300, y: 220 },
    }),
    [
      {
        type: "pointerMove",
        duration: 0,
        origin: "viewport",
        x: 100,
        y: 200,
      },
      { type: "pointerDown", button: 0 },
      { type: "pause", duration: 80 },
      {
        type: "pointerMove",
        duration: 120,
        origin: "viewport",
        x: 110,
        y: 201,
      },
      {
        type: "pointerMove",
        duration: 450,
        origin: "viewport",
        x: 300,
        y: 220,
      },
      { type: "pause", duration: 100 },
    ],
  );

  const dropActions = buildCapturedPointerGestureActions({
    dropTarget: { x: 400, y: 240 },
    phase: "drop",
    source: { x: 100, y: 200 },
    target: { x: 300, y: 220 },
  });
  assert.deepEqual(
    dropActions.slice(-3),
    [
      {
        type: "pointerMove",
        duration: 450,
        origin: "viewport",
        x: 400,
        y: 240,
      },
      { type: "pause", duration: 100 },
      { type: "pointerUp", button: 0 },
    ],
  );

  for (const runnerName of [
    "Run-ProductiveJourneyGate.mjs",
    "UiAcceptanceRunner.mjs",
  ]) {
    const runner = readFileSync(path.join(scriptsDirectory, runnerName), "utf8");
    assert.match(runner, /from "\.\/WebDriverPointerGestures\.mjs"/u);
    assert.doesNotMatch(runner, /const visibleCenter =/u);
  }
});

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
    "canvas-scrollbar-focus",
    "canvas-scrollbar-hover",
    "canvas-scrollbar-idle",
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

test("the manifest covers the integrated workspace and every critical Project dialog state", () => {
  const scenariosById = new Map(
    manifest.scenarios.map((scenario) => [scenario.id, scenario]),
  );
  const workspaceScenario = scenariosById.get("project-workspace-integrated");
  assert.equal(workspaceScenario?.implementationPath, "/workspace-preview.html");
  assert.equal(workspaceScenario?.comparison.surface, "project-workspace");
  assert.equal(workspaceScenario?.comparison.kind, "paired");
  assert.equal(
    existsSync(path.join(workspace, "src", "workspace-preview.tsx")),
    true,
    "the integrated workspace entrypoint is missing",
  );

  const workspaceEntrypoint = readFileSync(
    path.join(workspace, "src", "workspace-preview.tsx"),
    "utf8",
  );
  assert.match(
    workspaceEntrypoint,
    /import App from "\.\/App";/u,
    "the integrated workspace must use the production App composition",
  );
  assert.match(
    workspaceEntrypoint,
    /<App\s/u,
    "the integrated workspace must render the production App composition",
  );

  const criticalStates = {
    "album-information-confirmation-single-change": {
      busy: false,
      kind: "albumInformationConfirmation",
    },
    "album-information-confirmation-busy": {
      busy: true,
      kind: "albumInformationConfirmation",
    },
    "project-close-confirmation": {
      busy: false,
      kind: "projectCloseConfirmation",
    },
    "project-close-confirmation-busy": {
      busy: true,
      kind: "projectCloseConfirmation",
    },
    "project-close-failure": { kind: "projectCloseFailure" },
    "project-graphics-failure": { kind: "graphicsFailure" },
    "export-progress-determinate": {
      cancelRequested: false,
      cancellable: true,
      kind: "exportProgress",
      progressKind: "determinate",
    },
    "export-progress-cancel-requested": {
      cancelRequested: true,
      cancellable: true,
      kind: "exportProgress",
      progressKind: "indeterminate",
    },
    "export-progress-non-cancellable": {
      cancelRequested: false,
      cancellable: false,
      kind: "exportProgress",
      progressKind: "determinate",
    },
    "export-failure-retryable": {
      cancelled: false,
      kind: "exportFailure",
      retryDisabled: false,
    },
    "export-failure-cancelled": {
      cancelled: true,
      kind: "exportFailure",
      retryDisabled: true,
    },
  };
  for (const [id, expected] of Object.entries(criticalStates)) {
    const scenario = scenariosById.get(id);
    assert.ok(scenario, `${id} is missing`);
    assert.match(scenario.implementationPath, /^\/project-dialog\.html\?state=/u);
    assert.equal(scenario.comparison.kind, "implementation-only");
    assert.equal(scenario.comparison.surface.startsWith("owned-"), true);

    const state = JSON.parse(
      new URL(scenario.implementationPath, "http://127.0.0.1").searchParams.get(
        "state",
      ),
    );
    assert.equal(state.kind, expected.kind, `${id} captures the wrong state kind`);
    for (const modifier of [
      "busy",
      "cancelRequested",
      "cancelled",
      "cancellable",
      "retryDisabled",
    ]) {
      if (modifier in expected) {
        assert.equal(
          state[modifier],
          expected[modifier],
          `${id} captures the wrong ${modifier} modifier`,
        );
      }
    }
    if ("progressKind" in expected) {
      assert.equal(
        state.progress?.kind,
        expected.progressKind,
        `${id} captures the wrong progress variant`,
      );
    }
  }

  const capturedDialogKinds = new Set(
    manifest.scenarios
      .filter((scenario) => scenario.implementationPath.startsWith("/project-dialog.html?state="))
      .map((scenario) =>
        JSON.parse(
          new URL(
            scenario.implementationPath,
            "http://127.0.0.1",
          ).searchParams.get("state"),
        ).kind,
      ),
  );
  assert.deepEqual(
    [...capturedDialogKinds].sort(),
    [
      "albumInformationConfirmation",
      "exportFailure",
      "exportProgress",
      "exportSuccess",
      "graphicsFailure",
      "projectCloseConfirmation",
      "projectCloseFailure",
      "projectOperationFailure",
    ],
  );
});

test("the manifest covers critical integrated workspace, panel, menu, and graphics states", () => {
  const scenariosById = new Map(
    manifest.scenarios.map((scenario) => [scenario.id, scenario]),
  );
  const expectedStates = {
    "project-workspace-integrated": {
      path: "/workspace-preview.html",
      ready: /Informações do Álbum/u,
    },
    "project-workspace-sheet-context": {
      path: "/workspace-preview.html",
      ready: /Design da Lâmina/u,
      actions: ["focus", "key"],
    },
    "project-workspace-photo-context": {
      path: "/workspace-preview.html?frame=photo",
      ready: /Zoom da Foto/u,
      actions: ["focus", "key", "click"],
    },
    "project-workspace-frame-placeholder-context": {
      path: "/workspace-preview.html?frame=empty",
      ready: /context-heading/u,
      actions: ["focus", "key", "click"],
    },
    "project-workspace-menu-open": {
      path: "/workspace-preview.html",
      ready: /application-menu-file/u,
      actions: ["click"],
    },
    "project-workspace-panels-persisted": {
      path: "/workspace-preview.html?layout=persisted",
      ready: /inspector-width/u,
    },
    "project-workspace-panels-collapsed": {
      path: "/workspace-preview.html?layout=collapsed",
      ready: /not\(:has/u,
    },
    "project-graphics-failure": {
      path: /^\/project-dialog\.html\?state=/u,
      ready: /ui-owned-window-shell/u,
    },
    "safe-application-shell": {
      path: "/welcome-preview.html?graphics=unsupported",
      ready: /safe-shell/u,
    },
  };

  for (const [id, expected] of Object.entries(expectedStates)) {
    const scenario = scenariosById.get(id);
    assert.ok(scenario, `${id} is missing`);
    if (expected.path instanceof RegExp) {
      assert.match(scenario.implementationPath, expected.path);
    } else {
      assert.equal(scenario.implementationPath, expected.path);
    }
    assert.match(scenario.readySelector, expected.ready);
    if (expected.actions) {
      assert.deepEqual(
        scenario.actions.map((action) => action.type),
        expected.actions,
        `${id} must exercise the declared user transition`,
      );
    }
  }
});

test("the manifest captures rendered structural command surfaces at the physical minimum", () => {
  const scenariosById = new Map(
    manifest.scenarios.map((scenario) => [scenario.id, scenario]),
  );
  const applicationMenu = scenariosById.get("sheet-structure-application-menu");
  const contextMenu = scenariosById.get("sheet-structure-context-menu");

  for (const scenario of [applicationMenu, contextMenu]) {
    assert.ok(scenario, "the structural command surface is missing");
    assert.equal(
      scenario.implementationPath,
      "/workspace-preview.html?structure=minimum-single-edges",
    );
    assert.equal(scenario.comparison.kind, "implementation-only");
    assert.equal(scenario.comparison.surface, "sheet-structure-commands");
    assert.equal(
      scenario.comparison.implementationCaptureSelector,
      ".app-shell",
    );
    assert.match(scenario.comparison.reason, /referência visual vigente/i);
    assert.match(scenario.readySelector, /disabled/u);
  }

  assert.deepEqual(applicationMenu.actions, [
    {
      type: "click",
      selector: '[aria-controls="application-menu-sheet"]',
    },
  ]);
  assert.match(applicationMenu.readySelector, /application-menu-sheet/u);
  assert.match(applicationMenu.readySelector, /Adicionar antes/u);
  assert.match(applicationMenu.readySelector, /Excluir/u);

  assert.deepEqual(contextMenu.actions, [
    {
      type: "context-click",
      selector:
        '[data-reorder-surface="bar"] [data-sheet-id="sheet-001"]',
    },
  ]);
  assert.match(contextMenu.readySelector, /Ações da Lâmina 01/u);
  assert.match(contextMenu.readySelector, /data-centered-sheet-id="sheet-001"/u);
  assert.match(contextMenu.readySelector, /data-viewport-offset-x="0"/u);
  assert.match(contextMenu.readySelector, /nth-of-type\(1\)/u);
  assert.match(contextMenu.readySelector, /nth-of-type\(4\)/u);
});

test("the manifest captures the rendered Canvas scrollbar at idle, hover, and focus", () => {
  const scenariosById = new Map(
    manifest.scenarios.map((scenario) => [scenario.id, scenario]),
  );
  const expectedActions = {
    "canvas-scrollbar-idle": [],
    "canvas-scrollbar-hover": ["hover"],
    "canvas-scrollbar-focus": ["focus"],
  };

  for (const [id, actionTypes] of Object.entries(expectedActions)) {
    const scenario = scenariosById.get(id);
    assert.ok(scenario, `${id} is missing`);
    assert.equal(scenario.readySelector, ".canvas-horizontal-scrollbar__thumb");
    assert.equal(scenario.comparison.surface, "continuous-canvas-scrollbar");
    assert.deepEqual(
      scenario.actions.map((action) => action.type),
      actionTypes,
    );
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

test("the manifest preserves Program 05 proofs and promotes Sheet reordering to the productive workspace", () => {
  const expectedScenarios = {
    "ui-architecture-map": [],
    "editor-zoom-keyboard-in": ["focus", "key"],
    "editor-zoom-keyboard-out": ["focus", "key", "key", "key"],
    "editor-zoom-wheel": ["wheel"],
    "editor-zoom-reset": ["focus", "key", "key", "key"],
    "editor-zoom-cap": [
      "focus",
      ...Array.from({ length: 12 }, () => "key"),
    ],
    "sheet-reorder-bar-preview": ["focus", "key", "assert", "drag"],
    "sheet-reorder-bar-commit": ["focus", "key", "assert", "drag"],
    "sheet-reorder-grid-preview": ["drag"],
    "sheet-reorder-grid-commit": ["drag"],
    "sheet-reorder-cancelled": ["focus", "key", "assert", "drag"],
    "sheet-reorder-cross-surface-cancelled": [
      "focus",
      "key",
      "assert",
      "drag",
    ],
    "sheet-reorder-invalid-drop": ["drag"],
    "sheet-reorder-invalid-target-preview": ["drag"],
    "frame-multi-selection-mixed": ["click", "click"],
    "frame-multi-selection-absolute-edit": ["click", "click", "input"],
    "frame-manipulation-move": ["click", "drag"],
    "frame-manipulation-resize": ["click", "drag"],
    "frame-layout-locked": ["click", "click", "drag"],
  };
  const scenariosById = new Map(
    manifest.scenarios.map((scenario) => [scenario.id, scenario]),
  );
  const productiveReorderIds = new Set([
    "sheet-reorder-bar-preview",
    "sheet-reorder-bar-commit",
    "sheet-reorder-grid-preview",
    "sheet-reorder-grid-commit",
    "sheet-reorder-cancelled",
    "sheet-reorder-cross-surface-cancelled",
    "sheet-reorder-invalid-drop",
    "sheet-reorder-invalid-target-preview",
  ]);

  assert.equal(manifest.scenarios.length, 58 + Object.keys(expectedScenarios).length);
  for (const [id, actionTypes] of Object.entries(expectedScenarios)) {
    const scenario = scenariosById.get(id);
    assert.ok(scenario, `${id} is missing`);
    const productive = productiveReorderIds.has(id);
    assert.equal(
      scenario.implementationPath,
      productive
        ? "/workspace-preview.html?structure=physical"
        : id === "ui-architecture-map"
          ? "/ui-architecture-prototype.html"
          : "/ui-architecture-prototype.html?view=editor",
      `${id} uses the wrong acceptance surface`,
    );
    assert.equal(scenario.comparison.kind, "implementation-only");
    assert.match(scenario.comparison.reason, /referência visual vigente/i);
    assert.equal(
      scenario.comparison.implementationCaptureSelector,
      productive ? ".app-shell" : ".ui-architecture-prototype",
    );
    assert.deepEqual(
      scenario.actions.map((action) => action.type),
      actionTypes,
      `${id} exercises the wrong gesture sequence`,
    );
    assert.match(scenario.readySelector, /data-/u);
  }

  const keyboardZoom = scenariosById.get("editor-zoom-keyboard-in");
  assert.deepEqual(keyboardZoom.actions.at(-1), {
    type: "key",
    key: "Plus",
    modifiers: ["Control"],
  });
  const wheelZoom = scenariosById.get("editor-zoom-wheel");
  assert.deepEqual(wheelZoom.actions[0], {
    type: "wheel",
    selector: '[data-frame-gesture-target="resize"]',
    deltaY: -120,
    modifiers: ["Control"],
  });
  const mixedSelection = scenariosById.get("frame-multi-selection-mixed");
  assert.deepEqual(mixedSelection.actions[1].modifiers, ["Control"]);
  for (const id of productiveReorderIds) {
    const drag = scenariosById
      .get(id)
      .actions.find((action) => action.type === "drag");
    assert.equal(drag.gesture, "pointer", `${id} must use pointer capture`);
  }
  assert.deepEqual(
    [
      "sheet-reorder-bar-preview",
      "sheet-reorder-bar-commit",
    ].map((id) =>
      scenariosById
        .get(id)
        .actions.find((action) => action.type === "drag").sourceXRatio,
    ),
    [0.25, 0.75],
  );
  for (const id of [
    "sheet-reorder-bar-preview",
    "sheet-reorder-grid-preview",
    "sheet-reorder-invalid-target-preview",
  ]) {
    assert.equal(scenariosById.get(id).actions.at(-1).phase, "preview");
  }
  assert.equal(
    scenariosById.get("sheet-reorder-cancelled").actions.at(-1).phase,
    "escape",
  );
  for (const id of [
    "sheet-reorder-bar-commit",
    "sheet-reorder-grid-commit",
    "sheet-reorder-cross-surface-cancelled",
    "sheet-reorder-invalid-drop",
    "frame-manipulation-move",
    "frame-manipulation-resize",
    "frame-layout-locked",
  ]) {
    assert.equal(scenariosById.get(id).actions.at(-1).phase, "drop");
  }
  for (const id of [
    "sheet-reorder-bar-preview",
    "sheet-reorder-bar-commit",
    "sheet-reorder-grid-preview",
    "sheet-reorder-grid-commit",
    "sheet-reorder-cancelled",
    "sheet-reorder-cross-surface-cancelled",
    "sheet-reorder-invalid-drop",
    "sheet-reorder-invalid-target-preview",
  ]) {
    assert.equal(
      scenariosById.get(id).implementationPath,
      "/workspace-preview.html?structure=physical",
    );
    assert.match(scenariosById.get(id).readySelector, /data-reorder-surface/u);
  }
  for (const id of [
    "sheet-reorder-bar-preview",
    "sheet-reorder-grid-preview",
    "sheet-reorder-invalid-target-preview",
  ]) {
    assert.match(scenariosById.get(id).readySelector, /^\.app-shell:has/u);
  }
  assert.match(
    scenariosById.get("sheet-reorder-bar-preview").readySelector,
    /reorder-ghost.*data-active-sides.*sheet-preview/u,
  );
  assert.match(
    scenariosById.get("sheet-reorder-grid-preview").readySelector,
    /reorder-ghost.*data-active-sides.*sheet-preview/u,
  );

  const cancelled = scenariosById.get("sheet-reorder-cancelled");
  assert.match(cancelled.readySelector, /data-reorder-state="cancelled"/u);
  assert.match(cancelled.readySelector, /data-preview-order=/u);
  assert.match(cancelled.readySelector, /:not\(:has\([^)]*reorder-ghost/u);
  assert.match(cancelled.readySelector, /:not\(:has\([^)]*reorder-placeholder/u);

  const invalidDrop = scenariosById.get("sheet-reorder-invalid-drop");
  const invalidTargetPreview = scenariosById.get(
    "sheet-reorder-invalid-target-preview",
  );
  for (const scenario of [invalidDrop, invalidTargetPreview]) {
    assert.deepEqual(
      {
        ...scenario.actions[0],
        phase: undefined,
      },
      {
        type: "drag",
        gesture: "pointer",
        selector:
          '[data-reorder-surface="grid"] [data-sheet-id="sheet-003"]',
        targetSelector:
          '[data-reorder-surface="grid"] [data-sheet-id="sheet-001"]',
        phase: undefined,
      },
    );
  }
  assert.match(invalidDrop.readySelector, /data-reorder-state="cancelled"/u);
  assert.match(invalidDrop.readySelector, /data-preview-order=/u);
  assert.match(invalidDrop.readySelector, /:not\(:has\([^)]*reorder-ghost/u);
  assert.match(
    invalidDrop.readySelector,
    /:not\(:has\([^)]*reorder-placeholder/u,
  );
  assert.match(
    invalidTargetPreview.readySelector,
    /data-reorder-state="invalid"/u,
  );
  assert.match(invalidTargetPreview.readySelector, /data-reorder-ghost="true"/u);
  assert.match(invalidTargetPreview.readySelector, /data-testid="reorder-ghost"/u);
  assert.match(
    invalidTargetPreview.readySelector,
    /data-reorder-invalid-indicator/u,
  );
  assert.match(
    invalidTargetPreview.readySelector,
    /:not\(:has\([^)]*reorder-placeholder/u,
  );
  assert.match(invalidTargetPreview.readySelector, /data-preview-order=/u);
});

test("the manifest exercises this UI correction batch in the real renderer", () => {
  const scenariosById = new Map(
    manifest.scenarios.map((scenario) => [scenario.id, scenario]),
  );
  for (const id of [
    "project-recovery-modal",
    "external-copy-opening-dialog",
    "project-graphics-failure",
    "project-text-selection-policy",
    "canvas-keyboard-next-sheet",
    "canvas-keyboard-previous-sheet",
    "canvas-context-menu-surface-preserves-viewport",
    "sheet-bar-selection-preserves-navigation",
    "sheet-context-menu-outside-bar-click-preserves-navigation",
    "sheet-context-menu-outside-grid-click-preserves-navigation",
    "canvas-wheel-sheet-bar-forward",
    "canvas-wheel-sheet-bar-backward",
    "project-splitters-normal-100",
    "project-splitter-horizontal-hover-125",
    "project-splitter-vertical-focus-150",
    "project-splitters-resized",
  ]) {
    assert.ok(scenariosById.has(id), `${id} is missing`);
  }
  assert.match(
    scenariosById.get("project-recovery-modal").readySelector,
    /aria-modal/u,
  );
  assert.match(
    scenariosById.get("project-recovery-modal").readySelector,
    /ui-owned-window-shell/u,
  );
  assert.match(
    scenariosById.get("project-recovery-modal").readySelector,
    /ui-application-header__status/u,
  );
  assert.match(
    scenariosById.get("project-recovery-modal").implementationPath,
    /^\/dialog\.html\?kind=project-recovery/u,
  );
  assert.deepEqual(
    scenariosById.get("project-recovery-modal").viewport,
    { width: 492, height: 320 },
  );
  assert.deepEqual(
    scenariosById
      .get("project-recovery-modal")
      .actions.map((action) => action.type),
    ["assert-single-line", "assert-single-line", "assert-single-line"],
  );
  assert.match(
    scenariosById.get("external-copy-opening-dialog").implementationPath,
    /^\/dialog\.html\?kind=external-copy/u,
  );
  assert.deepEqual(
    scenariosById.get("external-copy-opening-dialog").viewport,
    { width: 440, height: 320 },
  );
  assert.deepEqual(
    scenariosById
      .get("external-copy-opening-dialog")
      .actions.map((action) => action.type),
    ["assert-single-line", "assert-single-line"],
  );
  assert.match(
    scenariosById.get("project-graphics-failure").implementationPath,
    /^\/project-dialog\.html\?state=/u,
  );
  assert.match(
    scenariosById.get("project-graphics-failure").readySelector,
    /aria-modal/u,
  );
  assert.deepEqual(
    scenariosById
      .get("sheet-bar-selection-preserves-navigation")
      .actions.map((action) => action.type),
    ["assert", "focus", "key", "assert", "pointer-click", "assert", "assert"],
  );
  assert.equal(
    scenariosById
      .get("sheet-bar-selection-preserves-navigation")
      .actions[2].key,
    "ArrowRight",
  );
  assert.match(
    scenariosById.get("sheet-bar-selection-preserves-navigation").readySelector,
    /aria-current/u,
  );
  for (const id of [
    "canvas-keyboard-next-sheet",
    "canvas-keyboard-previous-sheet",
  ]) {
    assert.match(scenariosById.get(id).readySelector, /canvas-sheet-navigation/u);
  }
  for (const id of [
    "canvas-wheel-sheet-bar-forward",
    "canvas-wheel-sheet-bar-backward",
  ]) {
    assert.equal(scenariosById.get(id).actions.at(-1).type, "wheel");
    assert.match(
      scenariosById.get(id).actions.at(-1).selector,
      /data-reorder-surface=.*bar/su,
    );
  }
  assert.deepEqual(
    [
      scenariosById.get("project-splitters-normal-100").implementationPath,
      scenariosById.get("project-splitter-horizontal-hover-125")
        .implementationPath,
      scenariosById.get("project-splitter-vertical-focus-150")
        .implementationPath,
    ],
    [
      "/workspace-preview.html?scale=1",
      "/workspace-preview.html?scale=1.25",
      "/workspace-preview.html?scale=1.5",
    ],
  );
  assert.deepEqual(
    scenariosById.get("project-text-selection-policy").actions.map(
      ({ expect, type }) => ({ expect, type }),
    ),
    [
      { expect: "none", type: "selection-drag" },
      { expect: "control", type: "selection-drag" },
    ],
  );
  assert.deepEqual(
    scenariosById.get("new-project-operational-failure-dialog").actions,
    [
      {
        expect: "text",
        selector: ".ui-standard-message__description.ui-copyable-text",
        type: "selection-drag",
      },
    ],
  );
});

test("manifest schema 3 accepts real context click, hit-tested click, modifier, wheel, and drag actions", () => {
  const interactive = structuredClone(manifest);
  interactive.schemaVersion = 3;
  interactive.scenarios[0].actions = [
    { type: "context-click", selector: "#sheet" },
    { type: "pointer-click", selector: "#sheet" },
    { type: "key", key: "Plus", modifiers: ["Control"] },
    { type: "click", selector: "#frame", modifiers: ["Control"] },
    {
      type: "wheel",
      selector: "#canvas",
      deltaY: -120,
      modifiers: ["Control"],
    },
    {
      type: "drag",
      gesture: "pointer",
      selector: "#source",
      sourceXRatio: 0.25,
      targetSelector: "#target",
      dropTargetSelector: "#opposite-surface",
      phase: "drop",
    },
    {
      type: "drag",
      gesture: "pointer",
      selector: "#source",
      targetSelector: "#target",
      phase: "escape",
    },
    { type: "selection-drag", selector: "#label", expect: "none" },
    { type: "assert-single-line", selector: "#dialog-action" },
  ];

  assert.equal(validateUiAcceptanceManifest(interactive), interactive);

  const unknownModifier = structuredClone(interactive);
  unknownModifier.scenarios[0].actions[2].modifiers = ["Alt"];
  assert.throws(
    () => validateUiAcceptanceManifest(unknownModifier),
    /modifier.*not supported/u,
  );

  const duplicateModifier = structuredClone(interactive);
  duplicateModifier.scenarios[0].actions[2].modifiers = [
    "Control",
    "Control",
  ];
  assert.throws(
    () => validateUiAcceptanceManifest(duplicateModifier),
    /modifier.*duplicates/u,
  );

  const zeroWheel = structuredClone(interactive);
  zeroWheel.scenarios[0].actions[4].deltaY = 0;
  assert.throws(
    () => validateUiAcceptanceManifest(zeroWheel),
    /deltaY.*non-zero integer/u,
  );

  const invalidDrag = structuredClone(interactive);
  invalidDrag.scenarios[0].actions[5].phase = "hover";
  assert.throws(
    () => validateUiAcceptanceManifest(invalidDrag),
    /phase.*preview, drop, or escape/u,
  );

  const invalidDropTarget = structuredClone(interactive);
  invalidDropTarget.scenarios[0].actions[5].dropTargetSelector = "";
  assert.throws(
    () => validateUiAcceptanceManifest(invalidDropTarget),
    /dropTargetSelector.*non-empty string/u,
  );

  const invalidGesture = structuredClone(interactive);
  invalidGesture.scenarios[0].actions[5].gesture = "native";
  assert.throws(
    () => validateUiAcceptanceManifest(invalidGesture),
    /gesture.*pointer/u,
  );

  const invalidSourceRatio = structuredClone(interactive);
  invalidSourceRatio.scenarios[0].actions[5].sourceXRatio = 1;
  assert.throws(
    () => validateUiAcceptanceManifest(invalidSourceRatio),
    /sourceXRatio.*between 0 and 1/u,
  );

  const ratioWithoutPointerCapture = structuredClone(interactive);
  delete ratioWithoutPointerCapture.scenarios[0].actions[5].gesture;
  assert.throws(
    () => validateUiAcceptanceManifest(ratioWithoutPointerCapture),
    /sourceXRatio.*gesture pointer/u,
  );

  const escapeWithoutPointerCapture = structuredClone(interactive);
  delete escapeWithoutPointerCapture.scenarios[0].actions[6].gesture;
  assert.throws(
    () => validateUiAcceptanceManifest(escapeWithoutPointerCapture),
    /phase escape.*pointer/u,
  );

  const invalidSelectionExpectation = structuredClone(interactive);
  invalidSelectionExpectation.scenarios[0].actions[7].expect = "native";
  assert.throws(
    () => validateUiAcceptanceManifest(invalidSelectionExpectation),
    /expect must be control, none, or text/u,
  );
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
    return script.includes("document.activeElement")
      ? { "element-6066-11e4-a52e-4f735466cecf": "element:#target" }
      : true;
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
  assert.deepEqual(requests.at(-1), {
    method: "POST",
    endpoint: "/session/session-1/element/element%3A%23target/value",
    body: { text: "\uE007", value: ["\uE007"] },
  });

  await performUiAcceptanceAction({
    ...common,
    action: { type: "key", key: "Space" },
  });
  assert.deepEqual(requests.at(-1).body, {
    text: "\uE00D",
    value: ["\uE00D"],
  });

  await performUiAcceptanceAction({
    ...common,
    action: { type: "key", key: "Escape" },
  });
  assert.deepEqual(requests.at(-1).body, {
    text: "\uE00C",
    value: ["\uE00C"],
  });

  await performUiAcceptanceAction({
    ...common,
    action: { type: "key", key: "ArrowRight" },
  });
  assert.deepEqual(requests.slice(-2), [
    {
      method: "POST",
      endpoint: "/session/session-1/ms/cdp/execute",
      body: {
        cmd: "Input.dispatchKeyEvent",
        params: {
          code: "ArrowRight",
          key: "ArrowRight",
          nativeVirtualKeyCode: 39,
          type: "rawKeyDown",
          windowsVirtualKeyCode: 39,
        },
      },
    },
    {
      method: "POST",
      endpoint: "/session/session-1/ms/cdp/execute",
      body: {
        cmd: "Input.dispatchKeyEvent",
        params: {
          code: "ArrowRight",
          key: "ArrowRight",
          nativeVirtualKeyCode: 39,
          type: "keyUp",
          windowsVirtualKeyCode: 39,
        },
      },
    },
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

test("runner emits real W3C secondary-button and hit-tested primary clicks", async () => {
  const { performUiAcceptanceAction } = await import(
    "./UiAcceptanceRunner.mjs"
  );
  const requests = [];

  await performUiAcceptanceAction({
    action: { type: "context-click", selector: "#sheet" },
    execute: async () => true,
    locateSelector: async (selector) => `element:${selector}`,
    locateText: async (text) => `text:${text}`,
    request: async (method, endpoint, body) => {
      requests.push({ body, endpoint, method });
      return null;
    },
    sessionId: "session-context-click",
  });

  await performUiAcceptanceAction({
    action: { type: "pointer-click", selector: "#underlay" },
    execute: async () => true,
    locateSelector: async (selector) => `element:${selector}`,
    locateText: async (text) => `text:${text}`,
    request: async (method, endpoint, body) => {
      requests.push({ body, endpoint, method });
      return null;
    },
    sessionId: "session-context-click",
  });

  assert.deepEqual(requests, [
    {
      method: "POST",
      endpoint: "/session/session-context-click/actions",
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
                  "element-6066-11e4-a52e-4f735466cecf": "element:#sheet",
                },
                x: 0,
                y: 0,
              },
              { type: "pointerDown", button: 2 },
              { type: "pointerUp", button: 2 },
            ],
          },
        ],
      },
    },
    {
      method: "POST",
      endpoint: "/session/session-context-click/actions",
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
                  "element-6066-11e4-a52e-4f735466cecf": "element:#underlay",
                },
                x: 0,
                y: 0,
              },
              { type: "pointerDown", button: 0 },
              { type: "pointerUp", button: 0 },
            ],
          },
        ],
      },
    },
  ]);
});

test("runner emits W3C actions for Ctrl gestures, wheel, and preview or committed drag", async () => {
  const {
    assertBrowserZoomUnchanged,
    performUiAcceptanceAction,
    requiresBrowserZoomInvariant,
  } = await import("./UiAcceptanceRunner.mjs");
  const requests = [];
  const common = {
    execute: async () => true,
    locateSelector: async (selector) => `element:${selector}`,
    locateText: async (text) => `text:${text}`,
    request: async (method, endpoint, body) => {
      requests.push({ body, endpoint, method });
      return null;
    },
    sessionId: "session-gestures",
  };

  await performUiAcceptanceAction({
    ...common,
    action: { type: "key", key: "Plus", modifiers: ["Control"] },
  });
  assert.deepEqual(requests.at(-1).body.actions[0].actions, [
    { type: "keyDown", value: "\uE009" },
    { type: "keyDown", value: "\uE025" },
    { type: "keyUp", value: "\uE025" },
    { type: "keyUp", value: "\uE009" },
  ]);

  await performUiAcceptanceAction({
    ...common,
    action: { type: "click", selector: "#frame", modifiers: ["Control"] },
  });
  const ctrlClick = requests.at(-1).body.actions;
  assert.deepEqual(ctrlClick[0].actions, [
    { type: "keyDown", value: "\uE009" },
    { type: "pause", duration: 0 },
    { type: "pause", duration: 0 },
    { type: "pause", duration: 0 },
    { type: "keyUp", value: "\uE009" },
  ]);
  assert.equal(ctrlClick[1].type, "pointer");
  assert.deepEqual(
    ctrlClick[1].actions.map((action) => action.type),
    ["pause", "pointerMove", "pointerDown", "pointerUp", "pause"],
  );

  await performUiAcceptanceAction({
    ...common,
    action: {
      type: "wheel",
      selector: "#canvas",
      deltaY: -120,
      modifiers: ["Control"],
    },
  });
  const ctrlWheel = requests.at(-1).body.actions;
  assert.equal(ctrlWheel[1].type, "wheel");
  assert.deepEqual(ctrlWheel[1].actions[1], {
    type: "scroll",
    duration: 0,
    origin: {
      "element-6066-11e4-a52e-4f735466cecf": "element:#canvas",
    },
    x: 0,
    y: 0,
    deltaX: 0,
    deltaY: -120,
  });
  assert.equal(
    requiresBrowserZoomInvariant([
      {
        type: "wheel",
        selector: "#canvas",
        deltaY: -120,
        modifiers: ["Control"],
      },
    ]),
    true,
  );
  assert.equal(
    requiresBrowserZoomInvariant([
      { type: "wheel", selector: "#canvas", deltaY: -120 },
      { type: "key", key: "Plus", modifiers: ["Control"] },
    ]),
    false,
  );
  const browserZoom = {
    devicePixelRatio: 1,
    innerHeight: 900,
    innerWidth: 1440,
    visualViewportScale: 1,
  };
  assert.doesNotThrow(() =>
    assertBrowserZoomUnchanged({
      after: { ...browserZoom },
      before: browserZoom,
      label: "editor-zoom-wheel implementation",
    }),
  );
  assert.throws(
    () =>
      assertBrowserZoomUnchanged({
        after: {
          ...browserZoom,
          devicePixelRatio: 1.25,
          innerWidth: 1152,
        },
        before: browserZoom,
        label: "editor-zoom-wheel implementation",
      }),
    /editor-zoom-wheel implementation changed browser zoom.*devicePixelRatio.*innerWidth/u,
  );

  await performUiAcceptanceAction({
    ...common,
    action: {
      type: "drag",
      selector: "#source",
      targetSelector: "#target",
      phase: "preview",
    },
  });
  assert.deepEqual(
    requests.at(-1).body.actions[0].actions.map((action) => action.type),
    ["pointerMove", "pointerDown", "pointerMove"],
  );

  await performUiAcceptanceAction({
    ...common,
    action: {
      type: "drag",
      selector: "#source",
      targetSelector: "#target",
      dropTargetSelector: "#opposite-surface",
      phase: "drop",
    },
  });
  assert.deepEqual(
    requests.at(-1).body.actions[0].actions.map((action) => action.type),
    [
      "pointerMove",
      "pointerDown",
      "pointerMove",
      "pointerMove",
      "pointerUp",
    ],
  );
  assert.deepEqual(requests.at(-1).body.actions[0].actions.at(-2), {
    type: "pointerMove",
    duration: 220,
    origin: {
      "element-6066-11e4-a52e-4f735466cecf":
        "element:#opposite-surface",
    },
    x: 0,
    y: 0,
  });
});

test("runner initiates declared pointer capture through the proven WebDriver sequence", async () => {
  const { performUiAcceptanceAction } = await import(
    "./UiAcceptanceRunner.mjs"
  );
  const requests = [];
  const executions = [];
  await performUiAcceptanceAction({
    action: {
      type: "drag",
      gesture: "pointer",
      selector: "#source",
      sourceXRatio: 0.25,
      targetSelector: "#target",
      phase: "preview",
    },
    execute: async (script, args) => {
      executions.push({ args, script });
      if (script.includes("getBoundingClientRect")) {
        return {
          source: { x: 100, y: 200 },
          target: { x: 300, y: 220 },
          dropTarget: null,
        };
      }
      return true;
    },
    locateSelector: async (selector) => `element:${selector}`,
    locateText: async (text) => `text:${text}`,
    request: async (method, endpoint, body) => {
      requests.push({ body, endpoint, method });
      return null;
    },
    sessionId: "session-pointer",
  });

  assert.match(executions[0].script, /scrollIntoView/u);
  assert.deepEqual(executions[0].args, [
    {
      "element-6066-11e4-a52e-4f735466cecf": "element:#source",
    },
  ]);
  assert.match(executions[1].script, /getBoundingClientRect/u);
  assert.equal(executions[1].args[3], 0.25);
  assert.deepEqual(
    requests.at(-1).body.actions[0].actions,
    [
      {
        type: "pointerMove",
        duration: 0,
        origin: "viewport",
        x: 100,
        y: 200,
      },
      { type: "pointerDown", button: 0 },
      { type: "pause", duration: 80 },
      {
        type: "pointerMove",
        duration: 120,
        origin: "viewport",
        x: 110,
        y: 201,
      },
      {
        type: "pointerMove",
        duration: 450,
        origin: "viewport",
        x: 300,
        y: 220,
      },
      { type: "pause", duration: 100 },
    ],
  );
});

test("runner verifies native selection policy through real pointer motion", async () => {
  const { performUiAcceptanceAction } = await import(
    "./UiAcceptanceRunner.mjs"
  );
  const requests = [];
  let executionCount = 0;
  await performUiAcceptanceAction({
    action: {
      type: "selection-drag",
      selector: "#chrome-label",
      expect: "none",
    },
    execute: async () => {
      executionCount += 1;
      return executionCount === 1
        ? { startX: -40, startY: -5, endX: 40, endY: 5 }
        : { controlSelection: 0, documentSelection: 0 };
    },
    locateSelector: async (selector) => `element:${selector}`,
    locateText: async (text) => `text:${text}`,
    request: async (method, endpoint, body) => {
      requests.push({ body, endpoint, method });
      return null;
    },
    sessionId: "session-selection",
  });

  assert.deepEqual(
    requests[0].body.actions[0].actions.map((action) => action.type),
    ["pointerMove", "pointerDown", "pointerMove", "pointerUp"],
  );
  assert.deepEqual(
    requests[0].body.actions[0].actions
      .filter((action) => action.type === "pointerMove")
      .map(({ x, y }) => ({ x, y })),
    [
      { x: -40, y: -5 },
      { x: 40, y: 5 },
    ],
  );
  assert.equal(executionCount, 2);
});

test("runner rejects a dialog action whose rendered label occupies more than one line", async () => {
  const { performUiAcceptanceAction } = await import(
    "./UiAcceptanceRunner.mjs"
  );
  const common = {
    action: {
      type: "assert-single-line",
      selector: "#dialog-action",
    },
    locateSelector: async (selector) => `element:${selector}`,
    locateText: async (text) => `text:${text}`,
    request: async () => null,
    sessionId: "session-dialog-geometry",
  };

  await assert.rejects(
    performUiAcceptanceAction({
      ...common,
      execute: async () => ({
        clientWidth: 112,
        lineCount: 2,
        scrollWidth: 112,
      }),
    }),
    /expected one rendered line/u,
  );

  await performUiAcceptanceAction({
    ...common,
    execute: async () => ({
      clientWidth: 168,
      lineCount: 1,
      scrollWidth: 168,
    }),
  });
});

test("runner limits Escape cancellation and its deterministic fallback to captured pointer reordering", async () => {
  const { performUiAcceptanceAction } = await import(
    "./UiAcceptanceRunner.mjs"
  );
  const requests = [];
  const executions = [];
  let activeStateChecks = 0;
  await performUiAcceptanceAction({
    action: {
      type: "drag",
      gesture: "pointer",
      selector: "#source",
      targetSelector: "#target",
      phase: "escape",
    },
    execute: async (script, args) => {
      executions.push({ args, script });
      if (script.includes("getBoundingClientRect")) {
        return {
          source: { x: 100, y: 100 },
          target: { x: 200, y: 100 },
          dropTarget: null,
        };
      }
      if (script.includes("document.querySelector")) {
        activeStateChecks += 1;
        return activeStateChecks === 1;
      }
      return true;
    },
    locateSelector: async (selector) => `element:${selector}`,
    locateText: async (text) => `text:${text}`,
    request: async (method, endpoint, body) => {
      requests.push({ body, endpoint, method });
      return null;
    },
    sessionId: "session-pointer-escape",
  });

  assert.deepEqual(requests.slice(-2), [
    {
      method: "POST",
      endpoint:
        "/session/session-pointer-escape/element/element%3Abody/value",
      body: { text: "\uE00C", value: ["\uE00C"] },
    },
    {
      method: "DELETE",
      endpoint: "/session/session-pointer-escape/actions",
      body: undefined,
    },
  ]);
  const activeStateExecutions = executions.filter(({ script }) =>
    script.includes("document.querySelector"),
  );
  for (const execution of activeStateExecutions) {
    assert.match(
      execution.args[0],
      /data-reorder-state.*preview.*invalid/su,
    );
  }
  const fallbackExecution = executions.find(({ script }) =>
    script.includes('KeyboardEvent("keydown"'),
  );
  assert.ok(fallbackExecution);
  assert.match(fallbackExecution.script, /KeyboardEvent\("keydown"/u);
  assert.match(fallbackExecution.script, /KeyboardEvent\("keyup"/u);
  assert.match(fallbackExecution.script, /key: "Escape"/u);
  assert.equal(activeStateChecks, 2);
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

function assertRunnerReleasesHeldActions(runner, checkoutLineEndings) {
  const normalizedRunner = runner.replaceAll("\r\n", "\n");
  const captureFunction = normalizedRunner.indexOf(
    "async function navigateAndCapture",
  );
  const screenshot = normalizedRunner.indexOf(
    "const screenshot = await captureUiAcceptanceScreenshot",
    captureFunction,
  );
  const release = normalizedRunner.indexOf(
    "await request(\"DELETE\", `/session/${sessionId}/actions`);",
    screenshot,
  );
  const nextFunction = normalizedRunner.indexOf("\n}\n", screenshot);

  assert.ok(
    release > screenshot,
    `${checkoutLineEndings}: held actions must be released after capture`,
  );
  assert.ok(
    release < nextFunction,
    `${checkoutLineEndings}: held actions must be released inside navigateAndCapture`,
  );
  assert.match(
    normalizedRunner.slice(captureFunction, nextFunction),
    /finally[\s\S]*DELETE/u,
    `${checkoutLineEndings}: capture cleanup must also run after an action or screenshot failure`,
  );
}

test("runner releases held pointer and modifier state for LF and CRLF checkouts", () => {
  const runner = readFileSync(
    path.join(workspace, "scripts", "Run-UiAcceptance.mjs"),
    "utf8",
  );
  const lfRunner = runner.replaceAll("\r\n", "\n");

  assertRunnerReleasesHeldActions(lfRunner, "LF checkout");
  assertRunnerReleasesHeldActions(
    lfRunner.replaceAll("\n", "\r\n"),
    "CRLF checkout",
  );
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

test("runner snapshots HEAD and dirty state before and after capture", () => {
  const runner = readFileSync(
    path.join(workspace, "scripts", "Run-UiAcceptance.mjs"),
    "utf8",
  );
  const initialSnapshot = runner.indexOf(
    "const initialSourceInputs = captureSourceInputs();",
  );
  const captureLoop = runner.indexOf("for (const scenario of scenariosToCapture)");
  const finalSnapshot = runner.indexOf(
    "const sourceInputsResult = finalizeUiAcceptanceSourceEvidence(",
  );
  const evidenceWrite = runner.indexOf(
    "writeFileSync(evidencePath",
  );

  assert.ok(initialSnapshot >= 0, "the initial source snapshot is missing");
  assert.ok(
    initialSnapshot < captureLoop,
    "the initial source snapshot must precede scenario capture",
  );
  assert.ok(
    finalSnapshot > captureLoop,
    "the final source snapshot must follow scenario capture",
  );
  assert.ok(
    finalSnapshot < evidenceWrite,
    "source integrity must be finalized before evidence is written",
  );
  assert.match(
    runner,
    /sourceInputsResult\.changedDuringCapture[\s\S]*process\.exitCode = 1/u,
    "source mutation must fail the capture gate",
  );
});

test("runner reloads every captured surface with a scenario-isolated URL", () => {
  const runner = readFileSync(
    path.join(workspace, "scripts", "Run-UiAcceptance.mjs"),
    "utf8",
  );

  assert.match(
    runner,
    /targetUrl\.searchParams\.set\("ui-acceptance-scenario", label\)/u,
    "a repeated implementationPath must still perform a document navigation",
  );
  assert.match(
    runner,
    /url: targetUrl\.href/u,
    "WebDriver must receive the isolated URL",
  );
  assert.match(runner, /MYALBUNS_UI_DEVICE_SCALE_FACTOR/u);
  assert.match(runner, /MYALBUNS_UI_SCENARIO_IDS/u);
  assert.match(runner, /for \(const scenario of scenariosToCapture\)/u);
  assert.match(
    runner,
    /`--force-device-scale-factor=\$\{deviceScaleFactor\}`/u,
    "the real renderer scale must be selectable without changing scenario geometry",
  );
  assert.match(
    runner,
    /const MAX_SCALED_VIEWPORT_ROUNDING = 4;[\s\S]*const viewportRoundingTolerance =\s*deviceScaleFactor === 1 \? 0 : MAX_SCALED_VIEWPORT_ROUNDING/u,
    "scaled viewport setup may admit only bounded browser-chrome rounding",
  );
});

test("runner scrolls an opposite drop surface into the pointer viewport", async () => {
  const { performUiAcceptanceAction } = await import(
    "./UiAcceptanceRunner.mjs"
  );
  const executions = [];

  await performUiAcceptanceAction({
    action: {
      type: "drag",
      gesture: "pointer",
      selector: "#source",
      targetSelector: "#target",
      dropTargetSelector: "#opposite-surface",
      phase: "drop",
    },
    execute: async (script, args) => {
      executions.push({ args, script });
      if (script.includes("getBoundingClientRect")) {
        return {
          source: { x: 100, y: 200 },
          target: { x: 300, y: 220 },
          dropTarget: { x: 1_200, y: 420 },
        };
      }
      return true;
    },
    locateSelector: async (selector) => `element:${selector}`,
    locateText: async (text) => `text:${text}`,
    request: async () => null,
    sessionId: "session-cross-surface",
  });

  assert.match(executions[1].script, /scrollIntoView/u);
  assert.deepEqual(executions[1].args, [
    {
      "element-6066-11e4-a52e-4f735466cecf":
        "element:#opposite-surface",
    },
  ]);
  assert.match(executions[2].script, /getBoundingClientRect/u);
});

test("the report labels captures as unreviewed and includes every scenario", () => {
  const evidence = {
    collectedAtUtc: "2026-08-24T12:00:00.000Z",
    gitCommit: "abc123",
    sourceInputsDirty: true,
    sourceInputs: {
      initial: { dirty: true, gitCommit: "abc123" },
      final: { dirty: true, gitCommit: "abc123" },
    },
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

test("a visual review is complete, explicit, and bound to the captured commit", () => {
  const evidence = {
    gitCommit: "abc123",
    sourceInputsDirty: false,
    sourceInputs: {
      initial: { dirty: false, gitCommit: "abc123" },
      final: { dirty: false, gitCommit: "abc123" },
    },
    scenarios: manifest.scenarios.slice(0, 2).map((scenario) => ({
      ...scenario,
      captureStatus: "captured-unreviewed",
    })),
  };
  const review = {
    schemaVersion: 1,
    gitCommit: "abc123",
    reviewedAtUtc: "2026-08-25T12:00:00.000Z",
    reviewer: "Codex visual review",
    scenarios: evidence.scenarios.map((scenario) => ({
      id: scenario.id,
      outcome: "accepted",
      notes: "Conferido contra a referência vigente e as decisões aceitas.",
    })),
  };

  assert.equal(validateUiAcceptanceReview(evidence, review), review);
  const reviewedReport = renderUiAcceptanceReport(
    {
      ...evidence,
      collectedAtUtc: "2026-08-25T11:00:00.000Z",
      captureStatus: "captured-unreviewed",
      sourceInputsDirty: false,
      scenarios: evidence.scenarios.map((scenario) => ({
        ...scenario,
        implementationScreenshot: `screenshots/${scenario.id}.png`,
        implementationUrl: `http://127.0.0.1:1437${scenario.implementationPath}`,
        ...(scenario.comparison.kind === "paired"
          ? {
              referenceScreenshot: `screenshots/${scenario.id}-reference.png`,
              referenceUrl: `http://127.0.0.1:1437${scenario.referencePath}`,
            }
          : {}),
      })),
    },
    review,
  );
  assert.match(reviewedReport, /Aceito · revisão registrada/u);
  assert.match(reviewedReport, /Codex visual review/u);
  assert.doesNotMatch(reviewedReport, /Nenhuma captura foi aprovada automaticamente/u);

  const stale = structuredClone(review);
  stale.gitCommit = "outro-commit";
  assert.throws(
    () => validateUiAcceptanceReview(evidence, stale),
    /must match the captured commit/u,
  );

  const incomplete = structuredClone(review);
  incomplete.scenarios.pop();
  assert.throws(
    () => validateUiAcceptanceReview(evidence, incomplete),
    /must cover every captured scenario/u,
  );

  const dirtyEvidence = { ...evidence, sourceInputsDirty: true };
  assert.throws(
    () => validateUiAcceptanceReview(dirtyEvidence, review),
    /cannot review evidence captured from a dirty worktree/u,
  );
});

test("source changes during capture invalidate every screenshot and block review", () => {
  const evidence = {
    gitCommit: "commit-before",
    sourceInputsDirty: false,
    sourceInputs: {
      initial: { dirty: false, gitCommit: "commit-before" },
      final: null,
    },
    captureStatus: "captured-unreviewed",
    scenarios: manifest.scenarios.slice(0, 2).map((scenario) => ({
      ...scenario,
      captureStatus: "captured-unreviewed",
      reviewStatus: "not-reviewed",
    })),
  };

  const result = finalizeUiAcceptanceSourceEvidence(evidence, {
    dirty: true,
    gitCommit: "commit-before",
  });

  assert.equal(result.changedDuringCapture, true);
  assert.equal(result.reviewable, false);
  assert.match(result.invalidationReason, /dirty state changed/u);
  assert.equal(evidence.captureStatus, "capture-invalidated");
  assert.deepEqual(
    evidence.scenarios.map((scenario) => scenario.captureStatus),
    ["capture-invalidated", "capture-invalidated"],
  );
  assert.deepEqual(
    evidence.scenarios.map((scenario) => scenario.reviewStatus),
    ["unvalidated", "unvalidated"],
  );
  const invalidatedReport = renderUiAcceptanceReport({
    ...evidence,
    collectedAtUtc: "2026-08-25T12:00:00.000Z",
    scenarios: evidence.scenarios.map((scenario) => ({
      ...scenario,
      implementationUrl: `http://127.0.0.1:1437${scenario.implementationPath}`,
      ...(scenario.comparison.kind === "paired"
        ? {
            referenceUrl: `http://127.0.0.1:1437${scenario.referencePath}`,
          }
        : {}),
    })),
  });
  assert.match(invalidatedReport, /2 foram invalidados/u);
  assert.match(invalidatedReport, /Captura invalidada/u);
  assert.match(invalidatedReport, /dirty state changed during UI acceptance capture/u);

  const review = {
    schemaVersion: 1,
    gitCommit: "commit-before",
    reviewedAtUtc: "2026-08-25T12:00:00.000Z",
    reviewer: "Codex visual review",
    scenarios: evidence.scenarios.map((scenario) => ({
      id: scenario.id,
      outcome: "unvalidated",
      notes: "As fontes mudaram durante a captura.",
    })),
  };
  assert.throws(
    () => validateUiAcceptanceReview(evidence, review),
    /source inputs changed during capture/u,
  );

  const headChanged = {
    ...evidence,
    captureStatus: "captured-unreviewed",
    scenarios: evidence.scenarios.map((scenario) => ({
      ...scenario,
      captureStatus: "captured-unreviewed",
      reviewStatus: "not-reviewed",
    })),
    sourceInputs: {
      initial: { dirty: false, gitCommit: "commit-before" },
      final: null,
    },
  };
  const headResult = finalizeUiAcceptanceSourceEvidence(headChanged, {
    dirty: false,
    gitCommit: "commit-after",
  });
  assert.equal(headResult.changedDuringCapture, true);
  assert.match(headResult.invalidationReason, /HEAD changed/u);
  assert.equal(headChanged.captureStatus, "capture-invalidated");
});

test("stable source snapshots preserve captures but only clean inputs are reviewable", () => {
  const cleanEvidence = {
    captureStatus: "captured-unreviewed",
    scenarios: [{ captureStatus: "captured-unreviewed" }],
    sourceInputs: {
      initial: { dirty: false, gitCommit: "stable-commit" },
      final: null,
    },
  };
  const cleanResult = finalizeUiAcceptanceSourceEvidence(cleanEvidence, {
    dirty: false,
    gitCommit: "stable-commit",
  });
  assert.equal(cleanResult.changedDuringCapture, false);
  assert.equal(cleanResult.reviewable, true);
  assert.equal(cleanEvidence.captureStatus, "captured-unreviewed");

  const dirtyEvidence = {
    captureStatus: "captured-unreviewed",
    scenarios: [{ captureStatus: "captured-unreviewed" }],
    sourceInputs: {
      initial: { dirty: true, gitCommit: "stable-commit" },
      final: null,
    },
  };
  const dirtyResult = finalizeUiAcceptanceSourceEvidence(dirtyEvidence, {
    dirty: true,
    gitCommit: "stable-commit",
  });
  assert.equal(dirtyResult.changedDuringCapture, false);
  assert.equal(dirtyResult.reviewable, false);
  assert.equal(dirtyEvidence.captureStatus, "captured-unreviewed");
});
