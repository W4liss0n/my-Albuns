import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type { MediaCatalogItem } from "../domain/project";
import { MediaThumbnail } from "./MediaThumbnail";

test("uses the loaded preview ratio when catalog dimensions are unavailable", () => {
  const media = {
    id: "decorative-without-metadata",
    kind: "decorative",
    name: "Textura vertical",
    palette: null,
    sourceHeightPx: null,
    sourceWidthPx: null,
  } satisfies MediaCatalogItem;

  render(
    <MediaThumbnail
      aria-label="Miniatura compartilhada"
      media={media}
      previewUrl="/textura-vertical.png"
    />,
  );

  const thumbnail = screen.getByLabelText("Miniatura compartilhada");
  const image = thumbnail.querySelector("img");
  expect(image).not.toBeNull();
  Object.defineProperties(image!, {
    naturalHeight: { configurable: true, value: 1200 },
    naturalWidth: { configurable: true, value: 800 },
  });

  fireEvent.load(image!);

  expect(thumbnail).toHaveAttribute("data-portrait", "true");
  expect(thumbnail).toHaveStyle({
    "--media-aspect-ratio": "800 / 1200",
  });
});
