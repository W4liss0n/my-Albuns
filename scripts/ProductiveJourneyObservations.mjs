const EVENT_PATTERNS = (event) => [
  `"event":"${event}"`,
  `event="${event}"`,
];

export function eventCount(output, event) {
  return EVENT_PATTERNS(event).reduce(
    (total, pattern) => total + output.split(pattern).length - 1,
    0,
  );
}

function firstEventOffset(output, event) {
  const offsets = EVENT_PATTERNS(event)
    .map((pattern) => output.indexOf(pattern))
    .filter((offset) => offset >= 0);
  return offsets.length === 0 ? -1 : Math.min(...offsets);
}

export function assertCausalProjectHandoff(output) {
  const hostReady = firstEventOffset(output, "host_ready");
  const uiReady = firstEventOffset(output, "project_ui_ready");
  const globalExit = firstEventOffset(
    output,
    "global_exited_after_project_handoff",
  );
  for (const [event, offset] of [
    ["host_ready", hostReady],
    ["project_ui_ready", uiReady],
    ["global_exited_after_project_handoff", globalExit],
  ]) {
    if (offset < 0) {
      throw new Error(`The productive journey did not observe ${event}`);
    }
  }
  if (!(hostReady < uiReady && uiReady < globalExit)) {
    throw new Error(
      "The productive journey did not observe the causal order host_ready -> project_ui_ready -> global_exited_after_project_handoff",
    );
  }
  return true;
}

function correlatedCount(records, event, processId, extra = () => true) {
  return records.filter(
    (record) =>
      record.event === event &&
      Number(record.process_id) === processId &&
      extra(record),
  ).length;
}

function requireCorrelatedCount(records, event, processId, extra) {
  const count = correlatedCount(records, event, processId, extra);
  if (count !== 1) {
    throw new Error(
      `The productive journey expected ${event} for process ${processId} exactly once and observed ${count}`,
    );
  }
}

export function assertCorrelatedJourneyTerminals(
  records,
  { bootstraps, imagingAttempts },
) {
  for (const { globalProcessId, hostProcessId } of bootstraps) {
    requireCorrelatedCount(records, "host_ready", hostProcessId);
    requireCorrelatedCount(records, "project_ui_ready", hostProcessId);
    requireCorrelatedCount(
      records,
      "global_exited_after_project_handoff",
      globalProcessId,
    );
  }
  for (const { hostProcessId, imagingProcessId } of imagingAttempts) {
    requireCorrelatedCount(
      records,
      "imaging_process_spawned",
      hostProcessId,
      (record) => Number(record.imaging_process_id) === imagingProcessId,
    );
    requireCorrelatedCount(
      records,
      "imaging_process_stopped",
      imagingProcessId,
    );
  }
  return {
    bootstraps: bootstraps.length,
    imagingAttempts: imagingAttempts.length,
  };
}

const MICROMETERS_PER_INCH = 25_400;

function rasterDimensions(document, sheet, dpi) {
  const widthUm =
    sheet.activeSides === "both"
      ? document.sheetWidthUm
      : Math.floor(document.sheetWidthUm / 2);
  return {
    width: Math.round((widthUm * dpi) / MICROMETERS_PER_INCH),
    height: Math.round(
      (document.sheetHeightUm * dpi) / MICROMETERS_PER_INCH,
    ),
  };
}

export function assertDistinguishableSheetExport({
  document,
  sheets,
  visualDefaults,
  expectedBackgroundRgb,
  selectedSheetNumber,
  exportedDpi,
  jpegDimensions,
}) {
  if (
    !Number.isInteger(selectedSheetNumber) ||
    selectedSheetNumber <= 1 ||
    selectedSheetNumber > sheets.length
  ) {
    throw new Error("The productive journey did not select a non-initial sheet");
  }
  const firstSheet = sheets[0];
  const selectedSheet = sheets[selectedSheetNumber - 1];
  const firstSheetDimensions = rasterDimensions(
    document,
    firstSheet,
    exportedDpi,
  );
  const selectedSheetDimensions = rasterDimensions(
    document,
    selectedSheet,
    exportedDpi,
  );
  if (
    firstSheetDimensions.width === selectedSheetDimensions.width &&
    firstSheetDimensions.height === selectedSheetDimensions.height
  ) {
    throw new Error(
      "The productive journey sheets are not distinguishable in the JPEG output",
    );
  }
  if (
    jpegDimensions.width !== selectedSheetDimensions.width ||
    jpegDimensions.height !== selectedSheetDimensions.height ||
    (jpegDimensions.width === firstSheetDimensions.width &&
      jpegDimensions.height === firstSheetDimensions.height)
  ) {
    throw new Error(
      "The JPEG does not prove the selected non-initial sheet was exported",
    );
  }
  const background = visualDefaults?.background;
  if (
    background?.scope !== "bothSides" ||
    background.both?.kind !== "color" ||
    background.both.rgb !== expectedBackgroundRgb
  ) {
    throw new Error(
      "The saved Project does not contain the expected personalization",
    );
  }
  return {
    exportedSheetNumber: selectedSheetNumber,
    selectedSheetId: selectedSheet.id,
    selectedSheetActiveSides: selectedSheet.activeSides,
    selectedSheetDimensions,
    firstSheetDimensions,
    expectedBackgroundRgb,
  };
}
