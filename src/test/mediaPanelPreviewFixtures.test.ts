import { expect, test } from "vitest";

import { mediaPanelPreviewFixture } from "./mediaPanelPreviewFixtures";

test("provides a development-only imported-media catalog with usable previews", () => {
  const { mediaItems, mediaPreviews, mediaUsage } = mediaPanelPreviewFixture;

  expect(mediaItems).toHaveLength(10);
  expect(mediaItems.every(({ kind }) => kind === "photo")).toBe(true);
  expect(
    mediaItems.some(
      ({ sourceHeightPx, sourceWidthPx }) =>
        sourceHeightPx !== null &&
        sourceWidthPx !== null &&
        sourceHeightPx > sourceWidthPx,
    ),
  ).toBe(true);
  expect(
    mediaItems.some(
      ({ sourceHeightPx, sourceWidthPx }) =>
        sourceHeightPx !== null &&
        sourceWidthPx !== null &&
        sourceWidthPx > sourceHeightPx,
    ),
  ).toBe(true);

  for (const media of mediaItems) {
    expect(mediaPreviews[media.id]).toMatchObject({
      mediaId: media.id,
      state: "ready",
    });
    expect(mediaPreviews[media.id]?.url).toMatch(
      /^(?:data:image\/svg\+xml|\/.*\.svg(?:\?.*)?$)/,
    );
    expect(mediaUsage).toContainEqual(
      expect.objectContaining({ mediaId: media.id }),
    );
  }

  expect(mediaUsage.some(({ count }) => count === 0)).toBe(true);
  expect(mediaUsage.some(({ count }) => count > 1)).toBe(true);
});
