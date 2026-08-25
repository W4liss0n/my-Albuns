import { expect, test } from "vitest";

import type { AlbumInformation } from "../domain/project";
import { albumInformationDetails } from "./useAlbumInformationApplyController";

const baseline: AlbumInformation = {
  displayUnit: "mm",
  sheetWidthUm: 600_000,
  sheetHeightUm: 300_000,
  dpi: 300,
  bleedUm: 3_000,
  safetyUm: 3_000,
  firstSheet: "double",
  lastSheet: "double",
};

test("describes only the Album information field that actually changed", () => {
  const details = albumInformationDetails(
    { ...baseline, firstSheet: "singlePage" },
    baseline,
    {
      sheetWidthPx: 7_087,
      pageWidthPx: 3_543,
      heightPx: 3_543,
    },
  );

  expect(details).toEqual([
    {
      label: "Primeira Lâmina",
      value: "Lâmina dupla → Página única",
    },
  ]);
});

test("describes final raster size and structural and dimensional impact", () => {
  const details = albumInformationDetails(
    {
      ...baseline,
      sheetWidthUm: 700_000,
      sheetHeightUm: 350_000,
      dpi: 240,
      firstSheet: "singlePage",
    },
    baseline,
    {
      sheetWidthPx: 6_614,
      pageWidthPx: 3_307,
      heightPx: 3_307,
    },
  );

  expect(details).toEqual([
    {
      label: "Primeira Lâmina",
      value: "Lâmina dupla → Página única",
    },
    { label: "DPI", value: "300 → 240" },
    { label: "Largura da Lâmina", value: "600 mm → 700 mm" },
    { label: "Altura da Lâmina", value: "300 mm → 350 mm" },
    {
      label: "Resolução resultante",
      value: "Lâmina 6.614 × 3.307 px · Página 3.307 × 3.307 px",
    },
    {
      label: "Composição",
      value: "A proporção será preservada no novo formato.",
    },
  ]);
});

test("uses the selected Unit for changed measurements without unrelated raster details", () => {
  const details = albumInformationDetails(
    {
      ...baseline,
      bleedUm: 5_000,
      displayUnit: "cm",
    },
    baseline,
    {
      sheetWidthPx: 7_087,
      pageWidthPx: 3_543,
      heightPx: 3_543,
    },
  );

  expect(details).toEqual([
    { label: "Unidade", value: "mm → cm" },
    { label: "Sangria", value: "0.3 cm → 0.5 cm" },
  ]);
});
