import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import placementFixture from "../../tests/fixtures/photo-placement-cases.json";
import type {
  ComposedSheet,
  PhotoPlacementPlan,
} from "../domain/project";
import { SheetPreview } from "./SheetPreview";
import { decorativeCorpus } from "../test/decorativePreview";
import { SheetDesignInspector } from "./SheetDesignInspector";

test.each([false, true])("clips decorative previews with Cache available: %s", (withCache) => {
  const sheet = decorativeCorpus.states.split.composition.sheets[0];
  const background = sheet.backgrounds.find((item) => item.kind === "media" && item.clipRect);
  if (!background || background.kind !== "media" || !background.clipRect) throw new Error("Missing Core crop fixture");
  const previewSheet = { ...sheet, overlays: [background, background] };
  const { container } = render(<SheetPreview sheet={previewSheet} mediaPreviewUrls={withCache ? { [background.mediaId]: "asset://localhost/decorative.png" } : {}} />);
  const nodes = container.querySelectorAll(`[data-preview-background-id="${background.mediaId}"], [data-preview-overlay-id="${background.mediaId}"]`);
  expect(nodes).toHaveLength(3);
  const clipPaths = new Set<string>();
  for (const node of nodes) {
    const path = node.getAttribute("clip-path");
    expect(path).toMatch(/^url\(#.+\)$/);
    clipPaths.add(path!);
    const clip = container.querySelector(`${path!.slice(4, -1)} rect`);
    expect(clip).toHaveAttribute("x", String(background.clipRect.x));
    expect(clip).toHaveAttribute("width", String(background.clipRect.width));
    expect(node).toHaveAttribute("width", String(background.drawRect.width));
  }
  expect(clipPaths.size).toBe(3);
});

test("shows the clipped Decorative belonging to each Inspector side", () => {
  const sheet = decorativeCorpus.states.split.composition.sheets[0];
  const { container } = render(<SheetDesignInspector sheet={sheet} scope="both" mediaPreviewUrls={{}} onScopeChange={() => {}} />);
  const labels = Array.from(container.querySelectorAll(".sheet-design-role:first-of-type .sheet-design-value__copy"), (node) => node.textContent);
  expect(labels).toHaveLength(2);
  sheet.backgrounds.forEach((background, index) => {
    if (background.kind === "media") expect(labels[index]).toContain(background.name);
  });
});

const photoSheet: ComposedSheet = {
  sheetId: "sheet-001",
  number: 1,
  activeSides: "both",
  widthUm: 600_000,
  heightUm: 300_000,
  base: {
    rgb: "#FFFFFF",
    drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
  },
  backgrounds: [
    {
      kind: "color",
      rgb: "#FFFFFF",
      drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
    },
  ],
  overlays: [],
  frames: [
    {
      frameId: "frame-001",
      border: { kind: "none" }, opacityByte: 255,
      clipRect: {
        x: 20_000,
        y: 20_000,
        width: 280_000,
        height: 260_000,
      },
      borderFillRects: [
        { x: 20_000, y: 20_000, width: 280_000, height: 1_250 },
        { x: 20_000, y: 278_750, width: 280_000, height: 1_250 },
        { x: 20_000, y: 20_000, width: 1_250, height: 260_000 },
        { x: 298_750, y: 20_000, width: 1_250, height: 260_000 },
      ],
      zIndex: 0,
      photo: {
        mediaId: "media-001",
        name: "Serra ao amanhecer.jpg",
        drawRect: {
          x: -50_000,
          y: 20_000,
          width: 400_000,
          height: 260_000,
        },
        placement: placementFixture.cases[0]
          .expectedPlan as PhotoPlacementPlan,
        rotationDegrees: 12,
        mirrorX: true,
        blackAndWhite: false,
        palette: ["#10202b", "#648493", "#dfa75e"],
      },
    },
  ],
};

const placeholderSheet: ComposedSheet = {
  sheetId: "sheet-002",
  number: 2,
  activeSides: "both",
  widthUm: 600_000,
  heightUm: 300_000,
  base: {
    rgb: "#FFFFFF",
    drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
  },
  backgrounds: [
    {
      kind: "color",
      rgb: "#FFFFFF",
      drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
    },
  ],
  overlays: [
    {
      mediaId: "decorative-overlay",
      name: "Overlay translúcido.png",
      drawRect: {
        x: 0,
        y: 0,
        width: 600_000,
        height: 300_000,
      },
    },
  ],
  frames: [
    {
      frameId: "frame-002",
      clipRect: {
        x: 320_000,
        y: 40_000,
        width: 250_000,
        height: 220_000,
      },
      border: { kind: "none" as const }, opacityByte: 255,
      borderFillRects: [],
      zIndex: 0,
      photo: null,
    },
  ],
};

test("never draws a synthetic photo while opening the project and loading its real preview", () => {
  const view = render(<SheetPreview sheet={photoSheet} />);
  const photo = view.container.querySelector('[data-preview-photo-id="media-001"]')!;
  expect(photo.querySelectorAll("rect, circle, image")).toHaveLength(0);
  const url = "http://myalbuns-cache.localhost/opening.jpg";
  view.rerender(<SheetPreview sheet={photoSheet} mediaPreviewUrls={{ "media-001": url }} />);
  expect(photo.querySelectorAll("rect, circle")).toHaveLength(0);
  expect(photo.querySelector("image")).toHaveAttribute("href", url);
});

test("renders the composed geometry and visual layers of each sheet", () => {
  render(
    <>
      <SheetPreview sheet={photoSheet} mediaPreviewUrls={{ "media-001": "asset://localhost/real-photo.jpg" }} />
      <SheetPreview sheet={placeholderSheet} />
    </>,
  );

  const firstPreview = screen.getByRole("img", {
    name: "Prévia da Lâmina 01",
  });
  const secondPreview = screen.getByRole("img", {
    name: "Prévia da Lâmina 02",
  });

  expect(
    firstPreview.querySelector('[data-preview-frame-id="frame-001"]'),
  ).toHaveAttribute("x", "20000");
  expect(
    firstPreview.querySelector('[data-preview-frame-id="frame-001"]'),
  ).toHaveAttribute("width", "280000");
  expect(
    firstPreview.querySelector('[data-preview-photo-id="media-001"]'),
  ).toHaveAttribute(
    "transform",
    expect.stringContaining("scale(-1 1) rotate(12)"),
  );
  expect(
    firstPreview.querySelector('[data-preview-photo-id="media-001"] image'),
  ).toHaveAttribute("href", "asset://localhost/real-photo.jpg");

  expect(
    secondPreview.querySelector("[data-preview-photo-id]"),
  ).not.toBeInTheDocument();
  const previewPlaceholder = secondPreview.querySelector(
    '[data-preview-placeholder-id="frame-002"]',
  );
  expect(previewPlaceholder).toBeInTheDocument();
  expect(previewPlaceholder?.querySelector("rect")).toHaveAttribute(
    "fill",
    "#ece8e1",
  );
  expect(previewPlaceholder?.querySelector("line")).not.toBeInTheDocument();
  const previewFrame = secondPreview.querySelector(
    '[data-preview-frame-id="frame-002"]',
  );
  expect(previewFrame).toHaveAttribute("x", "320000");
  expect(previewFrame).toHaveAttribute("stroke", "#c9c2b7");
  expect(previewFrame).toHaveAttribute("stroke-opacity", "0.88");
  expect(
    secondPreview.querySelector(
      '[data-preview-overlay-id="decorative-overlay"]',
    ),
  ).toBeInTheDocument();
});

test("uses the shared Cache URL for a transparent Decorative Overlay", () => {
  const previewUrl =
    "asset://localhost/cache/decorative-overlay.png";
  render(
    <SheetPreview
      sheet={placeholderSheet}
      mediaPreviewUrls={{
        "decorative-overlay": previewUrl,
      }}
    />,
  );

  expect(
    screen
      .getByRole("img", { name: "Prévia da Lâmina 02" })
      .querySelector(
        '[data-preview-overlay-id="decorative-overlay"]',
      ),
  ).toHaveAttribute("href", previewUrl);
});

test("preserves the canonical visual stack supplied by CompositionCore", () => {
  const canonicalStack: ComposedSheet = {
    ...placeholderSheet,
    frames: [
      {
        ...placeholderSheet.frames[0],
        frameId: "frame-top",
        zIndex: 8,
      },
      {
        ...placeholderSheet.frames[0],
        frameId: "frame-bottom",
        zIndex: 1,
      },
    ],
  };

  render(<SheetPreview sheet={canonicalStack} />);

  const preview = screen.getByRole("img", {
    name: "Prévia da Lâmina 02",
  });
  expect(
    Array.from(
      preview.querySelectorAll("[data-preview-frame-id]"),
      (frame) => frame.getAttribute("data-preview-frame-id"),
    ),
  ).toEqual(["frame-top", "frame-bottom"]);
});

test("renders the persisted solid Frame border on top of Frame content", () => {
  render(
    <SheetPreview
      sheet={{ ...photoSheet, frames: photoSheet.frames.map((frame) => ({ ...frame,
        border: { kind: "solid", rgb: "#A0B0C0", widthUm: 1_250 },
      })) }}
    />,
  );

  const border = screen
    .getByRole("img", { name: /01/ })
    .querySelector('[data-preview-frame-border-id="frame-001"]');
  const segments = border?.querySelectorAll("rect") ?? [];
  expect(segments).toHaveLength(4);
  expect(segments[0]).toHaveAttribute("fill", "#A0B0C0");
  expect(segments[0]).toHaveAttribute("x", "20000");
  expect(segments[0]).toHaveAttribute("y", "20000");
  expect(segments[0]).toHaveAttribute("width", "280000");
  expect(segments[0]).toHaveAttribute("height", "1250");
  expect(segments[3]).toHaveAttribute("x", "298750");
  expect(segments[3]).toHaveAttribute("width", "1250");
});

test("groups Photo and Border under one opacity while the Frame outline remains visible", () => {
  render(<SheetPreview sheet={{ ...photoSheet, frames: photoSheet.frames.map((frame) => ({ ...frame,
    opacityByte: 128, border: { kind: "solid", rgb: "#205070", widthUm: 1_250 },
  })) }} />);
  const svg = screen.getByRole("img", { name: /01/ });
  const content = svg.querySelector('[data-preview-frame-content-id="frame-001"]');
  expect(content).toHaveAttribute("opacity", String(128 / 255));
  expect(content?.querySelector('[data-preview-photo-id="media-001"]')).toBeInTheDocument();
  expect(content?.querySelector('[data-preview-frame-border-id="frame-001"]')).toBeInTheDocument();
  expect(content?.querySelector('[data-preview-frame-id="frame-001"]')).toBeNull();
});

test("keeps preview strokes aligned with Canvas units at other sheet heights", () => {
  render(
    <SheetPreview
      sheet={{
        ...placeholderSheet,
        heightUm: 450_000,
      }}
    />,
  );

  const preview = screen.getByRole("img", {
    name: "Prévia da Lâmina 02",
  });
  expect(
    preview.querySelector('[data-preview-frame-id="frame-002"]'),
  ).toHaveAttribute("stroke-width", "1000");
});

test("keeps the physical Sheet surface straight in every renderer", () => {
  render(<SheetPreview sheet={placeholderSheet} />);

  const surface = screen
    .getByRole("img", { name: "Prévia da Lâmina 02" })
    .querySelector(":scope > rect");

  expect(surface).toHaveAttribute("rx", "0");
});

test("represents a single-page extremity as the normalized active surface", () => {
  const singlePageSheet = {
    ...placeholderSheet,
    activeSides: "right" as const,
    widthUm: 300_000,
    base: {
      rgb: "#FFFFFF",
      drawRect: { x: 0, y: 0, width: 300_000, height: 300_000 },
    },
    backgrounds: [
      {
        kind: "color" as const,
        rgb: "#FFFFFF",
        drawRect: { x: 0, y: 0, width: 300_000, height: 300_000 },
      },
    ],
    overlays: [],
    frames: [],
  } satisfies ComposedSheet;

  const { rerender } = render(<SheetPreview sheet={singlePageSheet} />);

  const preview = screen.getByRole("img", {
    name: "Prévia da Lâmina 02",
  });
  expect(preview).toHaveAttribute("viewBox", "0 0 300000 300000");
  expect(preview.querySelector("line")).not.toBeInTheDocument();
  expect(preview.querySelector("[data-preview-inactive-side]")).toBeNull();

  rerender(
    <SheetPreview sheet={{ ...singlePageSheet, activeSides: "left" }} />,
  );
  expect(preview.querySelector("[data-preview-inactive-side]")).toBeNull();
});
