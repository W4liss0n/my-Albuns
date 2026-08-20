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

  expect(details).toContain(
    "Resolução final: Lâmina 6.614 × 3.307 px · Página 3.307 × 3.307 px",
  );
  expect(details).toContain(
    "Dimensão: a proporção da composição será preservada no novo formato.",
  );
  expect(details).toContain(
    "Extremidades: Lâmina dupla / Lâmina dupla → Página única / Lâmina dupla",
  );
});
