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
  widthUm: 600_000,
  heightUm: 300_000,
  hasOverlay: false,
  frames: [
    {
      frameId: "frame-001",
      clipRect: {
        x: 20_000,
        y: 20_000,
        width: 280_000,
        height: 260_000,
      },
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
  widthUm: 600_000,
  heightUm: 300_000,
  hasOverlay: true,
  frames: [
    {
      frameId: "frame-002",
      clipRect: {
        x: 320_000,
        y: 40_000,
        width: 250_000,
        height: 220_000,
      },
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
  expect(
    secondPreview.querySelector('[data-preview-placeholder-id="frame-002"]'),
  ).toBeInTheDocument();
  expect(
    secondPreview.querySelector('[data-preview-frame-id="frame-002"]'),
  ).toHaveAttribute("x", "320000");
  expect(
    secondPreview.querySelector("[data-preview-overlay]"),
  ).toBeInTheDocument();
});
