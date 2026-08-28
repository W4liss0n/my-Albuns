import { describe, expect, test } from "vitest";

import type { SheetSnapshot } from "../domain/project";
import {
  SHEET_REORDER_AUTO_SCROLL,
  createSheetReorderSession,
  reduceSheetReorderSession,
  sheetReorderAutoScrollVelocity,
  sheetReorderRepresentation,
} from "./sheetReorderSession";

const confirmedOrder = ["initial", "second", "third", "fourth", "final"];

describe("sheet reorder session", () => {
  test("previews only on its origin and emits one commit effect after a valid drop", () => {
    const idle = createSheetReorderSession(physicalAlbum);

    const preview = reduceSheetReorderSession(idle, physicalAlbum, {
      type: "preview",
      origin: "bar",
      draggedSheetId: "fourth",
      targetIndex: 1,
    });

    expect(preview.effect).toBeNull();
    expect(preview.session).toMatchObject({
      status: "preview",
      confirmedOrder,
      origin: "bar",
      draggedSheetId: "fourth",
      targetIndex: 1,
      previewOrder: ["initial", "fourth", "second", "third", "final"],
      placeholderIndex: 1,
      ghost: { sheetId: "fourth" },
    });
    expect(sheetReorderRepresentation(preview.session, "bar")).toEqual({
      order: ["initial", "fourth", "second", "third", "final"],
      placeholderIndex: 1,
      ghost: { sheetId: "fourth" },
    });
    expect(sheetReorderRepresentation(preview.session, "grid")).toEqual({
      order: confirmedOrder,
      placeholderIndex: null,
      ghost: null,
    });

    const committed = reduceSheetReorderSession(
      preview.session,
      physicalAlbum,
      { type: "drop", surface: "bar" },
    );
    expect(committed.session.status).toBe("committing");
    expect(committed.effect).toEqual({
      sheetId: "fourth",
      targetIndex: 1,
    });
    expect(sheetReorderRepresentation(committed.session, "bar")).toEqual({
      order: confirmedOrder,
      placeholderIndex: null,
      ghost: null,
    });

    const duplicateDrop = reduceSheetReorderSession(
      committed.session,
      physicalAlbum,
      { type: "drop", surface: "bar" },
    );
    expect(duplicateDrop.effect).toBeNull();
    expect(duplicateDrop.session).toBe(committed.session);
  });

  test("keeps invalid positions free of preview order and placeholder", () => {
    const invalid = reduceSheetReorderSession(
      createSheetReorderSession(physicalAlbum),
      physicalAlbum,
      {
        type: "preview",
        origin: "grid",
        draggedSheetId: "initial",
        targetIndex: 2,
      },
    );

    expect(invalid.session).toMatchObject({
      status: "invalid",
      confirmedOrder,
      origin: "grid",
      draggedSheetId: "initial",
      targetIndex: 2,
      previewOrder: null,
      placeholderIndex: null,
      ghost: { sheetId: "initial" },
    });
    expect(sheetReorderRepresentation(invalid.session, "grid")).toEqual({
      order: confirmedOrder,
      placeholderIndex: null,
      ghost: { sheetId: "initial" },
    });

    const dropped = reduceSheetReorderSession(
      invalid.session,
      physicalAlbum,
      { type: "drop", surface: "grid" },
    );
    expect(dropped.effect).toBeNull();
    expect(dropped.session.status).toBe("cancelled");
    expect(sheetReorderRepresentation(dropped.session, "grid")).toEqual({
      order: confirmedOrder,
      placeholderIndex: null,
      ghost: null,
    });
  });

  test("cancels a preview on Escape or a drop in the opposite surface", () => {
    const preview = reduceSheetReorderSession(
      createSheetReorderSession(physicalAlbum),
      physicalAlbum,
      {
        type: "preview",
        origin: "bar",
        draggedSheetId: "fourth",
        targetIndex: 1,
      },
    ).session;

    const escaped = reduceSheetReorderSession(preview, physicalAlbum, {
      type: "escape",
    });
    expect(escaped.effect).toBeNull();
    expect(escaped.session.status).toBe("cancelled");
    expect(sheetReorderRepresentation(escaped.session, "bar")).toEqual({
      order: confirmedOrder,
      placeholderIndex: null,
      ghost: null,
    });

    const oppositeDrop = reduceSheetReorderSession(
      preview,
      physicalAlbum,
      { type: "drop", surface: "grid" },
    );
    expect(oppositeDrop.effect).toBeNull();
    expect(oppositeDrop.session.status).toBe("cancelled");
    expect(sheetReorderRepresentation(oppositeDrop.session, "grid").order).toEqual(
      confirmedOrder,
    );
  });
});

describe("sheet reorder automatic scrolling", () => {
  test("is zero outside the calibrated edge margin", () => {
    expect(
      sheetReorderAutoScrollVelocity({
        axis: "horizontal",
        pointerPosition: 320,
        viewportStart: 0,
        viewportEnd: 640,
      }),
    ).toBe(0);
    expect(
      sheetReorderAutoScrollVelocity({
        axis: "vertical",
        pointerPosition: SHEET_REORDER_AUTO_SCROLL.edgeMarginPx,
        viewportStart: 0,
        viewportEnd: 640,
      }),
    ).toBe(0);
  });

  test("grows progressively toward each edge and clamps at the calibrated limit", () => {
    const margin = SHEET_REORDER_AUTO_SCROLL.edgeMarginPx;
    const horizontalLimit =
      SHEET_REORDER_AUTO_SCROLL.maxVelocityPxPerSecond.horizontal;
    const verticalLimit =
      SHEET_REORDER_AUTO_SCROLL.maxVelocityPxPerSecond.vertical;

    expect(
      sheetReorderAutoScrollVelocity({
        axis: "horizontal",
        pointerPosition: margin / 2,
        viewportStart: 0,
        viewportEnd: 640,
      }),
    ).toBeCloseTo(-horizontalLimit * 0.25);
    expect(
      sheetReorderAutoScrollVelocity({
        axis: "horizontal",
        pointerPosition: 640 - margin / 2,
        viewportStart: 0,
        viewportEnd: 640,
      }),
    ).toBeCloseTo(horizontalLimit * 0.25);
    expect(
      sheetReorderAutoScrollVelocity({
        axis: "horizontal",
        pointerPosition: -100,
        viewportStart: 0,
        viewportEnd: 640,
      }),
    ).toBe(-horizontalLimit);
    expect(
      sheetReorderAutoScrollVelocity({
        axis: "vertical",
        pointerPosition: 800,
        viewportStart: 0,
        viewportEnd: 640,
      }),
    ).toBe(verticalLimit);
  });

  test("rejects unusable viewport geometry", () => {
    expect(
      sheetReorderAutoScrollVelocity({
        axis: "horizontal",
        pointerPosition: 20,
        viewportStart: 100,
        viewportEnd: 100,
      }),
    ).toBe(0);
  });
});

const physicalAlbum: readonly SheetSnapshot[] = [
  sheet("initial", 1, "initial", "right"),
  sheet("second", 2, "internal", "both"),
  sheet("third", 3, "internal", "both"),
  sheet("fourth", 4, "internal", "both"),
  sheet("final", 5, "final", "left"),
];

function sheet(
  id: string,
  number: number,
  role: SheetSnapshot["role"],
  activeSides: SheetSnapshot["activeSides"],
): SheetSnapshot {
  return {
    id,
    number,
    role,
    activeSides,
    pageNumbers: [],
    widthUm: activeSides === "both" ? 600_000 : 300_000,
    heightUm: 300_000,
    frames: [],
  };
}
