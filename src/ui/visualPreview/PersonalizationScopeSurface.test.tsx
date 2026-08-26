import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import {
  PersonalizationScopeSurface,
  type DecorativePreview,
  type VisualPersonalizationPreview,
} from ".";

function renderDecorativePreview({
  background,
  overlay = null,
}: {
  background: DecorativePreview;
  overlay?: DecorativePreview | null;
}) {
  const personalization: VisualPersonalizationPreview = {
    background: {
      scope: "bothSides",
      both: { kind: "image", preview: background },
    },
    fixedScope: "both",
    frameBorder: { kind: "none" },
    overlay: {
      scope: "bothSides",
      both: overlay ? { kind: "image", preview: overlay } : null,
    },
  };
  return render(
    <PersonalizationScopeSurface
      focus={{ kind: "local" }}
      frameGapUm={6_000}
      geometry={{
        bleedUm: 3_000,
        heightUm: 300_000,
        safetyUm: 5_000,
        widthUm: 600_000,
      }}
      personalization={personalization}
      presentation={{
        accessiblePreviewLabel: "Prévia do padrão visual",
        externalSelection: false,
        scopeControlsLabel: "Escopo",
        technicalGuides: false,
      }}
      onScopeChange={() => undefined}
    />,
  );
}

test("renders a pending Background preview as a shared media fallback", () => {
  const view = renderDecorativePreview({
    background: { state: "pending" },
  });

  expect(view.container.querySelector("image")).toBeNull();
  expect(
    screen.getByLabelText("Background de ambos os lados"),
  ).toHaveAttribute("data-preview-state", "pending");
  expect(
    screen.getByLabelText("Background de ambos os lados"),
  ).toHaveAttribute("fill", "#D8DEE2");
});

test("renders a ready Decorative preview from its non-empty Cache URL", () => {
  const view = renderDecorativePreview({
    background: {
      state: "ready",
      url: "asset://localhost/cache/background.png",
    },
  });

  expect(view.container.querySelector("image")).toHaveAttribute(
    "href",
    "asset://localhost/cache/background.png",
  );
  expect(view.container.querySelector("image")).toHaveAttribute(
    "data-preview-state",
    "ready",
  );
});

test("renders an absent Background preview as the shared media fallback", () => {
  renderDecorativePreview({ background: { state: "absent" } });

  expect(
    screen.getByLabelText("Background de ambos os lados"),
  ).toHaveAttribute("data-preview-state", "absent");
});

test("renders an unavailable Overlay without a retained preview as the shared outline", () => {
  renderDecorativePreview({
    background: { state: "absent" },
    overlay: { state: "unavailable", url: null },
  });

  expect(screen.getByLabelText("Overlay de ambos os lados")).toHaveAttribute(
    "data-preview-state",
    "unavailable",
  );
  expect(screen.getByLabelText("Overlay de ambos os lados")).toHaveAttribute(
    "stroke",
    "#2f7fba",
  );
  expect(screen.getByLabelText("Overlay de ambos os lados")).toHaveAttribute(
    "rx",
    "2000",
  );
  expect(screen.getByLabelText("Overlay de ambos os lados")).toHaveAttribute(
    "stroke-width",
    "2000",
  );
});

test("keeps the last known preview while a Decorative is unavailable", () => {
  const view = renderDecorativePreview({
    background: {
      state: "unavailable",
      url: "asset://localhost/cache/retained-background.png",
    },
  });

  expect(view.container.querySelector("image")).toHaveAttribute(
    "href",
    "asset://localhost/cache/retained-background.png",
  );
  expect(view.container.querySelector("image")).toHaveAttribute(
    "data-preview-state",
    "unavailable",
  );
});

test("never renders an image with an empty ready URL", () => {
  const view = renderDecorativePreview({
    background: { state: "ready", url: "" },
  });

  expect(view.container.querySelector("image")).toBeNull();
  expect(
    screen.getByLabelText("Background de ambos os lados"),
  ).toHaveAttribute("data-preview-state", "ready");
});
