import { render, screen } from "@testing-library/react";
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
