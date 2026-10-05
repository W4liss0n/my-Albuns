import { expect, test } from "vitest";

import type { NormalExportOptions } from "../application/normalExport";
import type { ProjectDialogState } from "../application/projectDialogPort";
import {
  parseInitialProjectDialogPresentation,
  parseInitialProjectDialogPreviewState,
  parseProjectDialogAction,
  parseProjectDialogActionEvent,
  parseProjectDialogPresentation,
  parseProjectDialogState,
  toIpcProjectDialogState,
} from "./projectDialogContract";

const exportOptions: NormalExportOptions = {
  scope: "range", sheetIds: ["sheet-001", "sheet-002"], mode: "page",
  format: { kind: "jpeg", quality: 90 }, destination: "C:/Exportados", conflictPolicy: "ask",
};

const states: readonly ProjectDialogState[] = [
  { kind: "exportMediaProblems", projectName: "Álbum", problems: [{ mediaId: "photo-1", fileName: "Foto.jpg", state: "absent" }], busy: true, message: "Procurando arquivos…" },
  { kind: "layoutDeletionConfirmation", busy: false },
  { kind: "edgeConversionConfirmation", message: "A sobreposição personalizada será removida." },
  { kind: "formatConversionSaveConfirmation" },
  { kind: "mediaRemovalConfirmation", mediaKind: "photo", count: 3, usedCount: 2, usageCount: 4, busy: false },
  { kind: "exportProblems", projectName: "Álbum", problems: [{ sheetId: "sheet-001", sheetNumber: 1, frameId: "frame-002", frameNumber: 2 }] },
  { kind: "imageProcessingProgress", progress: { kind: "determinate", completed: 5, total: 12, status: "" } },
  { kind: "imageProcessingProblems", importedCount: 2, problems: [{ fileName: "ruim.jpg", reason: "JPEG corrompido" }] },
  { kind: "imageProcessingProblems", importedCount: 2, problems: [], operationProblem: "Memória indisponível" },
  {
    busy: false,
    consequences: ["O fundo da lâmina 1 será removido."],
    kind: "albumInformationConfirmation",
  },
  { busy: false, kind: "projectCloseConfirmation" },
  { busy: true, formatConversion: true, kind: "projectCloseConfirmation" },
  { kind: "projectCloseFailure", message: "Falha ao fechar" },
  { kind: "projectOperationFailure", message: "Falha ao salvar" },
  { kind: "graphicsFailure", reason: "O contexto WebGL2 foi perdido." },
  {
    cancelRequested: false,
    cancellable: true,
    kind: "exportProgress",
    progress: {
      completed: 2,
      kind: "determinate",
      status: "Exportando",
      total: 5,
    },
  },
  {
    cancelled: false,
    kind: "exportFailure",
    message: "Falha ao exportar",
    retryDisabled: false,
  },
  { kind: "exportSuccess", message: "Exportação concluída" },
  { kind: "storageFull", message: "O disco está cheio.", canClearCache: true, busy: false },
  { kind: "storageFull", message: "O disco está cheio.", canClearCache: false, busy: true },
  {
    kind: "exportConfiguration",
    sheets: [{ sheetId: "sheet-001", number: 1, pageCount: 2 }, { sheetId: "sheet-002", number: 2, pageCount: 1 }],
    options: exportOptions,
    busy: true,
    message: "Preparando a exportação…",
  },
  { kind: "exportConflicts", files: ["C:/Exportados/Lâmina 01.jpg", "C:/Exportados/Lâmina 02.jpg"] },
];

test.each(states)("round-trips the $kind state through the native contract", (state) => {
  expect(parseProjectDialogState(toIpcProjectDialogState(state))).toEqual(
    state,
  );
});

test("rejects malformed states and actions at the native seam", () => {
  expect(parseProjectDialogState({ kind: "imageProcessingProblems", importedCount: 2, problems: [], operationProblem: 123 })).toBeNull();
  expect(
    parseProjectDialogState({
      busy: false,
      consequences: [{ label: "DPI", value: "300 → 240" }],
      kind: "albumInformationConfirmation",
    }),
  ).toBeNull();
  expect(
    parseProjectDialogState({
      cancelRequested: false,
      cancellable: true,
      kind: "exportProgress",
      progress: {
        completed: Number.MAX_SAFE_INTEGER + 1,
        kind: "determinate",
        status: "Exportando",
        total: 5,
      },
    }),
  ).toBeNull();
  expect(parseProjectDialogAction("unknownAction")).toBeNull();
});

test("decodes the initial URL only through the validated state contract", () => {
  const state = states[1];
  expect(
    parseInitialProjectDialogPreviewState(
      `?state=${encodeURIComponent(JSON.stringify(state))}`,
    ),
  ).toEqual(state);
  expect(
    parseInitialProjectDialogPreviewState("?state=%7Bnot-json%7D"),
  ).toBeNull();
});

test("keeps the dialog action and initial window bound to their session", () => {
  expect(
    parseProjectDialogActionEvent({
      action: "confirmAlbumInformation",
      sessionId: "album-information-7",
    }),
  ).toEqual({
    action: "confirmAlbumInformation",
    sessionId: "album-information-7",
  });
  expect(
    parseProjectDialogActionEvent({
      action: "confirmAlbumInformation",
      sessionId: "",
    }),
  ).toBeNull();
  const presentation = {
    sessionId: "album-information-7",
    windowWidth: 520,
    state: states[0],
  };
  expect(parseProjectDialogPresentation(presentation)).toEqual(presentation);
  for (const windowWidth of [undefined, 0, -1, 1.5, NaN, Infinity, 65536]) {
    expect(parseProjectDialogPresentation({ ...presentation, windowWidth })).toBeNull();
  }
  expect(
    parseInitialProjectDialogPresentation(
      `?presentation=${encodeURIComponent(JSON.stringify(presentation))}`,
    ),
  ).toEqual(presentation);
  expect(
    parseProjectDialogPresentation({ sessionId: "", state: states[0] }),
  ).toBeNull();
  expect(
    parseInitialProjectDialogPresentation("?presentation=%7Bnot-json%7D"),
  ).toBeNull();
});

function stateOfKind<Kind extends ProjectDialogState["kind"]>(kind: Kind) {
  const state = states.find((candidate) => candidate.kind === kind);
  if (!state) throw new Error(`Missing ${kind} fixture`);
  return toIpcProjectDialogState(state);
}

// Each case breaks one field of a state the contract accepts, so the
// rejection can only come from that field's validation.
test.each<[string, ProjectDialogState["kind"], Record<string, unknown>]>([
  ["Export options that fail their own contract", "exportConfiguration", { options: { ...exportOptions, sheetIds: [] } }],
  ["an Export configuration without Sheets", "exportConfiguration", { sheets: [] }],
  ["a Sheet with three Pages", "exportConfiguration", { sheets: [{ sheetId: "sheet-001", number: 1, pageCount: 3 }] }],
  ["a Sheet numbered zero", "exportConfiguration", { sheets: [{ sheetId: "sheet-001", number: 0, pageCount: 2 }] }],
  ["a Sheet without an identifier", "exportConfiguration", { sheets: [{ number: 1, pageCount: 2 }] }],
  ["a non-boolean Export configuration busy flag", "exportConfiguration", { busy: "false" }],
  ["a non-text Export configuration message", "exportConfiguration", { message: undefined }],
  ["an Export conflict that is not a path", "exportConflicts", { files: ["C:/Exportados/Lâmina 01.jpg", 2] }],
  ["Export conflicts that are not a list", "exportConflicts", { files: "C:/Exportados/Lâmina 01.jpg" }],
  ["an unknown media kind", "mediaRemovalConfirmation", { mediaKind: "video" }],
  ["a negative used-media count", "mediaRemovalConfirmation", { usedCount: -1 }],
  ["a fractional usage count", "mediaRemovalConfirmation", { usageCount: 1.5 }],
  ["a non-numeric media count", "mediaRemovalConfirmation", { count: "3" }],
  ["a non-boolean media removal busy flag", "mediaRemovalConfirmation", { busy: undefined }],
  ["a non-boolean Layout deletion busy flag", "layoutDeletionConfirmation", { busy: 1 }],
  ["a non-text Edge conversion message", "edgeConversionConfirmation", { message: 7 }],
  ["an unknown progress kind", "imageProcessingProgress", { progress: { kind: "spinner", status: "" } }],
  ["determinate progress without a total", "imageProcessingProgress", { progress: { kind: "determinate", completed: 5, status: "" } }],
  ["indeterminate progress without a status", "imageProcessingProgress", { progress: { kind: "indeterminate" } }],
  ["a progress kind inherited from Object.prototype", "imageProcessingProgress", { progress: { kind: "constructor", status: "" } }],
  ["an image problem without a reason", "imageProcessingProblems", { problems: [{ fileName: "ruim.jpg" }] }],
  ["image problems that are not a list", "imageProcessingProblems", { problems: null }],
  ["a negative imported count", "imageProcessingProblems", { importedCount: -1 }],
  ["an unknown missing-media state", "exportMediaProblems", { problems: [{ mediaId: "photo-1", fileName: "Foto.jpg", state: "lost" }] }],
  ["a non-boolean missing-media busy flag", "exportMediaProblems", { busy: null }],
  ["a non-text missing-media message", "exportMediaProblems", { message: 3 }],
  ["missing media without a Project name", "exportMediaProblems", { projectName: undefined }],
  ["a Layout problem at Frame zero", "exportProblems", { problems: [{ sheetId: "sheet-001", sheetNumber: 1, frameId: "frame-002", frameNumber: 0 }] }],
  ["Layout problems without a Project name", "exportProblems", { projectName: undefined }],
  ["a non-boolean Album information busy flag", "albumInformationConfirmation", { busy: undefined }],
  ["a non-boolean Close confirmation busy flag", "projectCloseConfirmation", { busy: "yes" }],
  ["a non-boolean format conversion flag", "projectCloseConfirmation", { formatConversion: "yes" }],
  ["a non-text Close failure message", "projectCloseFailure", { message: null }],
  ["a non-text operation failure message", "projectOperationFailure", { message: ["Falha"] }],
  ["a non-text graphics failure reason", "graphicsFailure", { reason: 404 }],
  ["a non-boolean Export cancellable flag", "exportProgress", { cancellable: "true" }],
  ["a non-boolean Export cancel-requested flag", "exportProgress", { cancelRequested: 0 }],
  ["Export progress without progress", "exportProgress", { progress: null }],
  ["a non-boolean Export retry flag", "exportFailure", { retryDisabled: undefined }],
  ["a non-boolean Export cancelled flag", "exportFailure", { cancelled: "no" }],
  ["a non-text Export failure message", "exportFailure", { message: undefined }],
  ["a non-text Export success message", "exportSuccess", { message: 1 }],
  ["a non-text Storage message", "storageFull", { message: undefined }],
  ["a non-boolean Storage clear-cache flag", "storageFull", { canClearCache: "yes" }],
  ["a non-boolean Storage busy flag", "storageFull", { busy: 0 }],
])("rejects %s in the %s state", (_name, kind, malformedFields) => {
  const accepted = stateOfKind(kind);
  expect(parseProjectDialogState(accepted)).not.toBeNull();

  const malformed = { ...accepted, ...malformedFields };
  expect(parseProjectDialogState(malformed)).toBeNull();
  expect(
    parseProjectDialogPresentation({ sessionId: "dialog-1", windowWidth: 440, state: malformed }),
  ).toBeNull();
});

test("rejects states that are not a known kind of record", () => {
  for (const value of [
    null, undefined, "exportSuccess", 7, [],
    {}, { kind: 3 }, { kind: "unknownState", message: "" },
    // Names inherited from Object.prototype are not state kinds.
    { kind: "constructor" }, { kind: "toString" }, { kind: "__proto__" },
  ]) {
    expect(parseProjectDialogState(value)).toBeNull();
  }
});

test.each(["configureExport", "chooseExportDestination"] as const)(
  "parses the %s action only with valid Export options",
  (name) => {
    const pngOptions = { ...exportOptions, scope: "album", mode: "sheet", format: { kind: "png" }, conflictPolicy: "replace" };
    expect(parseProjectDialogAction({ [name]: exportOptions })).toEqual({ [name]: exportOptions });
    expect(parseProjectDialogAction({ [name]: pngOptions })).toEqual({ [name]: pngOptions });
    // Only the validated option fields cross the seam.
    expect(
      parseProjectDialogAction({ [name]: { ...exportOptions, format: { kind: "jpeg", quality: 90, progressive: true }, overwriteSystemFiles: true } }),
    ).toEqual({ [name]: exportOptions });
    expect(
      parseProjectDialogActionEvent({ action: { [name]: exportOptions }, sessionId: "export-3" }),
    ).toEqual({ action: { [name]: exportOptions }, sessionId: "export-3" });

    for (const malformed of [
      null, undefined, "C:/Exportados", [],
      { ...exportOptions, scope: "selection" },
      { ...exportOptions, sheetIds: [] },
      { ...exportOptions, sheetIds: "sheet-001" },
      { ...exportOptions, sheetIds: ["sheet-001", ""] },
      { ...exportOptions, sheetIds: ["sheet-001", 2] },
      { ...exportOptions, sheetIds: ["sheet-001", "sheet-001"] },
      { ...exportOptions, mode: "spread" },
      { ...exportOptions, destination: undefined },
      { ...exportOptions, destination: 7 },
      { ...exportOptions, conflictPolicy: "overwrite" },
      { ...exportOptions, format: undefined },
      { ...exportOptions, format: null },
      { ...exportOptions, format: {} },
      { ...exportOptions, format: { kind: "tiff" } },
      { ...exportOptions, format: { kind: "jpeg" } },
      { ...exportOptions, format: { kind: "jpeg", quality: 0 } },
      { ...exportOptions, format: { kind: "jpeg", quality: 101 } },
      { ...exportOptions, format: { kind: "jpeg", quality: 90.5 } },
      { ...exportOptions, format: { kind: "jpeg", quality: "90" } },
      { ...exportOptions, format: { kind: "png", quality: 90 } },
      { ...exportOptions, format: { kind: "pdf", quality: 90 } },
    ]) {
      expect(parseProjectDialogAction({ [name]: malformed })).toBeNull();
      expect(
        parseProjectDialogActionEvent({ action: { [name]: malformed }, sessionId: "export-3" }),
      ).toBeNull();
    }
  },
);

test("rejects actions that are neither a known name nor a known object action", () => {
  for (const value of [
    null, undefined, 7, [], {}, ["cancelExport"],
    { cancelExport: true }, { action: "cancelExport" },
    // An inherited key is not an action the dialog sent.
    Object.create({ configureExport: exportOptions }),
    "constructor", "toString", "__proto__", "hasOwnProperty",
  ]) {
    expect(parseProjectDialogAction(value)).toBeNull();
    expect(parseProjectDialogActionEvent({ action: value, sessionId: "export-3" })).toBeNull();
  }
  expect(parseProjectDialogActionEvent({ action: "cancelExport" })).toBeNull();
  expect(parseProjectDialogActionEvent({ action: "cancelExport", sessionId: "x".repeat(129) })).toBeNull();
  expect(parseProjectDialogActionEvent({ action: "cancelExport", sessionId: "x".repeat(128) })).toEqual({
    action: "cancelExport", sessionId: "x".repeat(128),
  });
});
