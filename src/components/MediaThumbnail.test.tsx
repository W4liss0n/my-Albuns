import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type { MediaCatalogItem } from "../domain/project";
import { MediaThumbnail } from "./MediaThumbnail";
import { loadedMediaPreviewImage } from "../application/mediaPreviewImages";

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

test("lends only the current loaded thumbnail and releases it when replaced or unmounted", () => {
  const url = "http://myalbuns-cache.localhost/photo.jpg";
  const media = { sourceWidthPx: 1_200, sourceHeightPx: 800 };
  const view = render(<MediaThumbnail media={media} previewUrl={url} />);
  const image = view.container.querySelector("img")!;
  expect(loadedMediaPreviewImage(url)).toBeUndefined();
  Object.defineProperties(image, {
    complete: { value: true }, naturalWidth: { value: 1_200 },
    naturalHeight: { value: 800 }, currentSrc: { value: url },
  });
  fireEvent.load(image);
  expect(loadedMediaPreviewImage(url)).toBe(image);

  const second = render(<MediaThumbnail media={media} previewUrl={url} />);
  const duplicate = second.container.querySelector("img")!;
  Object.defineProperties(duplicate, {
    complete: { value: true }, naturalWidth: { value: 1_200 },
    naturalHeight: { value: 800 }, currentSrc: { value: url },
  });
  second.unmount();
  expect(loadedMediaPreviewImage(url)).toBe(image);

  const replacement = "http://myalbuns-cache.localhost/relinked.jpg";
  view.rerender(<MediaThumbnail media={media} previewUrl={replacement} />);
  expect(loadedMediaPreviewImage(url)).toBeUndefined();
  expect(loadedMediaPreviewImage(replacement)).toBeUndefined();
  expect(view.container.querySelector("img")).not.toBe(image);
  expect(image.src).toBe(url);
  view.unmount();
  expect(loadedMediaPreviewImage(replacement)).toBeUndefined();
});
