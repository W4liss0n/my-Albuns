// @vitest-environment node
import { expect, test } from "vitest";

import type { SheetSnapshot } from "../domain/project";
import {
  isSheetStructureIntent,
  materializeSheetStructureIntent,
  sheetSelectionAvailability,
} from "./sheetStructure";

/** Only the facts these rules read: identity and the core's per-Sheet availability. */
function album(sides: readonly ("both" | "left" | "right")[]): SheetSnapshot[] {
  return sides.map((activeSides, index) => ({
    id: `s${index + 1}`,
    activeSides,
    structure: {
      availability: {
        canAddAfter: true,
        canAddBefore: true,
        canConvertEdge: false,
        canDelete: sides.length > 2,
        canDuplicate: activeSides === "both",
      },
      minimumReorderIndex: 0,
      maximumReorderIndex: sides.length - 1,
    },
  }) as SheetSnapshot);
}

test("several Sheets can be deleted while two remain and duplicated when all are double", () => {
  const sheets = album(["right", "both", "both", "both", "left"]);

  expect(sheetSelectionAvailability(sheets, ["s2", "s4"])).toEqual({
    canDelete: true,
    canDuplicate: true,
  });
  expect(sheetSelectionAvailability(sheets, ["s1", "s2", "s3"])).toEqual({
    canDelete: true,
    canDuplicate: false,
  });
  expect(
    sheetSelectionAvailability(sheets, ["s1", "s2", "s3", "s4"]).canDelete,
  ).toBe(false);
});

test("a selection with an unknown or no Sheet allows nothing; repeats count once", () => {
  const sheets = album(["both", "both", "both"]);
  const nothing = { canDelete: false, canDuplicate: false };

  expect(sheetSelectionAvailability(sheets, [])).toEqual(nothing);
  expect(sheetSelectionAvailability(sheets, ["s1", "gone"])).toEqual(nothing);
  expect(sheetSelectionAvailability(sheets, ["s1", "s1"])).toEqual(
    sheetSelectionAvailability(sheets, ["s1"]),
  );
});

test("one Sheet follows the per-Sheet availability from the core", () => {
  const sheets = album(["right", "both", "left"]);
  for (const sheet of sheets) {
    const { canDelete, canDuplicate } = sheet.structure.availability;
    expect(sheetSelectionAvailability(sheets, [sheet.id])).toEqual({
      canDelete,
      canDuplicate,
    });
  }
});

test("a queued plural command is cancelled when the latest Album no longer allows it", () => {
  const captured = album(["both", "both", "both", "both"]);
  const deleteTwo = { kind: "deleteSheets" as const, sheetIds: ["s2", "s3"] };
  const duplicateTwo = { kind: "duplicateSheets" as const, sheetIds: ["s2", "s3"] };
  expect(isSheetStructureIntent(deleteTwo)).toBe(true);
  expect(isSheetStructureIntent(duplicateTwo)).toBe(true);

  expect(materializeSheetStructureIntent(captured, captured, deleteTwo)).toBe(deleteTwo);
  const shorter = captured.filter((sheet) => sheet.id !== "s4");
  expect(materializeSheetStructureIntent(captured, shorter, deleteTwo)).toBeNull();
  const withoutTarget = captured.filter((sheet) => sheet.id !== "s3");
  expect(materializeSheetStructureIntent(captured, withoutTarget, duplicateTwo)).toBeNull();
  expect(materializeSheetStructureIntent(captured, captured, duplicateTwo)).toBe(duplicateTwo);
});
