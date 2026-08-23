import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, test, vi } from "vitest";

import type { MediaCatalogItem } from "../domain/project";
import { DecorativeMediaPicker } from "./DecorativeMediaPicker";

const decorativeMedia: readonly MediaCatalogItem[] = [
  media("decorative-landscape", "Textura horizontal", 1200, 800),
  media("decorative-portrait", "Textura vertical", null, null),
];

test("closes on Escape and restores focus to the trigger", async () => {
  const user = userEvent.setup();
  renderPicker();
  const trigger = screen.getByRole("button", {
    name: "Escolher Decorativo para Background",
  });

  await user.click(trigger);
  const menu = screen.getByRole("menu", {
    name: "Decorativos para Background",
  });
  const menuItems = within(menu).getAllByRole("menuitem");
  expect(menuItems[0]).toHaveFocus();
  await user.keyboard("{ArrowRight}");
  expect(menuItems[1]).toHaveFocus();
  await user.keyboard("{Home}");
  expect(menuItems[0]).toHaveFocus();

  await user.keyboard("{Escape}");

  expect(menu).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
});

test("closes on an outside pointer and restores focus to the trigger", async () => {
  const user = userEvent.setup();
  render(
    <>
      <PickerHarness />
      <button type="button">Fora do seletor</button>
    </>,
  );
  const trigger = screen.getByRole("button", {
    name: "Escolher Decorativo para Background",
  });

  await user.click(trigger);
  await user.click(screen.getByRole("button", { name: "Fora do seletor" }));

  expect(
    screen.queryByRole("menu", { name: "Decorativos para Background" }),
  ).not.toBeInTheDocument();
  await waitFor(() => expect(trigger).toHaveFocus());
});

test("keeps one picker open and exposes valid menu item semantics", async () => {
  const user = userEvent.setup();
  render(<PairedPickerHarness />);

  await user.click(
    screen.getByRole("button", {
      name: "Escolher Decorativo para Background",
    }),
  );
  expect(
    screen.getByRole("menu", { name: "Decorativos para Background" }),
  ).toBeInTheDocument();

  await user.click(
    screen.getByRole("button", {
      name: "Escolher Decorativo para Overlay",
    }),
  );

  expect(
    screen.queryByRole("menu", { name: "Decorativos para Background" }),
  ).not.toBeInTheDocument();
  const overlayMenu = screen.getByRole("menu", {
    name: "Decorativos para Overlay",
  });
  const options = within(overlayMenu).getAllByRole("menuitem");
  expect(options[0]).toHaveFocus();
  expect(options[0]).not.toHaveAttribute("aria-pressed");
  expect(options[0]).toHaveAttribute("data-selected", "false");
  expect(
    options[0].querySelector(".media-preview-thumbnail"),
  ).toHaveAttribute("data-has-preview", "false");
  expect(
    within(overlayMenu).getByRole("menuitem", {
      name: "Importar Decorativo",
    }),
  ).toBeDisabled();
});

test("uses the shared intrinsic ratio inside the decorative menu", async () => {
  const user = userEvent.setup();
  renderPicker();
  await user.click(
    screen.getByRole("button", {
      name: "Escolher Decorativo para Background",
    }),
  );
  const portraitOption = screen.getByRole("menuitem", {
    name: "Usar Background Textura vertical",
  });
  const image = portraitOption.querySelector("img");
  expect(image).not.toBeNull();
  Object.defineProperties(image!, {
    naturalHeight: { configurable: true, value: 1200 },
    naturalWidth: { configurable: true, value: 800 },
  });

  fireEvent.load(image!);

  const thumbnail = portraitOption.querySelector<HTMLElement>(
    ".media-preview-thumbnail",
  );
  expect(thumbnail).toHaveAttribute("data-portrait", "true");
  expect(thumbnail).toHaveStyle({
    "--media-aspect-ratio": "800 / 1200",
  });
});

function PickerHarness() {
  const [open, setOpen] = useState(false);
  return (
    <DecorativeMediaPicker
      decorativeMedia={decorativeMedia}
      label="Background"
      mediaPreviewUrls={{
        "decorative-landscape": "/horizontal.png",
        "decorative-portrait": "/vertical.png",
      }}
      open={open}
      selectedMediaId={null}
      onOpenChange={setOpen}
      onSelect={vi.fn()}
    />
  );
}

function PairedPickerHarness() {
  const [openPicker, setOpenPicker] = useState<
    "Background" | "Overlay" | null
  >(null);
  return (
    <>
      {(["Background", "Overlay"] as const).map((label) => (
        <DecorativeMediaPicker
          decorativeMedia={decorativeMedia}
          key={label}
          label={label}
          mediaPreviewUrls={{}}
          open={openPicker === label}
          selectedMediaId={null}
          onOpenChange={(open) => setOpenPicker(open ? label : null)}
          onSelect={vi.fn()}
        />
      ))}
    </>
  );
}

function renderPicker() {
  return render(<PickerHarness />);
}

function media(
  id: string,
  name: string,
  sourceWidthPx: number | null,
  sourceHeightPx: number | null,
) {
  return {
    id,
    kind: "decorative",
    name,
    palette: null,
    sourceHeightPx,
    sourceWidthPx,
  } satisfies MediaCatalogItem;
}
