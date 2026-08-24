import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";

import { SheetGridPreview } from "./SheetGridPreview";

test("moves the Grade highlight when a sheet is clicked", async () => {
  const user = userEvent.setup();
  render(<SheetGridPreview />);

  const firstSheet = screen.getByRole("button", {
    name: "Ir para Lâmina 01, Lâmina inicial, Página 1",
  });
  const secondSheet = screen.getByRole("button", {
    name: "Ir para Lâmina 02, Páginas 2–3",
  });
  expect(secondSheet).toHaveAttribute("aria-current", "true");

  await user.click(firstSheet);

  expect(firstSheet).toHaveAttribute("aria-current", "true");
  expect(secondSheet).not.toHaveAttribute("aria-current");
});

test("recomposes the development Grade after applying Album changes", async () => {
  render(<SheetGridPreview />);

  const information = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  const firstPreview = screen.getByRole("img", {
    name: "Prévia da Lâmina 01",
  });
  expect(firstPreview).toHaveAttribute("viewBox", "0 0 300000 300000");

  fireEvent.change(information.getByLabelText("Primeira Lâmina"), {
    target: { value: "double" },
  });
  fireEvent.change(information.getByRole("textbox", { name: "Largura" }), {
    target: { value: "700" },
  });
  fireEvent.change(information.getByRole("textbox", { name: "Altura" }), {
    target: { value: "350" },
  });
  const applyInformation = information.getByRole("button", {
    name: "Aplicar",
  });
  await waitFor(() => expect(applyInformation).toBeEnabled());
  fireEvent.click(applyInformation);

  await waitFor(() =>
    expect(firstPreview).toHaveAttribute("viewBox", "0 0 700000 350000"),
  );
  expect(
    screen.getByRole("button", {
      name: "Ir para Lâmina 01, Páginas 1–2",
    }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", {
      name: "Ir para Lâmina 02, Páginas 3–4",
    }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", {
      name: "Ir para Lâmina 06, Lâmina final, Página 11",
    }),
  ).toBeInTheDocument();

  const design = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(design.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  fireEvent.click(design.getByRole("button", { name: "Aplicar" }));

  await waitFor(() =>
    expect(
      firstPreview.querySelector('[data-preview-background-color="#F7F5F0"]'),
    ).toBeInTheDocument(),
  );
});
