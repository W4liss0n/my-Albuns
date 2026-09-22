import { fireEvent, screen } from "@testing-library/react";

/** Exercise the same complete palette used in every product surface. */
export function chooseColor(label: string, hex: string) {
  fireEvent.click(screen.getByRole("button", { name: `Cor ${label}` }));
  fireEvent.change(screen.getByRole("textbox", { name: `Código da cor ${label}` }), { target: { value: hex } });
  fireEvent.click(screen.getByRole("button", { name: "Aplicar cor" }));
}
