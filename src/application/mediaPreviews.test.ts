import { expect, test } from "vitest";

import { renderableMediaPreviewUrls } from "./mediaPreviews";

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
