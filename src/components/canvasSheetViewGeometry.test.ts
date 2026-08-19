import { expect, test } from "vitest";

import type { ComposedSheet } from "../domain/project";
import { composition } from "./albumCanvasTestFixtures";
import { createCanvasSheetPresentation } from "./canvasGeometry";
import {
  createCanvasSheetViewGeometry,
  createNormalCanvasLayout,
} from "./canvasSheetViewGeometry";

test("projects the visible cut bounds of a double sheet", () => {
  const sheet = composition.sheets[0];
  const geometry = createCanvasSheetViewGeometry(
    sheet,
    createCanvasSheetPresentation(sheet),
    3_000,
    true,
  );

  expect(geometry).toEqual({
    activeBounds: { x: 3, y: 3, width: 594, height: 294 },
    inactiveSideBounds: null,
    visibleOuterBounds: { x: 3, y: 3, width: 594, height: 294 },
  });
});

test.each([
  {
    activeSides: "right" as const,
    activeBounds: { x: 300, y: 3, width: 297, height: 294 },
    inactiveSideBounds: { x: 3, y: 3, width: 297, height: 294 },
  },
  {
    activeSides: "left" as const,
    activeBounds: { x: 3, y: 3, width: 297, height: 294 },
    inactiveSideBounds: { x: 300, y: 3, width: 297, height: 294 },
  },
])(
  "preserves the inactive side while cropping a $activeSides single Page",
  ({ activeSides, activeBounds, inactiveSideBounds }) => {
    const sheet = singlePageSheet(activeSides);
    const geometry = createCanvasSheetViewGeometry(
      sheet,
      createCanvasSheetPresentation(sheet),
      3_000,
      true,
    );

    expect(geometry).toEqual({
      activeBounds,
      inactiveSideBounds,
      visibleOuterBounds: { x: 3, y: 3, width: 594, height: 294 },
    });
  },
);

test("keeps the complete sheet bounds in Sheet Edit Mode", () => {
  const sheet = composition.sheets[0];
  const geometry = createCanvasSheetViewGeometry(
    sheet,
    createCanvasSheetPresentation(sheet),
    3_000,
    false,
  );

  expect(geometry.visibleOuterBounds).toEqual({
    x: 0,
    y: 0,
    width: 600,
    height: 300,
  });
});

test("keeps the requested screen gap between visible cut areas", () => {
  const sheets = [
    composition.sheets[0],
    { ...composition.sheets[0], sheetId: "sheet-002", number: 2 },
  ];
  const entries = createNormalCanvasLayout(sheets, 3_000).entriesAtScale(2);

  expect(entries[0]).toMatchObject({ left: 0, right: 594, width: 594 });
  expect(entries[1]).toMatchObject({ left: 617, width: 594 });
});

function singlePageSheet(
  activeSides: "left" | "right",
): ComposedSheet {
  const source = composition.sheets[0];
  const drawRect = { x: 0, y: 0, width: 300_000, height: 300_000 };
  return {
    ...source,
    activeSides,
    widthUm: 300_000,
    base: { ...source.base, drawRect },
    backgrounds: [{ kind: "color", rgb: "#FFFFFF", drawRect }],
  };
}
