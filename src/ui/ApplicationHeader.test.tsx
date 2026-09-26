import {
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import { ApplicationHeader } from "./ApplicationHeader";
import { WindowControlsProvider } from "./WindowControlsContext";

const windowActions = {
  close: vi.fn(async () => undefined),
  fitContent: vi.fn(async () => undefined),
  minimize: vi.fn(async () => undefined),
  toggleMaximize: vi.fn(async () => undefined),
};

beforeEach(() => {
  vi.clearAllMocks();
});

test("exposes a draggable custom window titlebar with native controls", () => {
  const view = render(
    <WindowControlsProvider controls={windowActions}>
      <ApplicationHeader status="pronto" />
    </WindowControlsProvider>,
  );

  expect(
    screen.getByRole("banner", { name: "Barra da janela" }),
  ).toBeInTheDocument();
  expect(
    view.container.querySelector("[data-tauri-drag-region]"),
  ).not.toBeNull();

  expect(
    within(screen.getByRole("group", { name: "Controles da janela" }))
      .getAllByRole("button")
      .map((button) => button.getAttribute("aria-label")),
  ).toEqual([
    "Minimizar janela",
    "Maximizar ou restaurar janela",
    "Fechar janela",
  ]);

  fireEvent.click(screen.getByRole("button", { name: "Fechar janela" }));
  fireEvent.click(
    screen.getByRole("button", { name: "Minimizar janela" }),
  );
  fireEvent.click(
    screen.getByRole("button", {
      name: "Maximizar ou restaurar janela",
    }),
  );

  expect(windowActions.close).toHaveBeenCalledOnce();
  expect(windowActions.minimize).toHaveBeenCalledOnce();
  expect(windowActions.toggleMaximize).toHaveBeenCalledOnce();
});

test("uses the same titlebar with only the controls supported by a dialog", () => {
  const { rerender } = render(
    <WindowControlsProvider controls={windowActions}>
      <ApplicationHeader controls="close" />
    </WindowControlsProvider>,
  );

  expect(
    within(screen.getByRole("group", { name: "Controles da janela" }))
      .getAllByRole("button")
      .map((button) => button.getAttribute("aria-label")),
  ).toEqual(["Fechar janela"]);
  expect(
    screen.getByRole("banner", { name: "Barra da janela" }),
  ).toHaveAttribute("data-window-controls", "close");

  rerender(
    <WindowControlsProvider controls={windowActions}>
      <ApplicationHeader controls="none" />
    </WindowControlsProvider>,
  );

  expect(
    screen.queryByRole("group", { name: "Controles da janela" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("banner", { name: "Barra da janela" }),
  ).toHaveAttribute("data-window-controls", "none");
  expect(
    document.querySelector("[data-tauri-drag-region]"),
  ).not.toBeNull();
});

test("a document window puts its identity at the left and can emphasize its state", () => {
  render(<WindowControlsProvider controls={windowActions}>
    <ApplicationHeader align="start" context="Álbum Horizonte" metadata="300×300 mm · 2 lâminas" status="Alterações não salvas" statusEmphasis />
  </WindowControlsProvider>);
  const header = screen.getByRole("banner", { name: "Barra da janela" });
  expect(header).toHaveAttribute("data-align", "start");
  expect(header).toHaveAttribute("data-status-emphasis", "true");
  expect(screen.getByLabelText("MyAlbuns")).toBeInTheDocument();
  expect(screen.getByText("Alterações não salvas")).toBeInTheDocument();
});

test("viewer titlebar keeps maximize and close without an inaccessible minimize action", () => {
  render(<WindowControlsProvider controls={windowActions}>
    <ApplicationHeader showBrand={false} controls="maximize-close" context="Nome muito longo da imagem.jpg" />
  </WindowControlsProvider>);
  expect(screen.queryByRole("button", { name: "Minimizar janela" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Maximizar ou restaurar janela" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Fechar janela" })).toBeInTheDocument();
  expect(screen.getByTitle("Nome muito longo da imagem.jpg")).toBeInTheDocument();
});
