import { render, screen } from "@testing-library/react";
import { ImageOff } from "lucide-react";
import { expect, test } from "vitest";

import { AppIcon } from "./AppIcon";
import { EmptyState } from "./EmptyState";

test("provides one reusable empty presentation for application surfaces", () => {
  render(
    <EmptyState
      description="Importe arquivos para começar."
      eyebrow="Fotos"
      icon={<AppIcon icon={ImageOff} size={18} />}
      title="Nenhuma Foto importada"
    />,
  );

  const state = screen.getByRole("status", {
    name: "Nenhuma Foto importada",
  });
  expect(state).toHaveClass("ui-empty-state");
  expect(screen.getByText("Fotos")).toHaveClass("ui-empty-state__eyebrow");
  expect(state).toHaveTextContent("Importe arquivos para começar.");
});
