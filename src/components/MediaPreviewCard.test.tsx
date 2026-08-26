import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { MediaPreviewCard } from "./MediaPreviewCard";

test("owns selection and thumbnail presentation as one media option", () => {
  render(
    <MediaPreviewCard
      aria-label="Textura selecionada"
      kind="media"
      media={{ sourceHeightPx: 800, sourceWidthPx: 1200 }}
      previewUrl="/texture.png"
      selected
    />,
  );

  const card = screen.getByRole("button", { name: "Textura selecionada" });
  expect(card).toHaveClass("media-preview-card");
  expect(card).toHaveAttribute("data-selected", "true");
  expect(card.querySelector(".media-preview-thumbnail img")).toHaveAttribute(
    "src",
    "/texture.png",
  );
});

test("keeps dimming and placeholder presentation inside the discriminated owner", () => {
  const view = render(
    <>
      <MediaPreviewCard
        aria-label="Foto usada"
        dimmed
        kind="media"
        media={{ sourceHeightPx: 800, sourceWidthPx: 1200 }}
        selected={false}
      />
      <MediaPreviewCard aria-label="Importar" disabled kind="placeholder">
        +
      </MediaPreviewCard>
    </>,
  );

  expect(screen.getByRole("button", { name: "Foto usada" })).toHaveAttribute(
    "data-dimmed",
    "true",
  );
  const placeholder = screen.getByRole("button", { name: "Importar" });
  expect(placeholder).toHaveClass("media-preview-card--placeholder");
  expect(placeholder).toBeDisabled();
  expect(view.container.querySelector(".media-preview-card__placeholder"))
    .not.toBeNull();
});
