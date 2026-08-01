import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import type { Logger } from "../application/logging";
import { GlobalShell } from "./GlobalShell";

describe("GlobalShell", () => {
  test("renders the Welcome Screen and reports readiness", async () => {
    const write = vi.fn<Logger["write"]>();

    render(<GlobalShell logger={{ write }} />);

    expect(
      screen.getByRole("heading", { name: "Projetos recentes" }),
    ).toBeInTheDocument();
    for (const action of [
      "Novo Projeto",
      "Abrir Projeto",
      "Exportação em lote",
      "Configurações",
      "Ajuda",
    ]) {
      expect(screen.getByRole("button", { name: action })).toBeDisabled();
    }
    await waitFor(() => {
      expect(write).toHaveBeenCalledWith({
        level: "info",
        component: "global_shell",
        event: "welcome_screen_ready",
      });
    });
  });
});
