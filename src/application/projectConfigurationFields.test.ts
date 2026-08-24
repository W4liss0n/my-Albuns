import { expect, test } from "vitest";

import {
  INVALID_PHYSICAL_MEASUREMENT_MESSAGE,
  parseIntegerText,
  presentConfigurationValidationErrors,
} from "./projectConfigurationFields";

test("presents shared Project configuration validation by field", () => {
  expect(
    presentConfigurationValidationErrors([
      "sheetWidthNotPositive",
      "bleedEliminatesCutArea",
    ]),
  ).toEqual({
    sheetWidth: ["A largura da Lâmina deve ser maior que zero."],
    bleed: ["A Sangria deve manter uma Área de corte positiva."],
  });
  expect(INVALID_PHYSICAL_MEASUREMENT_MESSAGE).toBe(
    "Informe uma medida decimal que corresponda a micrômetros inteiros.",
  );
});

test("parses supported integers and rejects oversized text", () => {
  expect(parseIntegerText(" 300 ")).toBe(300);
  expect(parseIntegerText("3.5")).toBeNull();
  expect(parseIntegerText("0".repeat(256))).toBeNull();
});
