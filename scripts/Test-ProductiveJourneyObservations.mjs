import assert from "node:assert/strict";
import {
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { createOwnedCacheGuard } from "./ProductiveJourneyCacheSafety.mjs";
import {
  assertEmptyCacheExport,
  assertCausalProjectHandoff,
  assertCorrelatedJourneyTerminals,
  assertDistinguishableSheetExport,
  assertPhysicalAlbumProjectCoreEvents,
  assertReopenedHostExport,
} from "./ProductiveJourneyObservations.mjs";

test("productive journey uses the neutral captured-pointer gesture policy", () => {
  const runner = readFileSync(
    path.join(
      path.dirname(fileURLToPath(import.meta.url)),
      "Run-ProductiveJourneyGate.mjs",
    ),
    "utf8",
  );

  assert.match(runner, /from "\.\/WebDriverPointerGestures\.mjs"/u);
  assert.doesNotMatch(runner, /const visibleCenter =/u);
});

test("locates the New Project flow through stable accessible names", () => {
  const scripts = path.dirname(fileURLToPath(import.meta.url));
  const runner = readFileSync(
    path.join(scripts, "Run-ProductiveJourneyGate.mjs"),
    "utf8",
  );
  const wrapper = readFileSync(
    path.join(scripts, "Test-ProductiveJourney.ps1"),
    "utf8",
  );

  assert.match(
    runner,
    /"css selector",\s*"button\[aria-label='Novo Projeto'\]"/,
  );
  assert.match(runner, /\["Largura da Lâmina fechada", "50\.8"\]/);
  assert.match(runner, /\["Altura da Lâmina fechada", "25\.4"\]/);
  assert.match(
    runner,
    /"css selector",\s*"button\[aria-label='Continuar'\]"/,
  );
  assert.match(
    runner,
    /"css selector",\s*"button\[aria-label='Criar Projeto'\]"/,
  );
  assert.match(runner, /"input\[aria-label='DPI'\]"/);
  assert.match(runner, /"button\[form='album-information-settings'\]"/);
  assert.match(runner, /MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DEBUG_PORT/);
  assert.match(runner, /MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DATA_DIRECTORY/);
  assert.match(runner, /projectDialogDebugPort/);
  assert.match(
    runner,
    /productive Grade click did not activate Sheet 2/,
  );
  assert.match(runner, /button\.active \.sheet-tile__number/);
  assert.match(
    runner,
    /startAttachedWebDriver\(\s*driver\.projectDialogDebugPort/,
  );
  assert.match(runner, /Aplicar alterações no Álbum\?/);
  assert.match(runner, /selectApplicationMenuCommand/);
  assert.match(runner, /clickElementWhenInteractable/);
  assert.match(runner, /withProjectDialog/);
  assert.match(runner, /function accessibleProjectDialogXpath/);
  assert.match(
    runner,
    /@aria-labelledby = \/\/\*\[normalize-space\(\)=\$\{title\}\]\/@id/,
  );
  assert.doesNotMatch(
    runner,
    /@role='dialog' and @aria-label=/,
  );
  assert.match(runner, /openPhotoImportDialog/);
  assert.match(runner, /globalInspectorPreferencePreserved/);
  assert.match(runner, /projectLocalSelectionReset/);
  assert.doesNotMatch(runner, /recovery Project Host after resolution/);
  assert.match(runner, /waitForHttpUnavailable/);
  assert.match(runner, /waitForWebViewDataDirectoryRelease/);
  assert.match(runner, /waitForHostUiReady/);
  assert.match(runner, /webDriverSessionTimeoutMilliseconds/);
  assert.match(runner, /missing-Original Processador terminal/);
  assert.match(runner, /Number\(missingOriginalAttempt\.imaging_process_id\)/);
  assert.match(runner, /MYALBUNS_DEV_ALTERNATE_HOST_WEBVIEW_DEBUG_PORT/);
  assert.match(runner, /reopenedHostDebugPort/);
  assert.match(runner, /The productive Host WebView2 process was not observable/);
  assert.match(runner, /const observationDeadline = Math\.min\(deadline, Date\.now\(\) \+ 500\)/);
  assert.match(runner, /\$\{label\} confirmation dialog/);
  assert.doesNotMatch(
    runner,
    /\/\/button\[normalize-space\(\)='(?:Novo Projeto|Próximo|Criar)'\]/,
  );
  assert.doesNotMatch(runner, /document-dpi-control|Aplicar DPI/);
  assert.doesNotMatch(
    runner,
    /button\[aria-label='(?:Desfazer|Refazer|Salvar)'\]/,
  );
  assert.doesNotMatch(
    runner,
    /\/\/button\[normalize-space\(\)='Exportar Lâmina'\]/,
  );
  assert.doesNotMatch(runner, /Importar JPEG…/);
  assert.doesNotMatch(runner, /\.media-card\[data-media-id\]/);
  assert.match(runner, /switchToWebDriverWindow\(/);
  assert.match(
    runner,
    /const ownerTargets = await devToolsTargets\(\s*recoveryGlobalDebugPort/,
  );
  assert.match(
    runner,
    /startAttachedWebDriver\(\s*recoveryGlobalDebugPort/,
  );
  assert.match(
    runner,
    /parsed\.pathname\.endsWith\("\/dialog\.html"\)[\s\S]*parsed\.searchParams\.get\("kind"\) === "project-recovery"/u,
  );
  assert.match(runner, /const actionGeometry =/u);
  assert.match(runner, /action\.lineCount === 1/u);
  assert.match(runner, /recoveryPresentation\.viewportWidth !== 492/u);
  assert.match(
    runner,
    /parsed\.searchParams\.get\("kind"\) === "external-copy"/u,
  );
  assert.match(runner, /externalCopyPresentation\.viewportWidth !== 440/u);
  assert.match(runner, /nativeOwnedWindowState\(externalCopyGlobal\)/u);
  assert.match(runner, /cancelRestoredGlobalAndCleanedHost/u);
  assert.match(runner, /\[externalCopyPath\]/u);
  assert.match(runner, /realPathActivationsCompletedSerially/u);
  assert.match(runner, /pickerCancellationPreservedAttempt/u);
  assert.match(runner, /emptyActivationDidNotResurrectGlobal/u);
  assert.match(runner, /samePendingHostCompletedHandoff/u);
  assert.match(runner, /selectedExternalCopy\.exactProcess === true/u);
  assert.match(runner, /new Event\('webglcontextlost'/u);
  assert.match(runner, /canvas_context_restore_failed/u);
  assert.match(runner, /graphicsDialogOwnedAndProjectBlocked/u);
  assert.match(runner, /cancelledCloseRearmedSingleGraphicsDialog/u);
  assert.match(runner, /workspaceInertBeforeDialogTerminal/u);
  assert.match(runner, /exportDisabledBeforeDialogTerminal/u);
  assert.match(wrapper, /externalCopyOpening\.pickerCancellationPreservedAttempt/u);
  assert.match(wrapper, /graphicsFailure\.dialogOwnedByProject/u);
  assert.doesNotMatch(runner, /DEBUG-project-dialog-targets/);
  assert.doesNotMatch(runner, /localStateStartedEmpty/);
});

test("keeps the physical Album structure proof in the productive WebView2 contract", () => {
  const scripts = path.dirname(fileURLToPath(import.meta.url));
  const runner = readFileSync(
    path.join(scripts, "Run-ProductiveJourneyGate.mjs"),
    "utf8",
  );
  const wrapper = readFileSync(
    path.join(scripts, "Test-ProductiveJourney.ps1"),
    "utf8",
  );

  assert.match(wrapper, /presentation\.viewportWidth -ne 492/u);
  assert.match(wrapper, /invalidRecoveryActionGeometry\.Count -ne 0/u);

  assert.match(runner, /physicalAlbumStructure:\s*\{/);
  assert.match(runner, /projectCoreEvents/);
  assert.match(runner, /"Lâmina",\s*"Adicionar depois"/);
  assert.match(runner, /"Lâmina",\s*"Excluir"/);
  assert.match(runner, /\.sheet-grid-slot\[data-sheet-id=/);
  assert.match(
    runner,
    /"add_sheet",\s*"reorder_sheet",\s*"delete_sheet"/,
  );
  assert.match(wrapper, /\$physicalAlbumStructure\.afterAdd\.count -ne 4/);
  assert.match(wrapper, /\$physicalAlbumStructure\.afterReorder\.count -ne 4/);
  assert.match(wrapper, /\$physicalAlbumStructure\.afterDelete\.count -ne 3/);
  assert.match(wrapper, /physical-album-structure-ui-project-core/);
  assert.match(wrapper, /physicalAlbumStructure = \$gate\.physicalAlbumStructure/);
});

test("correlates the physical Album ProjectCore events to one Host in causal order", () => {
  const input = [
    {
      event: "project_intent_applied",
      intent: "add_sheet",
      process_id: 203,
      revision: 9,
    },
    {
      event: "project_intent_applied",
      intent: "reorder_sheet",
      process_id: 203,
      revision: 10,
    },
    {
      event: "project_intent_applied",
      intent: "delete_sheet",
      process_id: 203,
      revision: 11,
    },
  ];

  assert.deepEqual(
    assertPhysicalAlbumProjectCoreEvents(input, {
      hostProcessId: 203,
      intents: ["add_sheet", "reorder_sheet", "delete_sheet"],
    }),
    [
      {
        event: "project_intent_applied",
        intent: "add_sheet",
        processId: 203,
        revision: 9,
      },
      {
        event: "project_intent_applied",
        intent: "reorder_sheet",
        processId: 203,
        revision: 10,
      },
      {
        event: "project_intent_applied",
        intent: "delete_sheet",
        processId: 203,
        revision: 11,
      },
    ],
  );

  assert.throws(
    () =>
      assertPhysicalAlbumProjectCoreEvents(
        input.map(({ process_id: _processId, ...record }) => record),
        {
          hostProcessId: 203,
          intents: ["add_sheet", "reorder_sheet", "delete_sheet"],
        },
      ),
    /Host 203/,
  );
  assert.throws(
    () =>
      assertPhysicalAlbumProjectCoreEvents(
        [input[0], { ...input[1], revision: 12 }, input[2]],
        {
          hostProcessId: 203,
          intents: ["add_sheet", "reorder_sheet", "delete_sheet"],
        },
      ),
    /causal and consecutive/,
  );
});

test("emits Project intent outcomes with the Desktop Host process id", () => {
  const commands = readFileSync(
    path.join(
      path.dirname(fileURLToPath(import.meta.url)),
      "..",
      "src-tauri",
      "src",
      "project_commands.rs",
    ),
    "utf8",
  );

  assert.match(
    commands,
    /tracing::warn!\([\s\S]{0,400}?process_id\s*=\s*process_id[\s\S]{0,400}?event\s*=\s*"project_intent_rejected"/,
  );
  assert.match(
    commands,
    /tracing::info!\([\s\S]{0,400}?process_id\s*=\s*process_id[\s\S]{0,400}?event\s*=\s*"project_intent_applied"/,
  );
});

function withJunctionFixture(configure, assertion) {
  const root = mkdtempSync(path.join(os.tmpdir(), "myalbuns-cache-guard-"));
  const scratch = path.join(root, "scratch");
  const processDataRoot = path.join(scratch, "process-data");
  const cacheRoot = path.join(processDataRoot, "Local", "MyAlbuns2", "Cache");
  const external = path.join(root, "external");
  const sentinel = path.join(external, "sentinel.txt");
  mkdirSync(external, { recursive: true });
  writeFileSync(sentinel, "must-survive");
  let junction;
  try {
    junction = configure({ cacheRoot, external, processDataRoot });
    assertion({
      cacheRoot,
      guard: createOwnedCacheGuard({ scratch, processDataRoot }),
      sentinel,
    });
    assert.equal(readFileSync(sentinel, "utf8"), "must-survive");
  } finally {
    if (junction && lstatSync(junction, { throwIfNoEntry: false })?.isSymbolicLink()) {
      unlinkSync(junction);
    }
    rmSync(root, { recursive: true, force: true });
  }
}

test(
  "refuses a junction at the owned Cache root without touching its external target",
  { skip: process.platform !== "win32" },
  () => {
    withJunctionFixture(
      ({ cacheRoot, external }) => {
        mkdirSync(path.dirname(cacheRoot), { recursive: true });
        symlinkSync(external, cacheRoot, "junction");
        return cacheRoot;
      },
      ({ cacheRoot, guard }) => {
        assert.throws(
          () => guard.purgeOwnedCache(cacheRoot),
          /redirected|reparse/i,
        );
      },
    );
  },
);

test(
  "refuses a junction in a Cache ancestor without touching its external target",
  { skip: process.platform !== "win32" },
  () => {
    withJunctionFixture(
      ({ cacheRoot, external, processDataRoot }) => {
        mkdirSync(path.join(external, "Cache"), { recursive: true });
        const ancestor = path.join(processDataRoot, "Local", "MyAlbuns2");
        mkdirSync(path.dirname(ancestor), { recursive: true });
        symlinkSync(external, ancestor, "junction");
        return ancestor;
      },
      ({ cacheRoot, guard }) => {
        assert.throws(
          () => guard.purgeOwnedCache(cacheRoot),
          /redirected|reparse/i,
        );
      },
    );
  },
);

test("purges a regular Cache tree without recursive removal through redirects", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "myalbuns-cache-purge-"));
  const scratch = path.join(root, "scratch");
  const processDataRoot = path.join(scratch, "process-data");
  const cacheRoot = path.join(processDataRoot, "Local", "MyAlbuns2", "Cache");
  const media = path.join(cacheRoot, "project-proof", "Media");
  const outsideSentinel = path.join(root, "outside.txt");
  try {
    mkdirSync(media, { recursive: true });
    writeFileSync(path.join(media, "preview.jpg"), "derived");
    writeFileSync(outsideSentinel, "must-survive");
    const guard = createOwnedCacheGuard({ scratch, processDataRoot });

    assert.deepEqual(guard.purgeOwnedCache(cacheRoot), {
      entryCount: 0,
      byteCount: 0,
      jpegCount: 0,
    });
    assert.equal(readFileSync(outsideSentinel, "utf8"), "must-survive");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("accepts an export only when the real Cache namespace is empty before and after it", () => {
  assert.deepEqual(
    assertEmptyCacheExport({
      previewArtifactCountBeforePurge: 2,
      cacheEntryCountBeforeExport: 0,
      cacheByteCountBeforeExport: 0,
      cacheEntryCountAfterExport: 0,
      cacheByteCountAfterExport: 0,
    }),
    {
      previewArtifactCountBeforePurge: 2,
      cacheEntryCountBeforeExport: 0,
      cacheByteCountBeforeExport: 0,
      cacheEntryCountAfterExport: 0,
      cacheByteCountAfterExport: 0,
    },
  );
  assert.throws(
    () =>
      assertEmptyCacheExport({
        previewArtifactCountBeforePurge: 1,
        cacheEntryCountBeforeExport: 1,
        cacheByteCountBeforeExport: 42,
        cacheEntryCountAfterExport: 1,
        cacheByteCountAfterExport: 42,
      }),
    /Cache namespace was not empty before Export/,
  );
  assert.throws(
    () =>
      assertEmptyCacheExport({
        previewArtifactCountBeforePurge: 0,
        cacheEntryCountBeforeExport: 0,
        cacheByteCountBeforeExport: 0,
        cacheEntryCountAfterExport: 0,
        cacheByteCountAfterExport: 0,
      }),
    /no real preview artifact/,
  );
});

const completeJourney = [
  '{"event":"host_ready","process_id":101}',
  '{"event":"project_ui_ready","process_id":101}',
  '{"event":"global_exited_after_project_handoff","process_id":100}',
  '{"event":"imaging_process_stopped","process_id":102}',
  '{"event":"project_window_destroyed","process_id":101}',
].join("\n");

test("accepts only the observed causal handoff order", () => {
  assert.doesNotThrow(() => assertCausalProjectHandoff(completeJourney));

  assert.throws(
    () =>
      assertCausalProjectHandoff(
        [
          '{"event":"host_ready"}',
          '{"event":"global_exited_after_project_handoff"}',
        ].join("\n"),
      ),
    /project_ui_ready/,
  );
  assert.throws(
    () =>
      assertCausalProjectHandoff(
        [
          '{"event":"global_exited_after_project_handoff"}',
          '{"event":"host_ready"}',
          '{"event":"project_ui_ready"}',
        ].join("\n"),
      ),
    /causal order/,
  );
});

test("correlates one terminal to each bootstrap and imaging attempt", () => {
  const records = [
    { event: "host_ready", process_id: 201 },
    { event: "project_ui_ready", process_id: 201 },
    { event: "global_exited_after_project_handoff", process_id: 200 },
    { event: "host_ready", process_id: 203 },
    { event: "project_ui_ready", process_id: 203 },
    { event: "global_exited_after_project_handoff", process_id: 202 },
    {
      event: "imaging_process_spawned",
      process_id: 201,
      imaging_process_id: 204,
      operation: "export",
    },
    { event: "imaging_process_stopped", process_id: 204 },
  ];

  assert.deepEqual(
    assertCorrelatedJourneyTerminals(records, {
      bootstraps: [
        { globalProcessId: 200, hostProcessId: 201 },
        { globalProcessId: 202, hostProcessId: 203 },
      ],
      imagingAttempts: [{ hostProcessId: 201, imagingProcessId: 204 }],
    }),
    { bootstraps: 2, imagingAttempts: 1 },
  );

  assert.throws(
    () =>
      assertCorrelatedJourneyTerminals(
        [...records, { event: "project_ui_ready", process_id: 203 }],
        {
          bootstraps: [
            { globalProcessId: 200, hostProcessId: 201 },
            { globalProcessId: 202, hostProcessId: 203 },
          ],
          imagingAttempts: [
            { hostProcessId: 201, imagingProcessId: 204 },
          ],
        },
      ),
    /project_ui_ready.*203.*2/,
  );

  assert.throws(
    () =>
      assertCorrelatedJourneyTerminals(
        [
          ...records,
          {
            event: "imaging_process_spawned",
            process_id: 203,
            imaging_process_id: 205,
            operation: "export",
          },
          { event: "imaging_process_stopped", process_id: 205 },
        ],
        {
          bootstraps: [
            { globalProcessId: 200, hostProcessId: 201 },
            { globalProcessId: 202, hostProcessId: 203 },
          ],
          imagingAttempts: [
            { hostProcessId: 201, imagingProcessId: 204 },
          ],
        },
      ),
    /exact.*Processador.*unexpected/i,
  );
});

test("accepts Export only when its Processador belongs to the reopened Host", () => {
  assert.equal(
    assertReopenedHostExport({
      savedHostProcessId: 201,
      reopenedHostProcessId: 203,
      exportHostProcessId: 203,
    }),
    true,
  );
  assert.throws(
    () =>
      assertReopenedHostExport({
        savedHostProcessId: 201,
        reopenedHostProcessId: 203,
        exportHostProcessId: 201,
      }),
    /reopened Host/,
  );
});

test("proves the selected non-initial sheet from distinguishable JPEG dimensions", () => {
  const input = {
    document: {
      sheetWidthUm: 50_800,
      sheetHeightUm: 25_400,
    },
    sheets: [
      { id: "sheet-001", activeSides: "right" },
      { id: "sheet-002", activeSides: "both" },
      { id: "sheet-003", activeSides: "left" },
    ],
    visualDefaults: {
      background: {
        scope: "bothSides",
        both: { kind: "color", rgb: "#204060" },
      },
    },
    expectedBackgroundRgb: "#204060",
    selectedSheetNumber: 2,
    exportedDpi: 360,
  };

  assert.deepEqual(
    assertDistinguishableSheetExport({
      ...input,
      jpegDimensions: { width: 720, height: 360 },
    }),
    {
      exportedSheetNumber: 2,
      selectedSheetId: "sheet-002",
      selectedSheetActiveSides: "both",
      selectedSheetDimensions: { width: 720, height: 360 },
      firstSheetDimensions: { width: 360, height: 360 },
      expectedBackgroundRgb: "#204060",
    },
  );

  assert.throws(
    () =>
      assertDistinguishableSheetExport({
        ...input,
        jpegDimensions: { width: 360, height: 360 },
      }),
    /selected non-initial sheet/,
  );
  assert.throws(
    () =>
      assertDistinguishableSheetExport({
        ...input,
        sheets: input.sheets.map((sheet) => ({
          ...sheet,
          activeSides: "both",
        })),
        jpegDimensions: { width: 720, height: 360 },
      }),
    /distinguishable/,
  );
  assert.throws(
    () =>
      assertDistinguishableSheetExport({
        ...input,
        visualDefaults: {
          background: {
            scope: "bothSides",
            both: { kind: "color", rgb: "#FFFFFF" },
          },
        },
        jpegDimensions: { width: 720, height: 360 },
      }),
    /personalization/,
  );
});
