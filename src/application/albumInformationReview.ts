import type {
  AlbumInformation,
  AlbumInformationImpact,
  SheetSnapshot,
} from "../domain/project";
import { albumInformationConversionLosses, type EdgeConversionLoss } from "./edgeConversionReview";

export interface AlbumInformationReview {
  readonly baseline: Readonly<AlbumInformation>;
  readonly information: Readonly<AlbumInformation>;
  readonly impact: Readonly<AlbumInformationImpact>;
  readonly conversionLosses: readonly EdgeConversionLoss[];
}

export type AlbumInformationCommitResult =
  | { kind: "completed" }
  | { kind: "reviewRequired"; review: AlbumInformationReview }
  | { kind: "rejected" };

const informationFields = [
  "displayUnit",
  "sheetWidthUm",
  "sheetHeightUm",
  "dpi",
  "bleedUm",
  "safetyUm",
  "firstSheet",
  "lastSheet",
] as const satisfies readonly (keyof AlbumInformation)[];

const physicalFields = new Set<keyof AlbumInformation>([
  "sheetWidthUm",
  "sheetHeightUm",
  "bleedUm",
  "safetyUm",
]);

const rasterFields = new Set<keyof AlbumInformation>([
  "sheetWidthUm",
  "sheetHeightUm",
  "dpi",
]);

export function createAlbumInformationReview(
  baseline: Readonly<AlbumInformation>,
  information: Readonly<AlbumInformation>,
  impact: Readonly<AlbumInformationImpact>,
  sheets: readonly SheetSnapshot[] = [],
): AlbumInformationReview {
  return { baseline, information, impact, conversionLosses: albumInformationConversionLosses(sheets, information) };
}

/**
 * Equality follows the canonical confirmation content: changed before/after
 * values, the Unit used to present changed measurements, and resulting raster
 * impact when dimensions or DPI changed, including the Core's dimensional
 * composition fingerprint. Unrelated History fields therefore do
 * not force another confirmation.
 */
export function albumInformationReviewEquals(
  left: AlbumInformationReview,
  right: AlbumInformationReview,
) {
  return JSON.stringify(reviewFacts(left)) === JSON.stringify(reviewFacts(right));
}

export function albumInformationReviewHasChanges(
  review: AlbumInformationReview,
) {
  return reviewFacts(review).changes.length > 0;
}

function reviewFacts(review: AlbumInformationReview) {
  const changes = informationFields.flatMap((field) =>
    review.baseline[field] === review.information[field]
      ? []
      : [[field, review.baseline[field], review.information[field]]],
  );
  const changedFields = new Set(changes.map(([field]) => field));
  return {
    changes,
    conversionLosses: review.conversionLosses,
    ...(informationFields.some(
      (field) => physicalFields.has(field) && changedFields.has(field),
    )
      ? { measurementUnit: review.information.displayUnit }
      : {}),
    ...(informationFields.some(
      (field) => rasterFields.has(field) && changedFields.has(field),
    )
      ? { impact: review.impact }
      : {}),
  };
}
