import { expect, test } from "vitest";

import type { CompositionPlan } from "../domain/project";
import {
  createContinuousCanvasLayout,
  SHEET_GAP_PX,
} from "./canvasGeometry";

function neutralLayers() {
  return {
    base: {
      rgb: "#FFFFFF",
      drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
    },
    backgrounds: [
      {
        kind: "color" as const,
        rgb: "#FFFFFF",
        drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
      },
    ],
    overlays: [],
  };
}

const threeSheets: CompositionPlan["sheets"] = [
  {
    sheetId: "sheet-001",
    number: 1,
    activeSides: "both",
    widthUm: 600_000,
    heightUm: 300_000,
    ...neutralLayers(),
    frames: [],
  },
  {
    sheetId: "sheet-002",
    number: 2,
    activeSides: "both",
    widthUm: 600_000,
    heightUm: 300_000,
    ...neutralLayers(),
    frames: [],
  },
  {
    sheetId: "sheet-003",
    number: 3,
    activeSides: "both",
    widthUm: 600_000,
    heightUm: 300_000,
    ...neutralLayers(),
    frames: [],
  },
];

test("limits continuous Canvas movement at the edge sheet centers", () => {
  const layout = createContinuousCanvasLayout(threeSheets);

  expect(layout.clampOffset(999, 0.5, 1_000)).toBe(350);
  expect(layout.clampOffset(-999, 0.5, 1_000)).toBe(-342);
  expect(layout.clampOffset(4, 0.5, 1_000)).toBe(4);
});

test("identifies the sheet nearest the visible Canvas center", () => {
  const layout = createContinuousCanvasLayout(threeSheets);

  expect(layout.centeredSheetId(350, 0.5, 1_000)).toBe(
    "sheet-001",
  );
  expect(layout.centeredSheetId(4, 0.5, 1_000)).toBe(
    "sheet-002",
  );
  expect(layout.centeredSheetId(-342, 0.5, 1_000)).toBe(
    "sheet-003",
  );
});

test("calculates the offset that centers any navigation target", () => {
  const layout = createContinuousCanvasLayout(threeSheets);

  expect(layout.centeredOffset("sheet-001", 0.5, 1_000)).toBe(350);
  expect(layout.centeredOffset("sheet-002", 0.5, 1_000)).toBe(4);
  expect(layout.centeredOffset("sheet-003", 0.5, 1_000)).toBe(
    -342,
  );
  expect(layout.centeredOffset("missing", 0.5, 1_000)).toBeNull();
});

test("keeps the visible gap between sheets fixed while Canvas scale changes", () => {
  const layout = createContinuousCanvasLayout(threeSheets);

  for (const scale of [0.5, 1, 1.75]) {
    const [first, second] = layout.entriesAtScale(scale);
    const visibleGap = (second.left - first.right) * scale;

    expect(visibleGap).toBeCloseTo(SHEET_GAP_PX);
  }
});

test("measures sheet geometry once and reuses it", () => {
  let widthReads = 0;
  const measuredSheets: CompositionPlan["sheets"] = threeSheets.map(
    (sheet) => ({
      ...sheet,
      get widthUm() {
        widthReads += 1;
        return sheet.widthUm;
      },
    }),
  );

  const layout = createContinuousCanvasLayout(measuredSheets);
  expect(widthReads).toBe(threeSheets.length);

  layout.clampOffset(999, 0.5, 1_000);
  layout.centeredSheetId(4, 0.5, 1_000);
  layout.centeredOffset("sheet-003", 0.5, 1_000);
  expect(widthReads).toBe(threeSheets.length);
});
