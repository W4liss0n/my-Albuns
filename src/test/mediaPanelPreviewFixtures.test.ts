import { expect, test } from "vitest";

import { mediaPanelPreviewFixture } from "./mediaPanelPreviewFixtures";

test("provides a development-only catalog for both acceptance tabs", () => {
  const { mediaItems, mediaPreviews, mediaUsage } = mediaPanelPreviewFixture;

  expect(mediaItems).toHaveLength(12);
  expect(new Set(mediaItems.map(({ kind }) => kind))).toEqual(
    new Set(["photo", "decorative"]),
  );
  expect(mediaItems.every(({ id }) => id.startsWith("test-"))).toBe(true);
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
