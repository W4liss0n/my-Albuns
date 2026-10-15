import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { MenuGroups, MenuItem } from "./MenuItem";

test("separates only the groups that still have items", () => {
  render(<div role="menu">
    <MenuGroups groups={[
      [false && <MenuItem label="Oculto" />],
      [<MenuItem label="Abrir" />, null],
      [undefined],
      [<MenuItem label="Mover" />, <MenuItem label="Copiar" />],
      [<MenuItem label="Remover" />],
    ]} />
  </div>);
  const order = [...screen.getByRole("menu").children].map((child) =>
    child.getAttribute("role") === "separator" ? "|" : child.textContent);
  expect(order).toEqual(["Abrir", "|", "Mover", "Copiar", "|", "Remover"]);
});
