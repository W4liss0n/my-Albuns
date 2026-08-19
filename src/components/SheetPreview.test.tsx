import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import placementFixture from "../../tests/fixtures/photo-placement-cases.json";
import type {
  ComposedSheet,
  PhotoPlacementPlan,
} from "../domain/project";
import { SheetPreview } from "./SheetPreview";

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
      borderFillRects: [],
      zIndex: 0,
      photo: null,
    },
  ],
};

test("renders the composed geometry and visual layers of each sheet", () => {
  render(
    <>
      <SheetPreview sheet={photoSheet} />
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
    expect.stringContaining("rotate(12) scale(-1 1)"),
  );
  expect(
    firstPreview.querySelector('[fill="#10202b"]'),
  ).toBeInTheDocument();

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
      frameBorder={{ kind: "solid", rgb: "#A0B0C0", widthUm: 1_250 }}
      sheet={photoSheet}
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
