import { describe, expect, test } from "vitest";

import type { SheetSnapshot } from "../domain/project";
import {
  planSheetReorder,
  sheetStructureAvailability,
} from "./sheetStructure";

function sheet(
  id: string,
  activeSides: SheetSnapshot["activeSides"] = "both",
): SheetSnapshot {
  return {
    activeSides,
    frames: [],
    heightUm: 300_000,
    id,
    number: 0,
    pageNumbers: [],
    role: "internal",
    widthUm: activeSides === "both" ? 600_000 : 300_000,
  };
}

const physicalAlbum = [
  sheet("initial", "right"),
  sheet("second"),
  sheet("third"),
  sheet("fourth"),
  sheet("final", "left"),
];

describe("physical Album structure projection", () => {
  test("disables only external insertion around single-Page ends", () => {
    expect(sheetStructureAvailability(physicalAlbum, "initial")).toEqual({
      canAddAfter: true,
      canAddBefore: false,
      canDelete: true,
    });
    expect(sheetStructureAvailability(physicalAlbum, "third")).toEqual({
      canAddAfter: true,
      canAddBefore: true,
      canDelete: true,
    });
    expect(sheetStructureAvailability(physicalAlbum, "final")).toEqual({
      canAddAfter: false,
      canAddBefore: true,
      canDelete: true,
    });
    expect(
      sheetStructureAvailability(physicalAlbum.slice(0, 2), "initial"),
    ).toMatchObject({ canDelete: false });
  });

  test("previews a valid final order without mutating the opposite surface", () => {
    const plan = planSheetReorder(physicalAlbum, "fourth", 1);
    expect(plan).toEqual({
      changed: true,
      order: ["initial", "fourth", "second", "third", "final"],
      sourceIndex: 3,
      targetIndex: 1,
      valid: true,
    });
    expect(physicalAlbum.map(({ id }) => id)).toEqual([
      "initial",
      "second",
      "third",
      "fourth",
      "final",
    ]);
  });

  test("offers no placeholder for no-op, out-of-range, or interior single-Page drops", () => {
    expect(planSheetReorder(physicalAlbum, "third", 2)).toMatchObject({
      changed: false,
      valid: false,
    });
    expect(planSheetReorder(physicalAlbum, "third", -1)).toMatchObject({
      changed: false,
      valid: false,
    });
    expect(planSheetReorder(physicalAlbum, "initial", 2)).toMatchObject({
      changed: true,
      valid: false,
    });
    expect(planSheetReorder(physicalAlbum, "final", 0)).toMatchObject({
      changed: true,
      valid: false,
    });
  });
});
