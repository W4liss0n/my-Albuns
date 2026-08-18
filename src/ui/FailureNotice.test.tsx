import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { FailureNotice } from "./FailureNotice";

test("presents an actionable failure through the standard alert", () => {
  render(
    <FailureNotice
      failure={{
        action: "Tente novamente.",
        message: "Não foi possível concluir a operação.",
      }}
      title="Operação interrompida"
    />,
  );

  const alert = screen.getByRole("alert");
  expect(alert).toHaveTextContent("Operação interrompida");
  expect(alert).toHaveTextContent("Não foi possível concluir a operação.");
  expect(alert).toHaveTextContent("Tente novamente.");
});
