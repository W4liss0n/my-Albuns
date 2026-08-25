import { expect, test } from "vitest";

import { parseProjectDialogState } from "./projectDialogState";

test("accepts structured Album information details", () => {
  expect(
    parseProjectDialogState({
      busy: false,
      details: [{ label: "DPI", value: "300 → 240" }],
      kind: "albumInformationConfirmation",
    }),
  ).toEqual({
    busy: false,
    details: [{ label: "DPI", value: "300 → 240" }],
    kind: "albumInformationConfirmation",
  });
  expect(
    parseProjectDialogState({
      busy: false,
      details: ["DPI: 300 → 240"],
      kind: "albumInformationConfirmation",
    }),
  ).toBeNull();
});

test("accepts standard operation failure and export success states", () => {
  expect(
    parseProjectDialogState({
      kind: "projectOperationFailure",
      message: "O Projeto não pôde ser salvo.",
    }),
  ).toEqual({
    kind: "projectOperationFailure",
    message: "O Projeto não pôde ser salvo.",
  });
  expect(
    parseProjectDialogState({
      kind: "exportSuccess",
      message: "A prova foi exportada com sucesso.",
    }),
  ).toEqual({
    kind: "exportSuccess",
    message: "A prova foi exportada com sucesso.",
  });
});

test("rejects malformed standard message states", () => {
  expect(
    parseProjectDialogState({
      kind: "projectOperationFailure",
      message: 42,
    }),
  ).toBeNull();
  expect(
    parseProjectDialogState({ kind: "exportSuccess" }),
  ).toBeNull();
});
