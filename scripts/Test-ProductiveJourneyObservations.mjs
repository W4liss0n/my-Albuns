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

import { createOwnedCacheGuard } from "./ProductiveJourneyCacheSafety.mjs";
import {
  assertEmptyCacheExport,
  assertCausalProjectHandoff,
  assertCorrelatedJourneyTerminals,
  assertDistinguishableSheetExport,
  assertReopenedHostExport,
} from "./ProductiveJourneyObservations.mjs";

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
