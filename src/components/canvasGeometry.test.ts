import { expect, test } from "vitest";

import type { CompositionPlan } from "../domain/project";
import {
  centeredSheetIdInContinuousCanvas,
  clampContinuousCanvasOffset,
} from "./canvasGeometry";

const threeSheets: CompositionPlan["sheets"] = [
  {
    sheetId: "sheet-001",
    number: 1,
    widthUm: 600_000,
    heightUm: 300_000,
    hasOverlay: false,
    frames: [],
  },
  {
    sheetId: "sheet-002",
    number: 2,
    widthUm: 600_000,
    heightUm: 300_000,
    hasOverlay: false,
    frames: [],
  },
  {
    sheetId: "sheet-003",
    number: 3,
    widthUm: 600_000,
    heightUm: 300_000,
    hasOverlay: false,
    frames: [],
  },
];

test("limits continuous Canvas movement at the edge sheet centers", () => {
  expect(
    clampContinuousCanvasOffset(threeSheets, 999, 0.5, 1_000),
  ).toBe(350);
  expect(
    clampContinuousCanvasOffset(threeSheets, -999, 0.5, 1_000),
  ).toBe(-302);
  expect(
    clampContinuousCanvasOffset(threeSheets, 24, 0.5, 1_000),
  ).toBe(24);
});

test("identifies the sheet nearest the visible Canvas center", () => {
  expect(
    centeredSheetIdInContinuousCanvas(
      threeSheets,
      350,
      0.5,
      1_000,
    ),
  ).toBe("sheet-001");
  expect(
    centeredSheetIdInContinuousCanvas(
      threeSheets,
      24,
      0.5,
      1_000,
    ),
  ).toBe("sheet-002");
  expect(
    centeredSheetIdInContinuousCanvas(
      threeSheets,
      -302,
      0.5,
      1_000,
    ),
  ).toBe("sheet-003");
});
