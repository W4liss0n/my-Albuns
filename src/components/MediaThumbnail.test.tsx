import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type { MediaCatalogItem } from "../domain/project";
import { MediaThumbnail } from "./MediaThumbnail";
import { loadedMediaPreviewImage } from "../application/mediaPreviewImages";

test("keeps the displayed image while its replacement loads", () => {
  const media = { sourceWidthPx: 1200, sourceHeightPx: 800 };
  const view = render(<MediaThumbnail media={media} previewUrl="/previous.jpg" />);
  const previous = view.container.querySelector("img")!;
  fireEvent.load(previous);
  view.rerender(<MediaThumbnail media={media} previewUrl="/replacement.jpg" />);
  expect(previous).toBeInTheDocument();
  expect(previous).toBeVisible();
  const replacement = view.container.querySelector<HTMLImageElement>('img[src="/replacement.jpg"]')!;
  expect(replacement).not.toBeNull();
  fireEvent.load(replacement);
  expect(replacement).toBeVisible();
  expect(previous).not.toBeInTheDocument();
});

test("distinguishes an absent thumbnail from loading and restores the photo when Cache arrives", () => {
  const media = { sourceWidthPx: 800, sourceHeightPx: 1200 };
  const view = render(<MediaThumbnail media={media} />);
  const thumbnail = view.container.firstChild;
  expect(thumbnail).toHaveAttribute("data-missing", "false");
  view.rerender(<MediaThumbnail media={media} missing />);
  expect(thumbnail).toHaveAttribute("data-missing", "true");
  expect(thumbnail).toHaveAttribute("data-portrait", "true");
  expect(thumbnail).toHaveStyle({ "--media-aspect-ratio": "800 / 1200" });
  view.rerender(<MediaThumbnail media={media} missing previewUrl="/cached.jpg" />);
  expect(thumbnail).toHaveAttribute("data-missing", "false");
  expect(view.container.querySelector("img")).toHaveAttribute("src", "/cached.jpg");
  expect(view.container.querySelector(".media-preview-thumbnail__missing-symbol")).toBeNull();
});

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

test("lends loaded generations without changing their sources and releases them after the swap", () => {
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
  expect(loadedMediaPreviewImage(url)).toBe(image);
  expect(loadedMediaPreviewImage(replacement)).toBeUndefined();
  const nextImage = view.container.querySelector<HTMLImageElement>(`img[src="${replacement}"]`)!;
  Object.defineProperties(nextImage, {
    complete: { value: true }, naturalWidth: { value: 1_200 },
    naturalHeight: { value: 800 }, currentSrc: { value: replacement },
  });
  fireEvent.load(nextImage);
  expect(loadedMediaPreviewImage(url)).toBeUndefined();
  expect(loadedMediaPreviewImage(replacement)).toBe(nextImage);
  expect(image.src).toBe(url);
  view.unmount();
  expect(loadedMediaPreviewImage(replacement)).toBeUndefined();
});

test("keeps only the last displayed and latest requested images during rapid replacements", () => {
  const media = { sourceWidthPx: 1200, sourceHeightPx: 800 };
  const view = render(<MediaThumbnail media={media} previewUrl="/a.jpg" />);
  const previous = view.container.querySelector("img")!;
  fireEvent.load(previous);
  view.rerender(<MediaThumbnail media={media} previewUrl="/b.jpg" />);
  const superseded = view.container.querySelector('img[src="/b.jpg"]')!;
  view.rerender(<MediaThumbnail media={media} previewUrl="/c.jpg" />);
  expect(superseded).not.toBeInTheDocument();
  expect(view.container.querySelectorAll("img")).toHaveLength(2);
  fireEvent.load(superseded);
  const latest = view.container.querySelector('img[src="/c.jpg"]')!;
  fireEvent.error(latest);
  expect(previous).toBeVisible();
  fireEvent.load(latest);
  expect(previous).not.toBeInTheDocument();
  expect(view.container.querySelectorAll("img")).toHaveLength(1);
});

test("clears a retired preview instead of retaining it across missing Cache", () => {
  const media = { sourceWidthPx: 1200, sourceHeightPx: 800 };
  const view = render(<MediaThumbnail media={media} previewUrl="/old.jpg" />);
  const previous = view.container.querySelector("img")!;
  fireEvent.load(previous);
  view.rerender(<MediaThumbnail media={media} missing />);
  expect(previous).not.toBeInTheDocument();
  expect(view.container.firstChild).toHaveAttribute("data-missing", "true");
  view.rerender(<MediaThumbnail media={media} previewUrl="/new.jpg" />);
  expect(view.container.querySelectorAll("img")).toHaveLength(1);
  expect(view.container.querySelector("img")).toHaveAttribute("src", "/new.jpg");
});

test("keeps the previous dimensions until the replacement is loaded", () => {
  const portrait = { sourceWidthPx: 800, sourceHeightPx: 1200 };
  const landscape = { sourceWidthPx: 1200, sourceHeightPx: 800 };
  const view = render(<MediaThumbnail media={portrait} previewUrl="/portrait.jpg" />);
  fireEvent.load(view.container.querySelector("img")!);
  view.rerender(<MediaThumbnail media={landscape} previewUrl="/landscape.jpg" />);
  expect(view.container.firstChild).toHaveAttribute("data-portrait", "true");
  fireEvent.load(view.container.querySelector('img[src="/landscape.jpg"]')!);
  expect(view.container.firstChild).toHaveAttribute("data-portrait", "false");
});
