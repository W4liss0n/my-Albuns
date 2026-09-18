import { expect, test } from "vitest";

import {
  invalidPhysicalMeasurementMessage,
  parseIntegerText,
  presentConfigurationValidationErrors,
} from "./projectConfigurationFields";

const rasterLimitsAt300Dpi = {
  sheetWidth: { minimumUm: 86, maximumUm: 5_548_672 },
  sheetHeight: { minimumUm: 43, maximumUm: 5_548_672 },
};

test("presents shared Project configuration validation by field", () => {
  expect(
    presentConfigurationValidationErrors([
      "sheetWidthNotPositive",
      "bleedEliminatesCutArea",
    ], { rasterLimits: rasterLimitsAt300Dpi,
      displayUnit: "mm",
      dpi: 300,
      sheetWidthPresentation: "openSheet",
    }),
  ).toEqual({
    sheetWidth: ["A largura da Lâmina deve ser maior que zero."],
    bleed: ["A Sangria deve manter uma Área de corte positiva."],
  });
  expect(invalidPhysicalMeasurementMessage("cm")).toBe(
    "Informe uma medida válida em cm.",
  );
});

test("presents deferred content transformations on their owning fields", () => {
  expect(
    presentConfigurationValidationErrors(
      [
        "sheetDimensionsRequireContentTransformation",
        "firstSheetConversionRequiresContentReorganization",
        "lastSheetConversionRequiresContentReorganization",
      ],
      { rasterLimits: rasterLimitsAt300Dpi,
        displayUnit: "mm",
        dpi: 300,
        sheetWidthPresentation: "openSheet",
      },
    ),
  ).toEqual({
    sheetWidth: [
      "A composição existente exige o fluxo de mudança dimensional segura.",
    ],
    firstSheet: [
      "A primeira Lâmina contém composição e exige o fluxo completo de conversão.",
    ],
    lastSheet: [
      "A última Lâmina contém composição e exige o fluxo completo de conversão.",
    ],
  });
});

test("presents raster ranges in the selected physical Unit and current DPI", () => {
  expect(
    presentConfigurationValidationErrors(
      ["sheetWidthRasterOutOfRange", "sheetHeightRasterOutOfRange"],
      { rasterLimits: rasterLimitsAt300Dpi,
        displayUnit: "cm",
        dpi: 300,
        sheetWidthPresentation: "openSheet",
      },
    ),
  ).toEqual({
    sheetWidth: [
      "Para 300 DPI, informe a largura da Lâmina entre 0.0086 cm e 554.8672 cm.",
    ],
    sheetHeight: [
      "Para 300 DPI, informe a altura da Lâmina entre 0.0043 cm e 554.8672 cm.",
    ],
  });

  expect(
    presentConfigurationValidationErrors(
      ["sheetHeightRasterOutOfRange"],
      { rasterLimits: rasterLimitsAt300Dpi,
        displayUnit: "in",
        dpi: 300,
        sheetWidthPresentation: "openSheet",
      },
    ).sheetHeight,
  ).toEqual([
    "Para 300 DPI, informe a altura da Lâmina entre aproximadamente 0.002 pol e 218.452 pol.",
  ]);
});

test("presents the creation width as a closed Sheet measurement", () => {
  expect(
    presentConfigurationValidationErrors(
      ["sheetWidthRasterOutOfRange"],
      { rasterLimits: rasterLimitsAt300Dpi,
        displayUnit: "cm",
        dpi: 300,
        sheetWidthPresentation: "closedSheet",
      },
    ).sheetWidth,
  ).toEqual([
    "Para 300 DPI, informe a largura da Lâmina fechada entre 0.0043 cm e 277.4336 cm.",
  ]);
});

test("parses supported integers and rejects oversized text", () => {
  expect(parseIntegerText(" 300 ")).toBe(300);
  expect(parseIntegerText("3.5")).toBeNull();
  expect(parseIntegerText("0".repeat(256))).toBeNull();
});

test("formats the supplied Core limits without reconstructing the raster rule", () => {
  const rasterLimits = {
    sheetWidth: { minimumUm: 2_000, maximumUm: 4_000 },
    sheetHeight: { minimumUm: 500, maximumUm: 900 },
  };
  expect(presentConfigurationValidationErrors(
    ["sheetWidthRasterOutOfRange", "sheetHeightRasterOutOfRange"],
    { rasterLimits, dpi: 300, displayUnit: "mm", sheetWidthPresentation: "closedSheet" },
  )).toEqual({
    sheetWidth: ["Para 300 DPI, informe a largura da Lâmina fechada entre 1 mm e 2 mm."],
    sheetHeight: ["Para 300 DPI, informe a altura da Lâmina entre 0.5 mm e 0.9 mm."],
  });
});
