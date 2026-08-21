import assert from "node:assert/strict";
import test from "node:test";

import {
  assertEmptyCacheExport,
  assertCausalProjectHandoff,
  assertCorrelatedJourneyTerminals,
  assertDistinguishableSheetExport,
} from "./ProductiveJourneyObservations.mjs";

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
