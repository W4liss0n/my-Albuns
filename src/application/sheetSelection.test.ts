// @vitest-environment node
import { expect, test } from "vitest";

import {
  effectiveSelectedSheetIds,
  nextSheetSelection,
  PLAIN_SHEET_SELECTION,
  sheetCommandTargetIds,
  sheetContextMenuTargetIds,
  sheetSelectionModifiers,
  type SheetSelection,
} from "./sheetSelection";

const ORDER = ["s1", "s2", "s3", "s4", "s5"];
const CTRL = { toggle: true, range: false };
const SHIFT = { toggle: false, range: true };
const CTRL_SHIFT = { toggle: true, range: true };

function only(sheetId: string): SheetSelection {
  return {
    selectedSheetIds: [sheetId],
    focusedSheetId: sheetId,
    anchorSheetId: sheetId,
  };
}

function click(
  current: SheetSelection,
  sheetId: string,
  modifiers = PLAIN_SHEET_SELECTION,
) {
  return nextSheetSelection(current, sheetId, modifiers, ORDER);
}

test("a plain click replaces the selection and moves the anchor", () => {
  const several = click(click(only("s1"), "s3", CTRL), "s4", CTRL);
  expect(click(several, "s2")).toEqual(only("s2"));
});

test("Ctrl adds and removes Sheets in Album order and moves the anchor", () => {
  const added = click(click(only("s4"), "s1", CTRL), "s2", CTRL);
  expect(added).toEqual({
    selectedSheetIds: ["s1", "s2", "s4"],
    focusedSheetId: "s2",
    anchorSheetId: "s2",
  });

  expect(click(added, "s1", CTRL)).toEqual({
    selectedSheetIds: ["s2", "s4"],
    focusedSheetId: "s2",
    anchorSheetId: "s1",
  });
});

test("removing the focused Sheet with Ctrl focuses the nearest one left", () => {
  const selection = click(click(only("s1"), "s5", CTRL), "s3", CTRL);
  expect(click(selection, "s3", CTRL)).toEqual({
    selectedSheetIds: ["s1", "s5"],
    focusedSheetId: "s1",
    anchorSheetId: "s3",
  });
});

test("Ctrl on the only selected Sheet keeps it selected", () => {
  expect(click(only("s2"), "s2", CTRL)).toEqual(only("s2"));
});

test("Shift selects the range from the anchor in either direction", () => {
  expect(click(only("s2"), "s4", SHIFT)).toEqual({
    selectedSheetIds: ["s2", "s3", "s4"],
    focusedSheetId: "s4",
    anchorSheetId: "s2",
  });
  const backwards = click(click(only("s2"), "s4", SHIFT), "s1", SHIFT);
  expect(backwards).toEqual({
    selectedSheetIds: ["s1", "s2"],
    focusedSheetId: "s1",
    anchorSheetId: "s2",
  });
});

test("Ctrl+Shift adds the range to the current selection", () => {
  const selection = click(only("s5"), "s1", CTRL);
  expect(click(selection, "s3", CTRL_SHIFT)).toEqual({
    selectedSheetIds: ["s1", "s2", "s3", "s5"],
    focusedSheetId: "s3",
    anchorSheetId: "s1",
  });
});

test("Shift without a known anchor acts as a plain or Ctrl click", () => {
  const lostAnchor = { ...only("s2"), anchorSheetId: "deleted" };
  expect(click(lostAnchor, "s4", SHIFT)).toEqual(only("s4"));
  expect(click(lostAnchor, "s4", CTRL_SHIFT)).toEqual({
    selectedSheetIds: ["s2", "s4"],
    focusedSheetId: "s4",
    anchorSheetId: "s4",
  });
});

test("ignores unknown Sheets and drops stale ids from the result", () => {
  const stale = {
    selectedSheetIds: ["gone", "s1", "s2"],
    focusedSheetId: "s2",
    anchorSheetId: "s2",
  };
  expect(click(stale, "missing")).toBe(stale);
  expect(click(stale, "s3", CTRL).selectedSheetIds).toEqual([
    "s1",
    "s2",
    "s3",
  ]);
});

test("a list without the focused Sheet means the focused Sheet alone", () => {
  expect(effectiveSelectedSheetIds(["s1", "s2"], "s4")).toEqual(["s4"]);
  expect(effectiveSelectedSheetIds(["s1", "s2"], "s2")).toEqual(["s1", "s2"]);
  expect(
    click(
      { selectedSheetIds: ["s1", "s2"], focusedSheetId: "s4", anchorSheetId: "s4" },
      "s5",
      CTRL,
    ).selectedSheetIds,
  ).toEqual(["s4", "s5"]);
});

test("reads Ctrl, Cmd and Shift from pointer and keyboard events", () => {
  expect(
    sheetSelectionModifiers({ ctrlKey: false, metaKey: true, shiftKey: false }),
  ).toEqual(CTRL);
  expect(
    sheetSelectionModifiers({ ctrlKey: true, metaKey: false, shiftKey: true }),
  ).toEqual(CTRL_SHIFT);
});

test("Delete and Duplicate target a Grade selection only while the Grade shows it", () => {
  const grid = {
    gridShowsSelection: true,
    selectedSheetIds: ["s2", "s4"],
    source: "grid" as const,
    implicitSheetId: "s1",
  };
  expect(sheetCommandTargetIds(grid)).toEqual(["s2", "s4"]);
  expect(sheetCommandTargetIds({ ...grid, gridShowsSelection: false })).toEqual(["s1"]);
  expect(sheetCommandTargetIds({ ...grid, source: "focus" })).toEqual(["s1"]);
  expect(
    sheetCommandTargetIds({ ...grid, source: "focus", implicitSheetId: null }),
  ).toEqual([]);
});

test("a Sheet menu acts on the whole target only when it was opened on one of several", () => {
  expect(sheetContextMenuTargetIds("s4", ["s2", "s4"])).toEqual(["s2", "s4"]);
  expect(sheetContextMenuTargetIds("s3", ["s2", "s4"])).toEqual(["s3"]);
  expect(sheetContextMenuTargetIds("s1", ["s1"])).toEqual(["s1"]);
});
