import { expect, test } from "vitest";

import type { ProjectDialogState } from "../../application/projectDialogPort";
import { defaultProjectDialogCloseAction } from "./projectDialogLifecycle";

test.each<{
  expected: ReturnType<typeof defaultProjectDialogCloseAction>;
  state: ProjectDialogState;
}>([
  { expected: null, state: { kind: "imageProcessingProgress", progress: { kind: "determinate", completed: 5, total: 12, status: "5 de 12" } } },
  {
    expected: "cancelAlbumInformation",
    state: {
      busy: false,
      details: [],
      kind: "albumInformationConfirmation",
    },
  },
  {
    expected: null,
    state: {
      busy: true,
      details: [],
      kind: "albumInformationConfirmation",
    },
  },
  {
    expected: "cancelProjectClose",
    state: { busy: false, kind: "projectCloseConfirmation" },
  },
  {
    expected: null,
    state: { busy: true, kind: "projectCloseConfirmation" },
  },
  {
    expected: "dismissProjectCloseFailure",
    state: { kind: "projectCloseFailure", message: "Falhou" },
  },
  { expected: "dismissImageProcessingProblems", state: { kind: "imageProcessingProblems", importedCount: 0, problems: [] } },
  {
    expected: "dismissProjectOperationFailure",
    state: { kind: "projectOperationFailure", message: "Falhou" },
  },
  {
    expected: "closeProjectAfterGraphicsFailure",
    state: { kind: "graphicsFailure", reason: "WebGL2 indisponível" },
  },
  {
    expected: "cancelExport",
    state: {
      cancelRequested: false,
      cancellable: true,
      kind: "exportProgress",
      progress: { kind: "indeterminate", status: "Exportando" },
    },
  },
  {
    expected: null,
    state: {
      cancelRequested: true,
      cancellable: true,
      kind: "exportProgress",
      progress: { kind: "indeterminate", status: "Cancelando" },
    },
  },
  {
    expected: null,
    state: {
      cancelRequested: false,
      cancellable: false,
      kind: "exportProgress",
      progress: { kind: "indeterminate", status: "Publicando" },
    },
  },
  {
    expected: "dismissExport",
    state: {
      cancelled: false,
      kind: "exportFailure",
      message: "Falhou",
      retryDisabled: false,
    },
  },
  {
    expected: "dismissExport",
    state: { kind: "exportSuccess", message: "Concluído" },
  },
])("maps $state.kind to its accepted close action", ({ expected, state }) => {
  expect(defaultProjectDialogCloseAction(state)).toBe(expected);
});
