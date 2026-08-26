import { expect, test } from "vitest";

import {
  renderableMediaPreviewUrl,
  renderableMediaPreviewUrls,
} from "./mediaPreviews";

test.each([
  [{ state: "pending" as const }, null],
  [{ state: "absent" as const }, null],
  [{ state: "ready" as const, url: "" }, null],
  [
    { state: "ready" as const, url: "asset://localhost/cache/ready.png" },
    "asset://localhost/cache/ready.png",
  ],
  [{ state: "unavailable" as const, url: null }, null],
  [
    {
      state: "unavailable" as const,
      url: "asset://localhost/cache/retained.png",
    },
    "asset://localhost/cache/retained.png",
  ],
])("selects one renderable Cache URL from %o", (preview, expected) => {
  expect(renderableMediaPreviewUrl(preview)).toBe(expected);
});

test("projects only ready and retained unavailable Cache preview URLs", () => {
  expect(
    renderableMediaPreviewUrls({
      absent: { mediaId: "absent", state: "absent", url: null },
      empty: { mediaId: "empty", state: "ready", url: "" },
      ready: {
        mediaId: "ready",
        state: "ready",
        url: "asset://localhost/cache/ready.png",
      },
      retained: {
        mediaId: "retained",
        state: "unavailable",
        url: "asset://localhost/cache/retained.png",
      },
      unavailable: {
        mediaId: "unavailable",
        state: "unavailable",
        url: null,
      },
    }),
  ).toEqual({
    ready: "asset://localhost/cache/ready.png",
    retained: "asset://localhost/cache/retained.png",
  });
});
