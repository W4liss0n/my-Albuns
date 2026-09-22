import type { ComponentProps } from "react";
import userEvent from "@testing-library/user-event";
import { decorativeCorpus } from "../test/decorativePreview";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import {
  type DecorativePreview,
  type VisualPersonalizationPreview,
} from "../ui/visualPreview";
import { VisualScopePreview } from "./VisualScopePreview";

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
    <VisualScopePreview
      content={{
        kind: "general",
        label: "Prévia do padrão visual",
        frameGapUm: 6_000,
        geometry: {
          bleedUm: 3_000,
          heightUm: 300_000,
          safetyUm: 5_000,
          widthUm: 600_000,
        },
        personalization,
      }}
      label="Escopo"
      scope={personalization.fixedScope}
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
    screen.getByLabelText("Fundo de ambos os lados"),
  ).toHaveAttribute("data-preview-state", "pending");
  expect(
    screen.getByLabelText("Fundo de ambos os lados"),
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
    screen.getByLabelText("Fundo de ambos os lados"),
  ).toHaveAttribute("data-preview-state", "absent");
});

test("renders an unavailable Overlay without a retained preview as the shared outline", () => {
  renderDecorativePreview({
    background: { state: "absent" },
    overlay: { state: "unavailable", url: null },
  });

  expect(screen.getByLabelText("Sobreposição de ambos os lados")).toHaveAttribute(
    "data-preview-state",
    "unavailable",
  );
  expect(screen.getByLabelText("Sobreposição de ambos os lados")).toHaveAttribute(
    "stroke",
    "#2f7fba",
  );
  expect(screen.getByLabelText("Sobreposição de ambos os lados")).toHaveAttribute(
    "rx",
    "2000",
  );
  expect(screen.getByLabelText("Sobreposição de ambos os lados")).toHaveAttribute(
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
    screen.getByLabelText("Fundo de ambos os lados"),
  ).toHaveAttribute("data-preview-state", "ready");
});


const generalContent: ComponentProps<typeof VisualScopePreview>["content"] = {
  kind: "general",
  label: "Padrões gerais",
  frameGapUm: 5_000,
  geometry: { widthUm: 600_000, heightUm: 300_000, bleedUm: 3_000, safetyUm: 5_000 },
  personalization: {
    background: { scope: "bothSides", both: { kind: "color", rgb: "#FFFFFF" } },
    frameBorder: { kind: "none" },
    overlay: { scope: "bothSides", both: null },
  },
};
const realContent: ComponentProps<typeof VisualScopePreview>["content"] = {
  kind: "sheet", sheet: decorativeCorpus.states.neutral.composition.sheets[0], mediaPreviewUrls: {},
};

test.each([generalContent, realContent])("keeps pointer, keyboard and committed scope independent for $kind content", async content => {
  const user = userEvent.setup();
  const onScopeChange = vi.fn();
  const view = render(<VisualScopePreview content={content} label="Páginas" scope="right" onScopeChange={onScopeChange} />);
  const left = screen.getByRole("button", { name: "Página esquerda" });
  const right = screen.getByRole("button", { name: "Página direita" });
  const both = screen.getByRole("button", { name: "Ambos os lados" });
  const matches = right.matches.bind(right);
  const focusVisible = vi.spyOn(right, "matches").mockImplementation(selector => selector === ":focus-visible" || matches(selector));

  await user.hover(left);
  act(() => right.focus());
  expect(screen.getByLabelText("Pré-seleção do lado esquerdo")).toBeInTheDocument();
  expect(screen.getByLabelText("Foco de teclado do lado direito")).toBeInTheDocument();
  expect(right).toHaveAttribute("aria-pressed", "true");
  expect(onScopeChange).not.toHaveBeenCalled();
  await user.unhover(left);
  expect(screen.queryByLabelText("Pré-seleção do lado esquerdo")).toBeNull();
  expect(screen.getByLabelText("Foco de teclado do lado direito")).toBeInTheDocument();

  await user.keyboard("{Enter}");
  expect(onScopeChange).toHaveBeenLastCalledWith("right");
  fireEvent.click(both);
  expect(onScopeChange).toHaveBeenLastCalledWith("both");
  expect(right).toHaveAttribute("aria-pressed", "true");
  view.rerender(<VisualScopePreview content={content} label="Páginas" scope="both" onScopeChange={onScopeChange} />);
  expect(both).toHaveAttribute("aria-pressed", "true");
  expect(screen.queryByLabelText("Lado não selecionado: esquerdo")).toBeNull();
  focusVisible.mockRestore();
});

test.each(["left", "right"] as const)("a real single page only exposes its active %s side", side => {
  const source = decorativeCorpus.states.single.composition.sheets[0];
  const onScopeChange = vi.fn();
  render(<VisualScopePreview content={{ kind: "sheet", sheet: { ...source, activeSides: side }, mediaPreviewUrls: {} }}
    label="Página única" scope={side} onScopeChange={onScopeChange} />);
  expect(screen.getAllByRole("button")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button"));
  expect(onScopeChange).toHaveBeenCalledExactlyOnceWith(side);
  expect(screen.getByRole("img", { name: "Prévia da lâmina 01" })).toBeInTheDocument();
});

test("external both-sides focus does not add a second button or change selection", () => {
  render(<VisualScopePreview content={generalContent} label="Páginas" scope="right"
    onScopeChange={vi.fn()} bothSidesControl="outside" focus={{ value: "both", onChange: vi.fn() }} />);
  expect(screen.getAllByRole("button")).toHaveLength(2);
  expect(screen.queryByRole("button", { name: "Ambos os lados" })).toBeNull();
  expect(screen.getByLabelText("Foco de teclado de ambos os lados")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Página direita" })).toHaveAttribute("aria-pressed", "true");
});
