import { expect, test } from "vitest";
import { createThreeSheetProjection } from "../test/projectFixtures";
import { albumInformationConversionLosses, edgeConversionLoss, edgeConversionLossDescription } from "./edgeConversionReview";
import { createAlbumInformationReview, albumInformationReviewEquals } from "./albumInformationReview";

const background = { kind: "custom", content: { kind: "color", rgb: "#123456" }, mapping: "side" } as const;
const overlay = { kind: "custom", content: { kind: "media", mediaId: "overlay-1" }, mapping: "side" } as const;

test.each([0, 2])("reviews only the disappearing applications on edge %s", (index) => {
  const sheets = createThreeSheetProjection().state.album.sheets;
  const sheet = sheets[index];
  sheet.activeSides = "both";
  sheet.visuals = {
    background: { kind: "perSide", left: background, right: background },
    overlay: { kind: "perSide", left: overlay, right: overlay },
  };
  const loss = edgeConversionLoss(sheets, sheet.id)!;
  expect(loss).toEqual({ sheetId: sheet.id, sheetNumber: sheet.number,
    side: index === 0 ? "left" : "right", background, overlay });
  expect(edgeConversionLossDescription(loss)).toContain("Background e Overlay personalizados");
  expect(edgeConversionLossDescription(loss)).toContain(index === 0 ? "esquerda" : "direita");
});

test.each(["default", "bothSides", "keptSide", "emptyOverlay", "expansion", "internal"])(
  "does not warn for %s", (mode) => {
    const sheets = createThreeSheetProjection().state.album.sheets;
    const sheet = sheets[mode === "internal" ? 1 : 0];
    sheet.activeSides = mode === "expansion" ? "right" : "both";
    sheet.visuals = { background: { kind: "default" }, overlay: { kind: "default" } };
    if (mode === "bothSides") sheet.visuals.background = { kind: "bothSides", content: background.content };
    if (mode === "keptSide") sheet.visuals.background = { kind: "perSide", left: { kind: "default" }, right: background };
    if (mode === "emptyOverlay") sheet.visuals.overlay = { kind: "perSide", left: { ...overlay, content: null }, right: { kind: "default" } };
    if (mode === "expansion" || mode === "internal") sheet.visuals.background = { kind: "perSide", left: background, right: background };
    expect(edgeConversionLoss(sheets, sheet.id)).toBeNull();
  },
);

test("Album information review detects changed local content, but ignores the retained page", () => {
  const projection = createThreeSheetProjection();
  const sheets = projection.state.album.sheets;
  sheets[0].activeSides = "both";
  sheets[0].visuals = { background: { kind: "perSide", left: background, right: background }, overlay: { kind: "default" } };
  const baseline = { ...projection.state.document, firstSheet: "double" as const, lastSheet: "double" as const };
  const information = { ...baseline, firstSheet: "singlePage" as const };
  const impact = { sheetWidthPx: 100, pageWidthPx: 50, heightPx: 50 };
  const review = createAlbumInformationReview(baseline, information, impact, sheets);
  expect(albumInformationConversionLosses(sheets, information)).toHaveLength(1);
  const changed = structuredClone(sheets);
  changed[0].visuals = { background: { kind: "perSide", left: background, right: { kind: "default" } }, overlay: { kind: "default" } };
  expect(albumInformationReviewEquals(review, createAlbumInformationReview(baseline, information, impact, changed))).toBe(true);
  changed[0].visuals.background = { kind: "perSide", left: { ...background, content: { kind: "media", mediaId: "new-background" } }, right: background };
  expect(albumInformationReviewEquals(review, createAlbumInformationReview(baseline, information, impact, changed))).toBe(false);
});
