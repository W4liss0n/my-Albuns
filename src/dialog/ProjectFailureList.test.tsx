import { render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { ProjectFailureList } from "./ProjectFailureList";

test("each Project that did not open is named with its own reason and action", () => {
  render(<ProjectFailureList projects={[
    {
      name: "SARAH XAVIER",
      message: "Este projeto já está aberto em outra janela.",
      action: "Use a janela já aberta ou feche-a antes de tentar novamente.",
    },
    { name: "YUELSON RODRIGO", message: "O arquivo não foi encontrado." },
  ]} />);

  const list = screen.getByRole("list", { name: "Projetos que não abriram" });
  const items = within(list).getAllByRole("listitem");
  expect(items.map((item) => item.textContent)).toEqual([
    "SARAH XAVIEREste projeto já está aberto em outra janela.Use a janela já aberta ou feche-a antes de tentar novamente.",
    "YUELSON RODRIGOO arquivo não foi encontrado.",
  ]);
});
