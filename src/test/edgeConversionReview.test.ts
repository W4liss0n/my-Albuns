import { expect, test } from "vitest";
import { createThreeSheetProjection } from "./projectFixtures";
import { edgeConversionLoss, edgeConversionLossDescription } from "../application/edgeConversionReview";
import { createAlbumInformationReview, albumInformationReviewEquals } from "../application/albumInformationReview";
import type { EdgeConversionLoss } from "../domain/project";

const background = { kind: "custom", content: { kind: "color", rgb: "#123456" }, mapping: "side" } as const;
const overlay = { kind: "custom", content: { kind: "media", mediaId: "overlay-1" }, mapping: "side" } as const;

test.each([0, 2])("presents the Core's loss facts for edge %s without deriving them from visuals", (index) => {
  const sheets = createThreeSheetProjection().state.album.sheets;
  const sheet = sheets[index];
  const loss: EdgeConversionLoss = { sheetId: sheet.id, sheetNumber: sheet.number,
    side: index === 0 ? "left" : "right", background, overlay };
  sheet.edgeConversionLoss = loss;
  expect(edgeConversionLoss(sheets, sheet.id)).toBe(loss);
  expect(edgeConversionLossDescription(loss)).toContain("Background e Overlay personalizados");
  expect(edgeConversionLossDescription(loss)).toContain(index === 0 ? "esquerda" : "direita");
  sheet.edgeConversionLoss = null;
  sheet.visuals = { background: { kind: "perSide", left: background, right: background }, overlay: { kind: "default" } };
  expect(edgeConversionLoss(sheets, sheet.id)).toBeNull();
  expect(edgeConversionLoss(sheets, "missing")).toBeNull();
});

test("Album information compares the supplied impact, including changed lost content", () => {
  const projection = createThreeSheetProjection();
  const baseline = { ...projection.state.document, firstSheet: "double" as const, lastSheet: "double" as const };
  const information = { ...baseline, firstSheet: "singlePage" as const };
  const loss: EdgeConversionLoss = { sheetId: "sheet-001", sheetNumber: 1, side: "left", background, overlay: null };
  const impact = { conversionLosses: [loss], sheetWidthPx: 100, pageWidthPx: 50, heightPx: 50 };
  const review = createAlbumInformationReview(baseline, information, impact);
  expect(review.conversionLosses).toEqual([loss]);
  expect(albumInformationReviewEquals(review, createAlbumInformationReview(baseline, information, structuredClone(impact)))).toBe(true);
  const changed = { ...impact, conversionLosses: [{ ...loss, background: null, overlay }] };
  expect(albumInformationReviewEquals(review, createAlbumInformationReview(baseline, information, changed))).toBe(false);
});
