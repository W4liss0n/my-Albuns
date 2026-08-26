import { expect, test } from "vitest";

import type {
  ProjectDialogAction,
  ProjectDialogState,
} from "../application/projectDialogPort";
import {
  parseInitialProjectDialogPresentation,
  parseInitialProjectDialogPreviewState,
  parseProjectDialogAction,
  parseProjectDialogActionEvent,
  parseProjectDialogPresentation,
  parseProjectDialogState,
  toIpcProjectDialogAction,
  toIpcProjectDialogState,
} from "./projectDialogContract";

const states: readonly ProjectDialogState[] = [
  {
    busy: false,
    details: [{ label: "DPI", value: "300 → 240" }],
    kind: "albumInformationConfirmation",
  },
  { busy: false, kind: "projectCloseConfirmation" },
  { kind: "projectCloseFailure", message: "Falha ao fechar" },
  { kind: "projectOperationFailure", message: "Falha ao salvar" },
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
];

const actions: readonly ProjectDialogAction[] = [
  "cancelAlbumInformation",
  "cancelExport",
  "cancelProjectClose",
  "confirmAlbumInformation",
  "discardAndClose",
  "dismissExport",
  "dismissProjectCloseFailure",
  "dismissProjectOperationFailure",
  "retryExport",
  "saveAndClose",
];

test.each(states)("round-trips the $kind state through the native contract", (state) => {
  expect(parseProjectDialogState(toIpcProjectDialogState(state))).toEqual(
    state,
  );
});

test.each(actions)("round-trips the %s semantic action", (action) => {
  expect(parseProjectDialogAction(toIpcProjectDialogAction(action))).toBe(
    action,
  );
});

test("rejects malformed states and actions at the native seam", () => {
  expect(
    parseProjectDialogState({
      busy: false,
      details: ["DPI: 300 → 240"],
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
    state: states[0],
  };
  expect(parseProjectDialogPresentation(presentation)).toEqual(presentation);
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
